//! 代理配置、状态与网卡地址的数据模型。
//!
//! 所有跨 IPC 边界传输的结构体统一使用 camelCase，与前端 `src/types.ts` 对齐。

use std::net::IpAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// 代理支持的传输协议。序列化为小写字符串，与前端 `'tcp' | 'udp'` 对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Tcp,
    Udp,
}

impl Transport {
    /// 传输协议的固定展示顺序：TCP 在前，UDP 在后。
    pub const ORDER: [Transport; 2] = [Transport::Tcp, Transport::Udp];

    /// 返回小写名称，便于日志与去重。
    pub const fn as_str(self) -> &'static str {
        match self {
            Transport::Tcp => "tcp",
            Transport::Udp => "udp",
        }
    }
}

/// IP 地址族，序列化为 `ipv4` / `ipv6`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

impl AddressFamily {
    /// 根据地址推断地址族。
    pub const fn of(ip: IpAddr) -> Self {
        match ip {
            IpAddr::V4(_) => AddressFamily::Ipv4,
            IpAddr::V6(_) => AddressFamily::Ipv6,
        }
    }
}

/// 一个可选择的“网卡 + IP”端点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceAddress {
    /// 稳定标识，格式为 `"{网卡名}|{IP}"`。
    pub id: String,
    /// 网卡展示名。
    pub name: String,
    /// 网卡上的 IP 地址。
    pub ip: IpAddr,
    /// 地址族。
    pub family: AddressFamily,
    /// 是否为回环地址。
    pub is_loopback: bool,
}

/// 启动代理所需的完整配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfig {
    /// 所选网卡展示名，仅用于错误信息与状态回显，绑定以 `ip` 为准。
    pub interface_name: String,
    /// 监听地址。
    pub ip: IpAddr,
    /// 监听端口，使用 i64 以便对负数、越界值给出字段级错误。
    pub port: i64,
    /// 启用的传输协议，至少一个。
    pub protocols: Vec<Transport>,
}

impl ProxyConfig {
    /// 校验与网卡无关的字段：端口范围、至少一种协议、地址是否为可绑定单播地址。
    ///
    /// 地址是否属于本机网卡由 [`ProxyConfig::ensure_local`] 检查。
    pub fn validate(&self) -> Result<(), AppError> {
        if !(1..=65535).contains(&self.port) {
            return Err(AppError::invalid_port(self.port));
        }
        if self.protocols.is_empty() {
            return Err(AppError::no_transport());
        }
        if self.ip.is_multicast() || (self.ip.is_unspecified() && !is_wildcard(self.ip)) {
            return Err(AppError::invalid_ip(self.ip));
        }
        Ok(())
    }

    /// 确认 `ip` 仍属于当前枚举到的本机网卡，或为通用的 `0.0.0.0`。
    pub fn ensure_local(&self, interfaces: &[InterfaceAddress]) -> Result<(), AppError> {
        if is_wildcard(self.ip) || interfaces.iter().any(|item| item.ip == self.ip) {
            Ok(())
        } else {
            Err(AppError::invalid_ip(self.ip))
        }
    }

    /// 校验通过后的端口。
    pub fn port_u16(&self) -> u16 {
        self.port as u16
    }

    /// 是否启用了某传输协议。
    pub fn has(&self, transport: Transport) -> bool {
        self.protocols.contains(&transport)
    }

    /// 返回按固定顺序排列、去重后的副本。
    pub fn normalized(&self) -> Self {
        let mut protocols: Vec<Transport> = Transport::ORDER
            .into_iter()
            .filter(|transport| self.protocols.contains(transport))
            .collect();
        protocols.dedup();
        Self {
            interface_name: self.interface_name.clone(),
            ip: self.ip,
            port: self.port,
            protocols,
        }
    }

    /// SOCKS5 端点展示文本；IPv6 地址使用方括号。
    pub fn endpoint(&self) -> String {
        endpoint_for(self.ip, self.port_u16())
    }
}

/// 是否是“所有网卡”的通用绑定地址 `0.0.0.0`。
pub fn is_wildcard(ip: IpAddr) -> bool {
    matches!(ip, IpAddr::V4(value) if value.is_unspecified())
}

/// 格式化 `socks5://host:port`，IPv6 自动加方括号。
pub fn endpoint_for(ip: IpAddr, port: u16) -> String {
    match ip {
        IpAddr::V4(ip) => format!("socks5://{ip}:{port}"),
        IpAddr::V6(ip) => format!("socks5://[{ip}]:{port}"),
    }
}

/// 代理运行状态。`state` 只会是 `running` 或 `stopped`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyState {
    Stopped,
    Running,
}

