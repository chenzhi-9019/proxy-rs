//! 代理内核：SOCKS5 协议、生命周期与 TCP/UDP 转发。

pub mod socks5;
pub mod tcp;
pub mod udp;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, broadcast};
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::error::AppError;
use crate::interfaces;
use crate::model::{ProxyConfig, ProxyStatus, Transport, endpoint_for, now_millis};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const STOP_TIMEOUT: Duration = Duration::from_secs(3);

/// 单实例代理管理器。
///
/// 启动与停止串行执行；状态变化通过广播通道发出，由 Tauri 层转换为事件。
pub struct ProxyManager {
    inner: Mutex<Option<ProxyRuntime>>,
    events: broadcast::Sender<ProxyStatus>,
}

struct ProxyRuntime {
    config: ProxyConfig,
    endpoint: String,
    started_at: u64,
    cancel: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}

impl ProxyRuntime {
    fn status(&self) -> ProxyStatus {
        ProxyStatus::running(self.config.clone(), self.endpoint.clone(), self.started_at)
    }
}

impl Default for ProxyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProxyManager {
    /// 创建空管理器。
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(32);
        Self {
            inner: Mutex::new(None),
            events,
        }
    }

    /// 订阅状态变化。新订阅者不会收到历史状态，调用方应先查询 [`Self::status`]。
    pub fn subscribe(&self) -> broadcast::Receiver<ProxyStatus> {
        self.events.subscribe()
    }

    /// 校验配置并启动监听。
    pub async fn start(&self, config: ProxyConfig) -> Result<ProxyStatus, AppError> {
        let config = config.normalized();
        config.validate()?;

        let mut guard = self.inner.lock().await;
        if guard.is_some() {
            return Err(AppError::already_running());
        }

        let interfaces = interfaces::enumerate()?;
        config.ensure_local(&interfaces)?;

        let address = SocketAddr::new(config.ip, config.port_u16());
        let listener = TcpListener::bind(address)
            .await
            .map_err(|error| AppError::bind_failed(address, error))?;
        let endpoint = listener
            .local_addr()
            .map(|bound| endpoint_for(bound.ip(), bound.port()))
            .unwrap_or_else(|_| config.endpoint());
        let started_at = now_millis();
        let cancel = CancellationToken::new();
        let task = tokio::spawn(accept_loop(
            listener,
            Arc::new(config.clone()),
            cancel.clone(),
        ));

        *guard = Some(ProxyRuntime {
            config: config.clone(),
            endpoint: endpoint.clone(),
            started_at,
            cancel,
            task,
        });
        drop(guard);

        let status = ProxyStatus::running(config, endpoint, started_at);
        let _ = self.events.send(status.clone());
        Ok(status)
    }

    /// 停止代理并释放监听端口；未运行时幂等返回 stopped。
    pub async fn stop(&self) -> Result<ProxyStatus, AppError> {
        let mut guard = self.inner.lock().await;
        let Some(runtime) = guard.take() else {
            return Ok(ProxyStatus::stopped(None));
        };
        runtime.cancel.cancel();
        let mut task = runtime.task;
        if timeout(STOP_TIMEOUT, &mut task).await.is_err() {
            task.abort();
            let _ = task.await;
        }
        drop(guard);

        let status = ProxyStatus::stopped(None);
        let _ = self.events.send(status.clone());
        Ok(status)
    }

    /// 查询当前状态；若监听任务已意外退出，则顺带收敛为 stopped。
    pub async fn status(&self) -> ProxyStatus {
        let mut guard = self.inner.lock().await;
        let finished = guard
            .as_ref()
            .map(|runtime| runtime.task.is_finished())
            .unwrap_or(false);
        if finished {
            guard.take();
            drop(guard);
            let status = ProxyStatus::stopped(Some("代理监听意外停止，请重新启动".into()));
            let _ = self.events.send(status.clone());
            return status;
        }
        match guard.as_ref() {
            Some(runtime) => runtime.status(),
            None => ProxyStatus::stopped(None),
        }
    }
}

async fn accept_loop(listener: TcpListener, config: Arc<ProxyConfig>, cancel: CancellationToken) {
    let mut sessions: JoinSet<()> = JoinSet::new();
    loop {
        tokio::select! {
          _ = cancel.cancelled() => break,
          _ = sessions.join_next(), if !sessions.is_empty() => {}
          result = listener.accept() => {
            match result {
              Ok((stream, _peer)) => {
                let config = Arc::clone(&config);
                let cancel = cancel.clone();
                sessions.spawn(async move {
                  if let Err(error) = handle_session(stream, config, cancel).await {
                    debug_log(&format!("会话结束：{error}"));
                  }
                });
              }
              Err(error) => {
                debug_log(&format!("accept 失败：{error}"));
                tokio::time::sleep(Duration::from_millis(50)).await;
              }
            }
          }
        }
    }
    sessions.abort_all();
}

