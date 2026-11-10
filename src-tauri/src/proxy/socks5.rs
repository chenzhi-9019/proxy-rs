//! RFC 1928 无认证 SOCKS5 协议编解码。
//!
//! 该模块只负责字节级协议，不接触真实网络，便于对握手、请求、回复与 UDP
//! 数据报格式做精确单测。

use std::fmt;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// SOCKS 版本号。
pub const VERSION: u8 = 0x05;

/// 无认证方法。
pub const METHOD_NO_AUTH: u8 = 0x00;

/// 无可接受的方法。
pub const METHOD_NONE_ACCEPTABLE: u8 = 0xff;

/// `CONNECT` 命令。
pub const CMD_CONNECT: u8 = 0x01;
/// `BIND` 命令（本实现不支持）。
pub const CMD_BIND: u8 = 0x02;
/// `UDP ASSOCIATE` 命令。
pub const CMD_UDP_ASSOCIATE: u8 = 0x03;

/// IPv4 地址类型。
pub const ATYP_IPV4: u8 = 0x01;
/// 域名地址类型。
pub const ATYP_DOMAIN: u8 = 0x03;
/// IPv6 地址类型。
pub const ATYP_IPV6: u8 = 0x04;

/// 成功。
pub const REP_SUCCEEDED: u8 = 0x00;
/// 一般失败。
pub const REP_GENERAL_FAILURE: u8 = 0x01;
/// 连接不被规则允许。
pub const REP_NOT_ALLOWED: u8 = 0x02;
/// 网络不可达。
pub const REP_NETWORK_UNREACHABLE: u8 = 0x03;
/// 主机不可达。
pub const REP_HOST_UNREACHABLE: u8 = 0x04;
/// 连接被拒绝。
pub const REP_CONNECTION_REFUSED: u8 = 0x05;
/// TTL 过期。
pub const REP_TTL_EXPIRED: u8 = 0x06;
/// 命令不支持。
pub const REP_COMMAND_NOT_SUPPORTED: u8 = 0x07;
/// 地址类型不支持。
pub const REP_ADDRESS_TYPE_NOT_SUPPORTED: u8 = 0x08;

/// SOCKS5 命令。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Connect,
    Bind,
    UdpAssociate,
    /// 未知命令，处理时回复 `0x07`。
    Other(u8),
}

impl Command {
    /// 从字节解析命令；未知值保留原始字节。
    pub const fn from_u8(value: u8) -> Self {
        match value {
            CMD_CONNECT => Command::Connect,
            CMD_BIND => Command::Bind,
            CMD_UDP_ASSOCIATE => Command::UdpAssociate,
            other => Command::Other(other),
        }
    }

    /// 命令对应的原始字节。
    pub const fn code(self) -> u8 {
        match self {
            Command::Connect => CMD_CONNECT,
            Command::Bind => CMD_BIND,
            Command::UdpAssociate => CMD_UDP_ASSOCIATE,
            Command::Other(value) => value,
        }
    }
}

/// 目标主机，可以是 IP 或域名。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Host {
    Ip(IpAddr),
    Domain(String),
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Host::Ip(ip) => write!(f, "{ip}"),
            Host::Domain(domain) => write!(f, "{domain}"),
        }
    }
}

/// 目标地址与端口。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub host: Host,
    pub port: u16,
}

impl Target {
    /// 构造 IP 目标。
    pub fn ip(ip: IpAddr, port: u16) -> Self {
        Self {
            host: Host::Ip(ip),
            port,
        }
    }

    /// 若目标是 IP，返回 `SocketAddr`。
    pub fn socket_addr(&self) -> Option<SocketAddr> {
        match self.host {
            Host::Ip(ip) => Some(SocketAddr::new(ip, self.port)),
            Host::Domain(_) => None,
        }
    }
}

/// 已解析的 SOCKS5 请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub command: Command,
    pub target: Target,
}