/// 返回给前端的完整代理状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStatus {
    pub state: ProxyState,
    pub endpoint: Option<String>,
    pub config: Option<ProxyConfig>,
    /// 启动时间（Unix epoch 毫秒）。
    pub started_at: Option<u64>,
    /// 异常停止时给用户看的原因。
    pub message: Option<String>,
}

impl ProxyStatus {
    /// 未运行状态。
    pub fn stopped(message: Option<String>) -> Self {
        Self {
            state: ProxyState::Stopped,
            endpoint: None,
            config: None,
            started_at: None,
            message,
        }
    }

    /// 运行中状态。
    pub fn running(config: ProxyConfig, endpoint: String, started_at: u64) -> Self {
        Self {
            state: ProxyState::Running,
            endpoint: Some(endpoint),
            config: Some(config),
            started_at: Some(started_at),
            message: None,
        }
    }
}

/// 当前 Unix 时间戳（毫秒）。
pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> ProxyConfig {
        ProxyConfig {
            interface_name: "以太网".into(),
            ip: "127.0.0.1".parse().expect("ip"),
            port: 1080,
            protocols: vec![Transport::Tcp],
        }
    }

    #[test]
    fn accepts_valid_config() {
        assert!(config().validate().is_ok());
    }

    #[test]
    fn rejects_invalid_ports() {
        for port in [0, -1, 65536, i64::MAX] {
            let mut cfg = config();
            cfg.port = port;
            assert_eq!(
                cfg.validate().expect_err("port").code,
                "invalid_port",
                "port {port}"
            );
        }
    }

    #[test]
    fn rejects_missing_transport() {
        let mut cfg = config();
        cfg.protocols.clear();
        assert_eq!(cfg.validate().expect_err("protocols").code, "no_transport");
    }

    #[test]
    fn rejects_invalid_addresses() {
        let mut cfg = config();
        cfg.ip = "::".parse().unwrap();
        assert_eq!(
            cfg.validate().expect_err("unspecified v6").code,
            "invalid_ip"
        );
        cfg.ip = "224.0.0.1".parse().unwrap();
        assert_eq!(cfg.validate().expect_err("multicast").code, "invalid_ip");
    }

    #[test]
    fn accepts_wildcard_address() {
        let mut cfg = config();
        cfg.ip = "0.0.0.0".parse().unwrap();
        assert!(cfg.validate().is_ok());
        assert!(cfg.ensure_local(&[]).is_ok());
        assert_eq!(cfg.endpoint(), "socks5://0.0.0.0:1080");
        assert!(is_wildcard(cfg.ip));
    }

    #[test]
    fn formats_ipv4_endpoint() {
        assert_eq!(config().endpoint(), "socks5://127.0.0.1:1080");
    }

    #[test]
    fn formats_ipv6_endpoint_with_brackets() {
        let mut cfg = config();
        cfg.ip = "::1".parse().unwrap();
        assert_eq!(cfg.endpoint(), "socks5://[::1]:1080");
    }

    #[test]
    fn checks_local_interface() {
        let interfaces = vec![InterfaceAddress {
            id: "以太网|127.0.0.1".into(),
            name: "以太网".into(),
            ip: "127.0.0.1".parse().unwrap(),
            family: AddressFamily::Ipv4,
            is_loopback: true,
        }];
        assert!(config().ensure_local(&interfaces).is_ok());
        let mut cfg = config();
        cfg.ip = "10.0.0.1".parse().unwrap();
        assert_eq!(
            cfg.ensure_local(&interfaces).expect_err("foreign").code,
            "invalid_ip"
        );
    }

    #[test]
    fn normalizes_protocol_order_and_duplicates() {
        let mut cfg = config();
        cfg.protocols = vec![Transport::Udp, Transport::Tcp, Transport::Udp];
        assert_eq!(
            cfg.normalized().protocols,
            vec![Transport::Tcp, Transport::Udp]
        );
    }

    #[test]
    fn serializes_camel_case_and_lowercase_state() {
        let status = ProxyStatus::running(config(), "socks5://127.0.0.1:1080".into(), 42);
        let value = serde_json::to_value(status).expect("serialize");
        assert_eq!(value["state"], "running");
        assert_eq!(value["startedAt"], 42);
        assert_eq!(value["config"]["interfaceName"], "以太网");
        assert_eq!(value["config"]["protocols"][0], "tcp");
    }

    #[test]
    fn stopped_status_is_empty() {
        let status = ProxyStatus::stopped(None);
        assert_eq!(status.state, ProxyState::Stopped);
        assert!(status.endpoint.is_none());
        assert!(status.config.is_none());
        assert!(status.started_at.is_none());
    }
}
