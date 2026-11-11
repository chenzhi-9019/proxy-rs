//! UDP `UDP ASSOCIATE` 中继。
//!
//! 每个关联拥有一个绑定在所选网卡 IP 上的中继套接字，以及按地址族懒创建的
//! 出站套接字。关联与控制连接同生命周期，客户端来源 IP 与端口会被锁定。

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::{TcpStream, UdpSocket};
use tokio::task::JoinHandle;
use tokio::time::{Instant, timeout};
use tokio_util::sync::CancellationToken;

use super::socks5::{self, Host, Target};

const IDLE_TIMEOUT: Duration = Duration::from_secs(300);
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(5);
const DATAGRAM_SIZE: usize = 65_535;

/// 在指定 IP 上绑定 UDP 中继套接字（端口由系统分配）。
pub async fn bind_relay(bind_ip: IpAddr) -> std::io::Result<UdpSocket> {
    UdpSocket::bind(SocketAddr::new(bind_ip, 0)).await
}

/// 运行 UDP 关联，直到控制连接关闭、服务停止或空闲超时。
pub async fn run(
    mut control: TcpStream,
    relay: UdpSocket,
    client_ip: IpAddr,
    bind_ip: IpAddr,
    cancel: CancellationToken,
) {
    let relay = Arc::new(relay);
    let client_address: Arc<Mutex<Option<SocketAddr>>> = Arc::new(Mutex::new(None));
    let association_cancel = CancellationToken::new();
    let mut upstream_v4: Option<Arc<UdpSocket>> = None;
    let mut upstream_v6: Option<Arc<UdpSocket>> = None;
    let mut readers: Vec<JoinHandle<()>> = Vec::new();
    let mut control_buffer = [0u8; 1];
    let mut buffer = vec![0u8; DATAGRAM_SIZE];

    let idle = tokio::time::sleep(IDLE_TIMEOUT);
    tokio::pin!(idle);

    loop {
        tokio::select! {
          _ = cancel.cancelled() => break,
          _ = &mut idle => break,
          result = control.read(&mut control_buffer) => {
            match result {
              Ok(0) | Err(_) => break,
              Ok(_) => {}
            }
          }
          result = relay.recv_from(&mut buffer) => {
            let Ok((length, source)) = result else { break };
            if source.ip() != client_ip {
              continue;
            }
            {
              let mut known = client_address.lock().expect("client address lock");
              match *known {
                None => *known = Some(source),
                Some(address) if address != source => continue,
                Some(_) => {}
              }
            }

            let Ok(packet) = socks5::parse_udp_packet(&buffer[..length]) else {
              continue;
            };
            let Some(target) = resolve_target(&packet.target).await else {
              continue;
            };

            let ipv4 = target.is_ipv4();
            let slot = if ipv4 {
                &mut upstream_v4
            } else {
                &mut upstream_v6
            };
            if slot.is_none() {
                let Ok(socket) = bind_upstream(ipv4, bind_ip).await else {
                    continue;
                };
                let socket = Arc::new(socket);
                readers.push(spawn_reader(
                    Arc::clone(&socket),
                    Arc::clone(&relay),
                    Arc::clone(&client_address),
                    association_cancel.clone(),
                ));
                *slot = Some(socket);
            }

            if let Some(socket) = slot.as_ref() {
                let _ = socket.send_to(&packet.payload, target).await;
            }
            idle
              .as_mut()
              .reset(Instant::now() + IDLE_TIMEOUT);
          }
        }
    }

    association_cancel.cancel();
    for reader in readers {
        reader.abort();
        let _ = reader.await;
    }
}

async fn resolve_target(target: &Target) -> Option<SocketAddr> {
    match &target.host {
        Host::Ip(ip) => Some(SocketAddr::new(*ip, target.port)),
        Host::Domain(domain) => {
            let result = timeout(
                RESOLVE_TIMEOUT,
                tokio::net::lookup_host((domain.as_str(), target.port)),
            )
            .await;
            result.ok()?.ok()?.next()
        }
    }
}

/// 创建出站套接字；地址族与所选源 IP 一致且源 IP 非回环时绑定该源地址。
async fn bind_upstream(ipv4: bool, bind_ip: IpAddr) -> std::io::Result<UdpSocket> {
    let unspecified = if ipv4 {
        SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 0)
    } else {
        SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED), 0)
    };
    let preferred =
        if !bind_ip.is_loopback() && !bind_ip.is_unspecified() && ipv4 == bind_ip.is_ipv4() {
            SocketAddr::new(bind_ip, 0)
        } else {
            unspecified
        };

    match UdpSocket::bind(preferred).await {
        Ok(socket) => Ok(socket),
        Err(error) => {
            eprintln!("[proxy-rs] 绑定 UDP 源地址 {preferred} 失败，回退默认路由：{error}");
            UdpSocket::bind(unspecified).await
        }
    }
}