/// 请求/握手解析错误。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    #[error("不支持的 SOCKS 版本: {0}")]
    UnsupportedVersion(u8),
    #[error("认证方法列表为空")]
    EmptyMethods,
    #[error("没有可接受的认证方法")]
    NoAcceptableMethod,
    #[error("非法的请求格式")]
    InvalidRequest,
    #[error("保留字段非零")]
    InvalidReserved,
    #[error("地址类型不支持: {0}")]
    UnsupportedAddressType(u8),
    #[error("域名长度非法")]
    InvalidDomainLength,
    #[error("请求被截断")]
    Truncated,
    #[error("I/O 错误: {0}")]
    Io(String),
}

impl From<io::Error> for ProtocolError {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            ProtocolError::Truncated
        } else {
            ProtocolError::Io(error.to_string())
        }
    }
}

/// 读取客户端问候并完成方法协商。
///
/// 仅接受无认证；没有 `0x00` 时写入 `0xFF` 后返回
/// [`ProtocolError::NoAcceptableMethod`]，调用方应关闭连接。
pub async fn negotiate<R>(stream: &mut R) -> Result<(), ProtocolError>
where
    R: AsyncRead + AsyncWrite + Unpin,
{
    let mut header = [0u8; 2];
    stream.read_exact(&mut header).await?;
    if header[0] != VERSION {
        return Err(ProtocolError::UnsupportedVersion(header[0]));
    }

    let count = header[1] as usize;
    if count == 0 {
        return Err(ProtocolError::EmptyMethods);
    }

    let mut methods = vec![0u8; count];
    stream.read_exact(&mut methods).await?;

    if methods.contains(&METHOD_NO_AUTH) {
        stream
            .write_all(&[VERSION, METHOD_NO_AUTH])
            .await
            .map_err(ProtocolError::from)?;
        Ok(())
    } else {
        // 尽力告知客户端，即使写入失败也按“无方法可接受”处理。
        let _ = stream.write_all(&[VERSION, METHOD_NONE_ACCEPTABLE]).await;
        Err(ProtocolError::NoAcceptableMethod)
    }
}

/// 读取一个完整的 SOCKS5 请求。
pub async fn read_request<R>(stream: &mut R) -> Result<Request, ProtocolError>
where
    R: AsyncRead + Unpin,
{
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;

    if header[0] != VERSION {
        return Err(ProtocolError::UnsupportedVersion(header[0]));
    }
    if header[2] != 0 {
        return Err(ProtocolError::InvalidReserved);
    }

    let command = Command::from_u8(header[1]);
    let target = read_target(stream, header[3]).await?;
    Ok(Request { command, target })
}

async fn read_target<R>(stream: &mut R, address_type: u8) -> Result<Target, ProtocolError>
where
    R: AsyncRead + Unpin,
{
    let host = match address_type {
        ATYP_IPV4 => {
            let mut bytes = [0u8; 4];
            stream.read_exact(&mut bytes).await?;
            Host::Ip(IpAddr::V4(Ipv4Addr::from(bytes)))
        }
        ATYP_IPV6 => {
            let mut bytes = [0u8; 16];
            stream.read_exact(&mut bytes).await?;
            Host::Ip(IpAddr::V6(Ipv6Addr::from(bytes)))
        }
        ATYP_DOMAIN => {
            let mut length = [0u8; 1];
            stream.read_exact(&mut length).await?;
            if length[0] == 0 {
                return Err(ProtocolError::InvalidDomainLength);
            }
            let mut bytes = vec![0u8; length[0] as usize];
            stream.read_exact(&mut bytes).await?;
            Host::Domain(String::from_utf8_lossy(&bytes).into_owned())
        }
        other => return Err(ProtocolError::UnsupportedAddressType(other)),
    };

    let mut port = [0u8; 2];
    stream.read_exact(&mut port).await?;
    Ok(Target {
        host,
        port: u16::from_be_bytes(port),
    })
}

/// 编码服务器回复。`bound` 为 `None` 时使用 `0.0.0.0:0`。
pub fn encode_reply(reply: u8, bound: Option<SocketAddr>) -> Vec<u8> {
    let mut buffer = vec![VERSION, reply, 0];
    match bound {
        Some(SocketAddr::V4(address)) => {
            buffer.push(ATYP_IPV4);
            buffer.extend_from_slice(&address.ip().octets());
            buffer.extend_from_slice(&address.port().to_be_bytes());
        }
        Some(SocketAddr::V6(address)) => {
            buffer.push(ATYP_IPV6);
            buffer.extend_from_slice(&address.ip().octets());
            buffer.extend_from_slice(&address.port().to_be_bytes());
        }
        None => {
            buffer.push(ATYP_IPV4);
            buffer.extend_from_slice(&[0, 0, 0, 0]);
            buffer.extend_from_slice(&[0, 0]);
        }
    }
    buffer
}