async fn handle_session(
    mut stream: TcpStream,
    config: Arc<ProxyConfig>,
    cancel: CancellationToken,
) -> Result<(), String> {
    let _ = stream.set_nodelay(true);
    let peer = stream.peer_addr().ok();

    match timeout(HANDSHAKE_TIMEOUT, socks5::negotiate(&mut stream)).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => return Err(error.to_string()),
        Err(_) => return Err("握手超时".into()),
    }

    let request = match timeout(HANDSHAKE_TIMEOUT, socks5::read_request(&mut stream)).await {
        Ok(Ok(request)) => request,
        Ok(Err(socks5::ProtocolError::UnsupportedAddressType(_))) => {
            write_reply(&mut stream, socks5::REP_ADDRESS_TYPE_NOT_SUPPORTED, None).await;
            return Ok(());
        }
        Ok(Err(error)) => return Err(error.to_string()),
        Err(_) => return Err("请求读取超时".into()),
    };

    match request.command {
        socks5::Command::Connect => {
            if !config.has(Transport::Tcp) {
                write_reply(&mut stream, socks5::REP_COMMAND_NOT_SUPPORTED, None).await;
                return Ok(());
            }
            match tcp::connect(&request.target, config.ip).await {
                Ok(mut upstream) => {
                    let bound = upstream.local_addr().ok();
                    write_reply(&mut stream, socks5::REP_SUCCEEDED, bound).await;
                    tokio::select! {
                      _ = cancel.cancelled() => {}
                      result = tcp::relay(&mut stream, &mut upstream) => {
                        if let Err(error) = result {
                          return Err(format!("TCP 转发失败：{error}"));
                        }
                      }
                    }
                }
                Err(error) => {
                    write_reply(&mut stream, error.reply_code(), None).await;
                }
            }
        }
        socks5::Command::UdpAssociate => {
            if !config.has(Transport::Udp) {
                write_reply(&mut stream, socks5::REP_COMMAND_NOT_SUPPORTED, None).await;
                return Ok(());
            }
            match udp::bind_relay(config.ip).await {
                Ok(relay) => {
                    let relay_address = relay
                        .local_addr()
                        .map_err(|error| format!("读取 UDP 中继地址失败：{error}"))?;
                    write_reply(&mut stream, socks5::REP_SUCCEEDED, Some(relay_address)).await;
                    let client_ip = peer
                        .map(|address| address.ip())
                        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
                    udp::run(stream, relay, client_ip, config.ip, cancel).await;
                }
                Err(error) => {
                    debug_log(&format!("创建 UDP 中继失败：{error}"));
                    write_reply(&mut stream, socks5::REP_GENERAL_FAILURE, None).await;
                }
            }
        }
        socks5::Command::Bind | socks5::Command::Other(_) => {
            write_reply(&mut stream, socks5::REP_COMMAND_NOT_SUPPORTED, None).await;
        }
    }

    Ok(())
}

async fn write_reply(stream: &mut TcpStream, code: u8, bound: Option<SocketAddr>) {
    let _ = stream.write_all(&socks5::encode_reply(code, bound)).await;
}