fn spawn_reader(
    socket: Arc<UdpSocket>,
    relay: Arc<UdpSocket>,
    client_address: Arc<Mutex<Option<SocketAddr>>>,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut buffer = vec![0u8; DATAGRAM_SIZE];
        loop {
            tokio::select! {
              _ = cancel.cancelled() => break,
              result = socket.recv_from(&mut buffer) => {
                let Ok((length, source)) = result else { break };
                let client = *client_address.lock().expect("client address lock");
                let Some(client) = client else { continue };
                let packet = socks5::encode_udp_response(source, &buffer[..length]);
                if relay.send_to(&packet, client).await.is_err() {
                  break;
                }
              }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio::time::sleep;

    async fn tcp_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("addr");
        let client = TcpStream::connect(address).await.expect("connect");
        let (server, _) = listener.accept().await.expect("accept");
        (client, server)
    }

    async fn udp_echo() -> SocketAddr {
        let socket = UdpSocket::bind("127.0.0.1:0").await.expect("bind echo");
        let address = socket.local_addr().expect("addr");
        tokio::spawn(async move {
            let mut buffer = [0u8; 2048];
            while let Ok((length, peer)) = socket.recv_from(&mut buffer).await {
                let _ = socket.send_to(&buffer[..length], peer).await;
            }
        });
        address
    }

    #[tokio::test]
    async fn relays_datagram_and_returns_source_header() {
        let (control, server) = tcp_pair().await;
        let relay = bind_relay("127.0.0.1".parse().unwrap())
            .await
            .expect("relay");
        let relay_address = relay.local_addr().expect("relay addr");
        let cancel = CancellationToken::new();
        let task = tokio::spawn(run(
            server,
            relay,
            "127.0.0.1".parse().unwrap(),
            "127.0.0.1".parse().unwrap(),
            cancel,
        ));

        let echo = udp_echo().await;
        let client = UdpSocket::bind("127.0.0.1:0").await.expect("client");
        let request = socks5::encode_udp_request(&Target::ip(echo.ip(), echo.port()), b"udp-ping");
        client
            .send_to(&request, relay_address)
            .await
            .expect("send request");

        let mut buffer = [0u8; 2048];
        let (length, _) = timeout(Duration::from_secs(2), client.recv_from(&mut buffer))
            .await
            .expect("response timeout")
            .expect("recv");
        let response = socks5::parse_udp_packet(&buffer[..length]).expect("parse response");
        assert_eq!(response.target, Target::ip(echo.ip(), echo.port()));
        assert_eq!(response.payload, b"udp-ping");

        drop(control);
        timeout(Duration::from_secs(2), task)
            .await
            .expect("association timeout")
            .expect("join");
    }

    #[tokio::test]
    async fn drops_fragmented_datagram_and_keeps_association() {
        let (_control, server) = tcp_pair().await;
        let relay = bind_relay("127.0.0.1".parse().unwrap())
            .await
            .expect("relay");
        let relay_address = relay.local_addr().expect("relay addr");
        let task = tokio::spawn(run(
            server,
            relay,
            "127.0.0.1".parse().unwrap(),
            "127.0.0.1".parse().unwrap(),
            CancellationToken::new(),
        ));

        let echo = udp_echo().await;
        let client = UdpSocket::bind("127.0.0.1:0").await.expect("client");
        let fake = [0u8, 0, 3, 0, 0, 0, 0, 0, 0, 1];
        client
            .send_to(&fake, relay_address)
            .await
            .expect("send fragments");
        let valid = socks5::encode_udp_request(&Target::ip(echo.ip(), echo.port()), b"after-frag");
        client
            .send_to(&valid, relay_address)
            .await
            .expect("send valid");

        let mut buffer = [0u8; 2048];
        let (length, _) = timeout(Duration::from_secs(2), client.recv_from(&mut buffer))
            .await
            .expect("response timeout")
            .expect("recv");
        let response = socks5::parse_udp_packet(&buffer[..length]).expect("parse response");
        assert_eq!(response.payload, b"after-frag");

        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn releases_relay_port_when_control_closes() {
        let (control, server) = tcp_pair().await;
        let relay = bind_relay("127.0.0.1".parse().unwrap())
            .await
            .expect("relay");
        let relay_address = relay.local_addr().expect("relay addr");
        let task = tokio::spawn(run(
            server,
            relay,
            "127.0.0.1".parse().unwrap(),
            "127.0.0.1".parse().unwrap(),
            CancellationToken::new(),
        ));

        drop(control);
        timeout(Duration::from_secs(2), task)
            .await
            .expect("association timeout")
            .expect("join");

        let mut released = false;
        for _ in 0..20 {
            if let Ok(socket) = UdpSocket::bind(relay_address).await {
                drop(socket);
                released = true;
                break;
            }
            sleep(Duration::from_millis(25)).await;
        }
        assert!(released, "relay port {relay_address} should be released");
    }
}