/// 把 I/O 错误映射为 SOCKS5 回复码。
pub fn reply_for_io_error(error: &io::Error) -> u8 {
    match error.kind() {
        io::ErrorKind::ConnectionRefused => REP_CONNECTION_REFUSED,
        io::ErrorKind::ConnectionAborted | io::ErrorKind::ConnectionReset => REP_CONNECTION_REFUSED,
        io::ErrorKind::NetworkUnreachable => REP_NETWORK_UNREACHABLE,
        io::ErrorKind::HostUnreachable | io::ErrorKind::AddrNotAvailable => REP_HOST_UNREACHABLE,
        io::ErrorKind::TimedOut => REP_TTL_EXPIRED,
        io::ErrorKind::PermissionDenied => REP_NOT_ALLOWED,
        _ => REP_GENERAL_FAILURE,
    }
}

/// 解析后的 UDP 数据报。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UdpPacket {
    pub target: Target,
    pub payload: Vec<u8>,
}

/// UDP 数据报解析错误。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UdpError {
    #[error("数据报过短")]
    TooShort,
    #[error("保留字段非零")]
    InvalidReserved,
    #[error("分片数据报被丢弃")]
    Fragmented,
    #[error("地址类型不支持: {0}")]
    UnsupportedAddressType(u8),
    #[error("域名长度非法")]
    InvalidDomainLength,
    #[error("数据报被截断")]
    Truncated,
}

/// 按 SOCKS5 UDP 请求格式解析数据报。
///
/// `FRAG != 0` 返回 [`UdpError::Fragmented`]，调用方应静默丢弃。
pub fn parse_udp_packet(buffer: &[u8]) -> Result<UdpPacket, UdpError> {
    if buffer.len() < 4 {
        return Err(UdpError::TooShort);
    }
    if buffer[0] != 0 || buffer[1] != 0 {
        return Err(UdpError::InvalidReserved);
    }
    if buffer[2] != 0 {
        return Err(UdpError::Fragmented);
    }

    let address_type = buffer[3];
    let (host, offset) = parse_udp_host(buffer, 4, address_type)?;
    if buffer.len() < offset + 2 {
        return Err(UdpError::Truncated);
    }
    let port = u16::from_be_bytes([buffer[offset], buffer[offset + 1]]);
    Ok(UdpPacket {
        target: Target { host, port },
        payload: buffer[offset + 2..].to_vec(),
    })
}

fn parse_udp_host(
    buffer: &[u8],
    start: usize,
    address_type: u8,
) -> Result<(Host, usize), UdpError> {
    match address_type {
        ATYP_IPV4 => {
            let end = start + 4;
            if buffer.len() < end {
                return Err(UdpError::Truncated);
            }
            let bytes: [u8; 4] = buffer[start..end].try_into().expect("length checked");
            Ok((Host::Ip(IpAddr::V4(Ipv4Addr::from(bytes))), end))
        }
        ATYP_IPV6 => {
            let end = start + 16;
            if buffer.len() < end {
                return Err(UdpError::Truncated);
            }
            let bytes: [u8; 16] = buffer[start..end].try_into().expect("length checked");
            Ok((Host::Ip(IpAddr::V6(Ipv6Addr::from(bytes))), end))
        }
        ATYP_DOMAIN => {
            if buffer.len() <= start {
                return Err(UdpError::Truncated);
            }
            let length = buffer[start] as usize;
            if length == 0 {
                return Err(UdpError::InvalidDomainLength);
            }
            let end = start + 1 + length;
            if buffer.len() < end {
                return Err(UdpError::Truncated);
            }
            let domain = String::from_utf8_lossy(&buffer[start + 1..end]).into_owned();
            Ok((Host::Domain(domain), end))
        }
        other => Err(UdpError::UnsupportedAddressType(other)),
    }
}