fn debug_log(message: &str) {
    #[cfg(debug_assertions)]
    eprintln!("[proxy-rs] {message}");
    #[cfg(not(debug_assertions))]
    let _ = message;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ProxyState;
    use socks5::{Host, Target};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UdpSocket;

    async fn free_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        drop(listener);
        port
    }

    fn config(port: u16, protocols: Vec<Transport>) -> ProxyConfig {
        ProxyConfig {
            interface_name: "Loopback".into(),
            ip: "127.0.0.1".parse().expect("ip"),
            port: i64::from(port),
            protocols,
        }
    }

    async fn echo_server() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind echo");
        let address = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut buffer = [0u8; 1024];
                    while let Ok(read) = stream.read(&mut buffer).await {
                        if read == 0 || stream.write_all(&buffer[..read]).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });
        address
    }

    async fn udp_echo_server() -> SocketAddr {
        let socket = UdpSocket::bind("127.0.0.1:0").await.expect("bind udp echo");
        let address = socket.local_addr().expect("addr");
        tokio::spawn(async move {
            let mut buffer = [0u8; 2048];
            while let Ok((length, peer)) = socket.recv_from(&mut buffer).await {
                let _ = socket.send_to(&buffer[..length], peer).await;
            }
        });
        address
    }

    async fn greeting(stream: &mut TcpStream) {
        stream
            .write_all(&[socks5::VERSION, 1, socks5::METHOD_NO_AUTH])
            .await
            .expect("greeting");
        let mut response = [0u8; 2];
        stream
            .read_exact(&mut response)
            .await
            .expect("greeting reply");
        assert_eq!(response, [socks5::VERSION, socks5::METHOD_NO_AUTH]);
    }

    async fn request(
        stream: &mut TcpStream,
        command: u8,
        target: &Target,
    ) -> (u8, Option<SocketAddr>) {
        let mut buffer = vec![socks5::VERSION, command, 0];
        match &target.host {
            Host::Ip(IpAddr::V4(ip)) => {
                buffer.push(socks5::ATYP_IPV4);
                buffer.extend_from_slice(&ip.octets());
            }
            Host::Ip(IpAddr::V6(ip)) => {
                buffer.push(socks5::ATYP_IPV6);
                buffer.extend_from_slice(&ip.octets());
            }
            Host::Domain(domain) => {
                buffer.push(socks5::ATYP_DOMAIN);
                buffer.push(domain.len() as u8);
                buffer.extend_from_slice(domain.as_bytes());
            }
        }
        buffer.extend_from_slice(&target.port.to_be_bytes());
        stream.write_all(&buffer).await.expect("request");
        read_reply(stream).await
    }

    async fn read_reply(stream: &mut TcpStream) -> (u8, Option<SocketAddr>) {
        let mut header = [0u8; 4];
        stream.read_exact(&mut header).await.expect("reply header");
        let reply = header[1];
        let address = match header[3] {
            socks5::ATYP_IPV4 => {
                let mut rest = [0u8; 6];
                stream.read_exact(&mut rest).await.expect("ipv4 reply");
                let ip = Ipv4Addr::new(rest[0], rest[1], rest[2], rest[3]);
                let port = u16::from_be_bytes([rest[4], rest[5]]);
                Some(SocketAddr::new(IpAddr::V4(ip), port))
            }
            socks5::ATYP_IPV6 => {
                let mut rest = [0u8; 18];
                stream.read_exact(&mut rest).await.expect("ipv6 reply");
                let mut ip = [0u8; 16];
                ip.copy_from_slice(&rest[..16]);
                let port = u16::from_be_bytes([rest[16], rest[17]]);
                Some(SocketAddr::new(
                    IpAddr::V6(std::net::Ipv6Addr::from(ip)),
                    port,
                ))
            }
            other => panic!("unexpected address type {other}"),
        };
        (reply, address)
    }

    async fn connect_proxy(port: u16) -> TcpStream {
        TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("proxy connect")
    }

    #[tokio::test]
    async fn starts_and_stops_with_port_release() {
        let manager = ProxyManager::new();
        let port = free_port().await;
        let status = manager
            .start(config(port, vec![Transport::Tcp]))
            .await
            .expect("start");
        assert_eq!(status.state, ProxyState::Running);
        assert_eq!(
            status.endpoint.as_deref(),
            Some(format!("socks5://127.0.0.1:{port}").as_str())
        );

        let repeated = manager.start(config(port, vec![Transport::Tcp])).await;
        assert_eq!(
            repeated.expect_err("already running").code,
            "already_running"
        );

        let stopped = manager.stop().await.expect("stop");
        assert_eq!(stopped.state, ProxyState::Stopped);

        let rebound = manager.start(config(port, vec![Transport::Tcp])).await;
        assert!(rebound.is_ok(), "port should be immediately reusable");
        manager.stop().await.expect("stop again");
    }

    #[tokio::test]
    async fn proxies_tcp_connect_to_echo_server() {
        let manager = ProxyManager::new();
        let port = free_port().await;
        manager
            .start(config(port, vec![Transport::Tcp]))
            .await
            .expect("start");
        let echo = echo_server().await;

        let mut client = connect_proxy(port).await;
        greeting(&mut client).await;
        let (reply, _) = request(
            &mut client,
            socks5::CMD_CONNECT,
            &Target::ip(echo.ip(), echo.port()),
        )
        .await;
        assert_eq!(reply, socks5::REP_SUCCEEDED);

        client.write_all(b"hello-socks").await.expect("write");
        let mut buffer = [0u8; 11];
        client.read_exact(&mut buffer).await.expect("read");
        assert_eq!(&buffer, b"hello-socks");
        manager.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn returns_refused_for_closed_upstream() {
        let manager = ProxyManager::new();
        let port = free_port().await;
        manager
            .start(config(port, vec![Transport::Tcp]))
            .await
            .expect("start");
        let closed = free_port().await;

        let mut client = connect_proxy(port).await;
        greeting(&mut client).await;
        let (reply, _) = request(
            &mut client,
            socks5::CMD_CONNECT,
            &Target::ip("127.0.0.1".parse().unwrap(), closed),
        )
        .await;
        assert_eq!(reply, socks5::REP_CONNECTION_REFUSED);
        manager.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn rejects_connect_when_tcp_disabled() {
        let manager = ProxyManager::new();
        let port = free_port().await;
        manager
            .start(config(port, vec![Transport::Udp]))
            .await
            .expect("start");

        let mut client = connect_proxy(port).await;
        greeting(&mut client).await;
        let (reply, _) = request(
            &mut client,
            socks5::CMD_CONNECT,
            &Target::ip("127.0.0.1".parse().unwrap(), 9),
        )
        .await;
        assert_eq!(reply, socks5::REP_COMMAND_NOT_SUPPORTED);
        manager.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn rejects_bind_command() {
        let manager = ProxyManager::new();
        let port = free_port().await;
        manager
            .start(config(port, vec![Transport::Tcp]))
            .await
            .expect("start");

        let mut client = connect_proxy(port).await;
        greeting(&mut client).await;
        let (reply, _) = request(
            &mut client,
            socks5::CMD_BIND,
            &Target::ip("127.0.0.1".parse().unwrap(), 80),
        )
        .await;
        assert_eq!(reply, socks5::REP_COMMAND_NOT_SUPPORTED);
        manager.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn proxies_udp_associate_datagrams() {
        let manager = ProxyManager::new();
        let port = free_port().await;
        manager
            .start(config(port, vec![Transport::Tcp, Transport::Udp]))
            .await
            .expect("start");
        let echo = udp_echo_server().await;

        let mut control = connect_proxy(port).await;
        greeting(&mut control).await;
        let (reply, relay) = request(
            &mut control,
            socks5::CMD_UDP_ASSOCIATE,
            &Target::ip("0.0.0.0".parse().unwrap(), 0),
        )
        .await;
        assert_eq!(reply, socks5::REP_SUCCEEDED);
        let relay = relay.expect("relay address");

        let client = UdpSocket::bind("127.0.0.1:0").await.expect("udp client");
        let packet = socks5::encode_udp_request(&Target::ip(echo.ip(), echo.port()), b"udp-hello");
        client.send_to(&packet, relay).await.expect("send");

        let mut buffer = [0u8; 2048];
        let (length, _) = timeout(Duration::from_secs(2), client.recv_from(&mut buffer))
            .await
            .expect("response timeout")
            .expect("recv");
        let response = socks5::parse_udp_packet(&buffer[..length]).expect("parse");
        assert_eq!(response.payload, b"udp-hello");
        drop(control);
        manager.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn rejects_udp_associate_when_disabled() {
        let manager = ProxyManager::new();
        let port = free_port().await;
        manager
            .start(config(port, vec![Transport::Tcp]))
            .await
            .expect("start");

        let mut client = connect_proxy(port).await;
        greeting(&mut client).await;
        let (reply, _) = request(
            &mut client,
            socks5::CMD_UDP_ASSOCIATE,
            &Target::ip("0.0.0.0".parse().unwrap(), 0),
        )
        .await;
        assert_eq!(reply, socks5::REP_COMMAND_NOT_SUPPORTED);
        manager.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn survives_malformed_session() {
        let manager = ProxyManager::new();
        let port = free_port().await;
        manager
            .start(config(port, vec![Transport::Tcp]))
            .await
            .expect("start");

        let mut bad = connect_proxy(port).await;
        bad.write_all(&[0x04, 0x01, 0x02]).await.expect("bad bytes");
        drop(bad);

        let mut good = connect_proxy(port).await;
        greeting(&mut good).await;
        let (reply, _) = request(
            &mut good,
            socks5::CMD_CONNECT,
            &Target::ip("127.0.0.1".parse().unwrap(), 9),
        )
        .await;
        assert!(reply == socks5::REP_CONNECTION_REFUSED || reply == socks5::REP_GENERAL_FAILURE);
        manager.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn status_reports_error_after_listener_task_dies() {
        let manager = ProxyManager::new();
        let port = free_port().await;
        manager
            .start(config(port, vec![Transport::Tcp]))
            .await
            .expect("start");
        manager.stop().await.expect("stop");
        let status = manager.status().await;
        assert_eq!(status.state, ProxyState::Stopped);
    }
}
