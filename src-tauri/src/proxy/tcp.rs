//! TCP `CONNECT` 的上游连接与双向转发。

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpSocket, TcpStream};
use tokio::time::timeout;

use super::socks5::{self, Host, Target};

/// 建立上游连接的超时时间。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// 上游连接失败原因。
#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("域名解析失败")]
    Resolve,
    #[error("目标地址列表为空")]
    NoAddress,
    #[error("连接超时")]
    Timeout,
    #[error("连接失败: {0}")]
    Io(io::Error),
}

impl ConnectError {
    /// 映射为 SOCKS5 回复码。
    pub fn reply_code(&self) -> u8 {
        match self {
            ConnectError::Resolve | ConnectError::NoAddress => socks5::REP_HOST_UNREACHABLE,
            ConnectError::Timeout => socks5::REP_TTL_EXPIRED,
            ConnectError::Io(error) => socks5::reply_for_io_error(error),
        }
    }
}

/// 连接目标；域名会解析后逐个尝试。
///
/// 当目标地址族与 `source_ip` 一致且源地址不是回环时，套接字会绑定该源 IP；
/// 绑定失败则回退到系统默认路由，不拒绝请求。
pub async fn connect(target: &Target, source_ip: IpAddr) -> Result<TcpStream, ConnectError> {
    let addresses: Vec<SocketAddr> = match &target.host {
        Host::Ip(ip) => vec![SocketAddr::new(*ip, target.port)],
        Host::Domain(domain) => {
            let lookup = timeout(
                CONNECT_TIMEOUT,
                tokio::net::lookup_host((domain.as_str(), target.port)),
            )
            .await;
            match lookup {
                Ok(Ok(addresses)) => addresses.collect(),
                Ok(Err(_)) | Err(_) => return Err(ConnectError::Resolve),
            }
        }
    };

    if addresses.is_empty() {
        return Err(ConnectError::NoAddress);
    }

    let mut last_error = None;
    for address in addresses {
        match connect_one(address, source_ip).await {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or(ConnectError::NoAddress))
}

async fn connect_one(address: SocketAddr, source_ip: IpAddr) -> Result<TcpStream, ConnectError> {
    let socket = new_socket(address, source_ip)?;
    match timeout(CONNECT_TIMEOUT, socket.connect(address)).await {
        Ok(Ok(stream)) => {
            let _ = stream.set_nodelay(true);
            Ok(stream)
        }
        Ok(Err(error)) => Err(ConnectError::Io(error)),
        Err(_) => Err(ConnectError::Timeout),
    }
}

/// 创建与目标地址族匹配的套接字，并按需绑定源地址。
fn new_socket(address: SocketAddr, source_ip: IpAddr) -> Result<TcpSocket, ConnectError> {
    let create = || {
        if address.is_ipv4() {
            TcpSocket::new_v4()
        } else {
            TcpSocket::new_v6()
        }
    };

    let socket = create().map_err(ConnectError::Io)?;
    if source_ip.is_ipv4() == address.is_ipv4()
        && !source_ip.is_loopback()
        && !source_ip.is_unspecified()
        && let Err(error) = socket.bind(SocketAddr::new(source_ip, 0))
    {
        eprintln!("[proxy-rs] 绑定源地址 {source_ip} 失败，回退默认路由：{error}");
        return create().map_err(ConnectError::Io);
    }
    Ok(socket)
}

/// 双向转发，任一方向 EOF 后正确半关闭。
pub async fn relay<Inbound, Outbound>(
    inbound: &mut Inbound,
    outbound: &mut Outbound,
) -> io::Result<()>
where
    Inbound: AsyncRead + AsyncWrite + Unpin,
    Outbound: AsyncRead + AsyncWrite + Unpin,
{
    tokio::io::copy_bidirectional(inbound, outbound).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn echo_server() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind echo");
        let address = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut buffer = [0u8; 1024];
                    loop {
                        match stream.read(&mut buffer).await {
                            Ok(0) | Err(_) => break,
                            Ok(read) => {
                                if stream.write_all(&buffer[..read]).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });
            }
        });
        address
    }

    async fn closed_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        drop(listener);
        port
    }

    #[tokio::test]
    async fn connects_to_ipv4_target() {
        let address = echo_server().await;
        let target = Target::ip(address.ip(), address.port());
        let mut stream = connect(&target, "127.0.0.1".parse().unwrap())
            .await
            .expect("connect");
        stream.write_all(b"ping").await.expect("write");
        let mut buffer = [0u8; 4];
        stream.read_exact(&mut buffer).await.expect("read");
        assert_eq!(&buffer, b"ping");
    }

    #[tokio::test]
    async fn resolves_domain_target() {
        let address = echo_server().await;
        let target = Target {
            host: Host::Domain("localhost".into()),
            port: address.port(),
        };
        let mut stream = connect(&target, "127.0.0.1".parse().unwrap())
            .await
            .expect("connect");
        stream.write_all(b"ok").await.expect("write");
        let mut buffer = [0u8; 2];
        stream.read_exact(&mut buffer).await.expect("read");
        assert_eq!(&buffer, b"ok");
    }

    #[tokio::test]
    async fn reports_connection_refused() {
        let port = closed_port().await;
        let target = Target::ip("127.0.0.1".parse().unwrap(), port);
        let error = connect(&target, "127.0.0.1".parse().unwrap())
            .await
            .expect_err("refused");
        assert_eq!(error.reply_code(), socks5::REP_CONNECTION_REFUSED);
    }

    #[tokio::test]
    async fn relay_transfers_both_directions() {
        let (mut client, mut proxy) = tokio::io::duplex(1024);
        let echo = echo_server().await;
        let mut upstream = TcpStream::connect(echo).await.expect("upstream");

        let relay = tokio::spawn(async move {
            relay(&mut proxy, &mut upstream).await.expect("relay");
        });

        client.write_all(b"hello-relay").await.expect("write");
        let mut buffer = [0u8; 11];
        client.read_exact(&mut buffer).await.expect("read");
        assert_eq!(&buffer, b"hello-relay");

        drop(client);
        relay.await.expect("join");
    }
}