/// 编码返回给客户端的 UDP 数据报（来源地址头 + 负载）。
pub fn encode_udp_response(source: SocketAddr, payload: &[u8]) -> Vec<u8> {
    let mut buffer = vec![0, 0, 0];
    match source {
        SocketAddr::V4(address) => {
            buffer.push(ATYP_IPV4);
            buffer.extend_from_slice(&address.ip().octets());
            buffer.extend_from_slice(&address.port().to_be_bytes());
        }
        SocketAddr::V6(address) => {
            buffer.push(ATYP_IPV6);
            buffer.extend_from_slice(&address.ip().octets());
            buffer.extend_from_slice(&address.port().to_be_bytes());
        }
    }
    buffer.extend_from_slice(payload);
    buffer
}

/// 编码客户端方向的 UDP 数据报，供集成测试与调试使用。
pub fn encode_udp_request(target: &Target, payload: &[u8]) -> Vec<u8> {
    let mut buffer = vec![0, 0, 0];
    match &target.host {
        Host::Ip(IpAddr::V4(ip)) => {
            buffer.push(ATYP_IPV4);
            buffer.extend_from_slice(&ip.octets());
        }
        Host::Ip(IpAddr::V6(ip)) => {
            buffer.push(ATYP_IPV6);
            buffer.extend_from_slice(&ip.octets());
        }
        Host::Domain(domain) => {
            let bytes = domain.as_bytes();
            let length = bytes.len().min(u8::MAX as usize);
            buffer.push(ATYP_DOMAIN);
            buffer.push(length as u8);
            buffer.extend_from_slice(&bytes[..length]);
        }
    }
    buffer.extend_from_slice(&target.port.to_be_bytes());
    buffer.extend_from_slice(payload);
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn request_from(bytes: &[u8]) -> Result<Request, ProtocolError> {
        let (mut client, mut server) = tokio::io::duplex(512);
        client.write_all(bytes).await.expect("write request");
        drop(client);
        read_request(&mut server).await
    }

    async fn negotiate_with(bytes: &[u8]) -> (Result<(), ProtocolError>, Vec<u8>) {
        let (mut client, mut server) = tokio::io::duplex(64);
        client.write_all(bytes).await.expect("write greeting");
        let result = negotiate(&mut server).await;
        drop(server);
        let mut response = Vec::new();
        client
            .read_to_end(&mut response)
            .await
            .expect("read response");
        (result, response)
    }

    #[test]
    fn command_codes_round_trip() {
        assert_eq!(Command::from_u8(CMD_CONNECT), Command::Connect);
        assert_eq!(Command::from_u8(CMD_BIND), Command::Bind);
        assert_eq!(Command::from_u8(CMD_UDP_ASSOCIATE), Command::UdpAssociate);
        assert_eq!(Command::from_u8(0x09), Command::Other(0x09));
        assert_eq!(Command::Other(0x09).code(), 0x09);
    }

    #[tokio::test]
    async fn accepts_no_auth_greeting() {
        let (result, response) = negotiate_with(&[VERSION, 2, METHOD_NO_AUTH, 0x02]).await;
        assert!(result.is_ok());
        assert_eq!(response, vec![VERSION, METHOD_NO_AUTH]);
    }

    #[tokio::test]
    async fn rejects_greeting_without_no_auth() {
        let (result, response) = negotiate_with(&[VERSION, 1, 0x02]).await;
        assert_eq!(result, Err(ProtocolError::NoAcceptableMethod));
        assert_eq!(response, vec![VERSION, METHOD_NONE_ACCEPTABLE]);
    }

    #[tokio::test]
    async fn rejects_invalid_version() {
        let (result, _) = negotiate_with(&[0x04, 1, METHOD_NO_AUTH]).await;
        assert_eq!(result, Err(ProtocolError::UnsupportedVersion(4)));
    }

    #[tokio::test]
    async fn rejects_empty_methods() {
        let (result, _) = negotiate_with(&[VERSION, 0]).await;
        assert_eq!(result, Err(ProtocolError::EmptyMethods));
    }

    #[tokio::test]
    async fn parses_ipv4_connect_request() {
        let request = request_from(&[VERSION, CMD_CONNECT, 0, ATYP_IPV4, 127, 0, 0, 1, 0x1f, 0x90])
            .await
            .expect("request");
        assert_eq!(request.command, Command::Connect);
        assert_eq!(
            request.target,
            Target::ip("127.0.0.1".parse().unwrap(), 8080)
        );
    }

    #[tokio::test]
    async fn parses_ipv6_connect_request() {
        let mut bytes = vec![VERSION, CMD_CONNECT, 0, ATYP_IPV6];
        bytes.extend_from_slice(&Ipv6Addr::LOCALHOST.octets());
        bytes.extend_from_slice(&1080u16.to_be_bytes());
        let request = request_from(&bytes).await.expect("request");
        assert_eq!(
            request.target,
            Target::ip(IpAddr::V6(Ipv6Addr::LOCALHOST), 1080)
        );
    }

    #[tokio::test]
    async fn parses_domain_request() {
        let mut bytes = vec![VERSION, CMD_CONNECT, 0, ATYP_DOMAIN, 11];
        bytes.extend_from_slice(b"example.com");
        bytes.extend_from_slice(&443u16.to_be_bytes());
        let request = request_from(&bytes).await.expect("request");
        assert_eq!(
            request.target,
            Target {
                host: Host::Domain("example.com".into()),
                port: 443
            }
        );
    }

    #[tokio::test]
    async fn rejects_unknown_command_but_parses_target() {
        let request = request_from(&[VERSION, 0x09, 0, ATYP_IPV4, 10, 0, 0, 1, 0, 80])
            .await
            .expect("request");
        assert_eq!(request.command, Command::Other(0x09));
    }

    #[tokio::test]
    async fn rejects_invalid_address_type() {
        let result = request_from(&[VERSION, CMD_CONNECT, 0, 0x05, 0, 0]).await;
        assert_eq!(result, Err(ProtocolError::UnsupportedAddressType(5)));
    }

    #[tokio::test]
    async fn rejects_reserved_field() {
        let result = request_from(&[VERSION, CMD_CONNECT, 1, ATYP_IPV4]).await;
        assert_eq!(result, Err(ProtocolError::InvalidReserved));
    }

    #[tokio::test]
    async fn rejects_empty_domain() {
        let result = request_from(&[VERSION, CMD_CONNECT, 0, ATYP_DOMAIN, 0]).await;
        assert_eq!(result, Err(ProtocolError::InvalidDomainLength));
    }

    #[tokio::test]
    async fn rejects_truncated_request() {
        let result = request_from(&[VERSION, CMD_CONNECT, 0, ATYP_IPV4, 10, 0]).await;
        assert_eq!(result, Err(ProtocolError::Truncated));
    }

    #[test]
    fn encodes_ipv4_reply() {
        let reply = encode_reply(REP_SUCCEEDED, Some("127.0.0.1:1080".parse().unwrap()));
        assert_eq!(
            reply,
            vec![VERSION, 0, 0, ATYP_IPV4, 127, 0, 0, 1, 0x04, 0x38]
        );
    }

    #[test]
    fn encodes_ipv6_reply() {
        let reply = encode_reply(
            REP_SUCCEEDED,
            Some(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 1080)),
        );
        let mut expected = vec![VERSION, 0, 0, ATYP_IPV6];
        expected.extend_from_slice(&Ipv6Addr::LOCALHOST.octets());
        expected.extend_from_slice(&1080u16.to_be_bytes());
        assert_eq!(reply, expected);
    }

    #[test]
    fn encodes_error_reply_with_unspecified_address() {
        let reply = encode_reply(REP_COMMAND_NOT_SUPPORTED, None);
        assert_eq!(reply, vec![VERSION, 7, 0, ATYP_IPV4, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn maps_io_errors_to_reply_codes() {
        let io_error = |kind| io::Error::new(kind, "test");
        assert_eq!(
            reply_for_io_error(&io_error(io::ErrorKind::ConnectionRefused)),
            REP_CONNECTION_REFUSED
        );
        assert_eq!(
            reply_for_io_error(&io_error(io::ErrorKind::NetworkUnreachable)),
            REP_NETWORK_UNREACHABLE
        );
        assert_eq!(
            reply_for_io_error(&io_error(io::ErrorKind::HostUnreachable)),
            REP_HOST_UNREACHABLE
        );
        assert_eq!(
            reply_for_io_error(&io_error(io::ErrorKind::TimedOut)),
            REP_TTL_EXPIRED
        );
        assert_eq!(
            reply_for_io_error(&io_error(io::ErrorKind::Other)),
            REP_GENERAL_FAILURE
        );
    }

    #[test]
    fn parses_udp_ipv4_packet() {
        let mut bytes = vec![0, 0, 0, ATYP_IPV4, 8, 8, 8, 8, 0x00, 0x35];
        bytes.extend_from_slice(b"dns");
        let packet = parse_udp_packet(&bytes).expect("udp packet");
        assert_eq!(packet.target, Target::ip("8.8.8.8".parse().unwrap(), 53));
        assert_eq!(packet.payload, b"dns");
    }

    #[test]
    fn parses_udp_ipv6_packet() {
        let mut bytes = vec![0, 0, 0, ATYP_IPV6];
        bytes.extend_from_slice(&Ipv6Addr::LOCALHOST.octets());
        bytes.extend_from_slice(&53u16.to_be_bytes());
        let packet = parse_udp_packet(&bytes).expect("udp packet");
        assert_eq!(
            packet.target,
            Target::ip(IpAddr::V6(Ipv6Addr::LOCALHOST), 53)
        );
        assert!(packet.payload.is_empty());
    }

    #[test]
    fn parses_udp_domain_packet() {
        let mut bytes = vec![0, 0, 0, ATYP_DOMAIN, 9];
        bytes.extend_from_slice(b"localhost");
        bytes.extend_from_slice(&9u16.to_be_bytes());
        bytes.extend_from_slice(b"payload");
        let packet = parse_udp_packet(&bytes).expect("udp packet");
        assert_eq!(
            packet.target,
            Target {
                host: Host::Domain("localhost".into()),
                port: 9
            }
        );
        assert_eq!(packet.payload, b"payload");
    }

    #[test]
    fn drops_fragmented_udp_packet() {
        let bytes = [0, 0, 1, ATYP_IPV4, 127, 0, 0, 1, 0, 53, 1, 2, 3];
        assert_eq!(parse_udp_packet(&bytes), Err(UdpError::Fragmented));
    }

    #[test]
    fn rejects_bad_udp_headers() {
        assert_eq!(parse_udp_packet(&[0, 0, 0]), Err(UdpError::TooShort));
        assert_eq!(
            parse_udp_packet(&[1, 0, 0, ATYP_IPV4, 0, 0, 0, 0, 0, 0]),
            Err(UdpError::InvalidReserved)
        );
        assert_eq!(
            parse_udp_packet(&[0, 0, 0, 0x09, 0, 0, 0, 0]),
            Err(UdpError::UnsupportedAddressType(9))
        );
        assert_eq!(
            parse_udp_packet(&[0, 0, 0, ATYP_DOMAIN, 0]),
            Err(UdpError::InvalidDomainLength)
        );
        assert_eq!(
            parse_udp_packet(&[0, 0, 0, ATYP_IPV4, 1, 2, 3]),
            Err(UdpError::Truncated)
        );
    }

    #[test]
    fn encodes_udp_request_and_response_round_trip() {
        let target = Target::ip("127.0.0.1".parse().unwrap(), 5353);
        let request = encode_udp_request(&target, b"ping");
        let decoded = parse_udp_packet(&request).expect("decode request");
        assert_eq!(decoded.target, target);
        assert_eq!(decoded.payload, b"ping");

        let response = encode_udp_response("127.0.0.1:5353".parse().unwrap(), b"pong");
        let decoded = parse_udp_packet(&response).expect("decode response");
        assert_eq!(decoded.target.port, 5353);
        assert_eq!(decoded.payload, b"pong");
    }

    #[test]
    fn encodes_domain_udp_request() {
        let target = Target {
            host: Host::Domain("example.com".into()),
            port: 53,
        };
        let request = encode_udp_request(&target, b"");
        assert_eq!(&request[..4], &[0, 0, 0, ATYP_DOMAIN]);
        let decoded = parse_udp_packet(&request).expect("decode");
        assert_eq!(decoded.target, target);
        assert!(decoded.payload.is_empty());
    }
}
