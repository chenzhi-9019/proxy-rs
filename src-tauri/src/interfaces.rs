//! 本机网卡枚举：过滤不可绑定地址、去重并提供稳定排序。

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use if_addrs::{Interface, get_if_addrs};

use crate::error::AppError;
use crate::model::{AddressFamily, InterfaceAddress};

/// 枚举本机所有可绑定的网卡地址。
///
/// 过滤规则：
/// - 跳过未指定地址（`0.0.0.0` / `::`）；
/// - 跳过 IPv6 链路本地地址（`fe80::/10`），因为跨平台绑定需要 scope id；
/// - 保留回环、虚拟网卡与 IPv4 链路本地地址。
pub fn enumerate() -> Result<Vec<InterfaceAddress>, AppError> {
    let interfaces = get_if_addrs().map_err(AppError::interface_error)?;
    let mut raw: Vec<RawInterface> = interfaces.into_iter().map(RawInterface::from).collect();

    // Windows 的 if-addrs 可能不返回回环接口；显式补齐，保证本机调试始终可用。
    for ip in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        if !raw.iter().any(|item| item.ip == ip) {
            raw.push(RawInterface {
                name: "Loopback".into(),
                ip,
                is_loopback: true,
                is_up: true,
            });
        }
    }

    Ok(build_addresses(raw))
}

/// 枚举过程中的中间结构，便于在不依赖系统 API 的情况下单测过滤与排序。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawInterface {
    pub name: String,
    pub ip: IpAddr,
    pub is_loopback: bool,
    pub is_up: bool,
}

impl From<Interface> for RawInterface {
    fn from(interface: Interface) -> Self {
        let ip = interface.ip();
        let is_loopback = interface.is_loopback() || ip.is_loopback();
        let is_up = interface.is_oper_up();
        Self {
            name: interface.name,
            ip,
            is_loopback,
            is_up,
        }
    }
}

/// 把原始网卡列表转换为稳定、去重的可选端点。
pub(crate) fn build_addresses(raw: Vec<RawInterface>) -> Vec<InterfaceAddress> {
    let mut seen = HashSet::new();
    let mut addresses: Vec<(InterfaceAddress, bool)> = raw
        .into_iter()
        .filter(|item| !item.ip.is_unspecified())
        .filter(|item| !is_ipv6_link_local(item.ip))
        .filter_map(|item| {
            let id = format!("{}|{}", item.name, item.ip);
            if !seen.insert(id.clone()) {
                return None;
            }
            Some((
                InterfaceAddress {
                    id,
                    name: item.name,
                    ip: item.ip,
                    family: AddressFamily::of(item.ip),
                    is_loopback: item.is_loopback,
                },
                item.is_up,
            ))
        })
        .collect();

    addresses.sort_by(|(left, left_up), (right, right_up)| {
        (
            !left_up,
            left.is_loopback,
            left.family == AddressFamily::Ipv6,
        )
            .cmp(&(
                !right_up,
                right.is_loopback,
                right.family == AddressFamily::Ipv6,
            ))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.ip.cmp(&right.ip))
    });

    addresses.into_iter().map(|(address, _)| address).collect()
}

/// IPv6 链路本地地址：`fe80::/10`。
fn is_ipv6_link_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V6(address) => (address.segments()[0] & 0xffc0) == 0xfe80,
        IpAddr::V4(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv6Addr;

    fn raw(name: &str, ip: &str, is_loopback: bool, is_up: bool) -> RawInterface {
        RawInterface {
            name: name.into(),
            ip: ip.parse().expect("valid ip"),
            is_loopback,
            is_up,
        }
    }

    #[test]
    fn filters_unspecified_and_ipv6_link_local() {
        let addresses = build_addresses(vec![
            raw("以太网", "0.0.0.0", false, true),
            raw("以太网", "fe80::1", false, true),
            raw("以太网", "2001:db8::1", false, true),
            raw("以太网", "192.168.1.10", false, true),
        ]);
        let ips: Vec<String> = addresses.iter().map(|item| item.ip.to_string()).collect();
        assert_eq!(ips, vec!["192.168.1.10", "2001:db8::1"]);
    }

    #[test]
    fn keeps_ipv4_link_local() {
        let addresses = build_addresses(vec![raw("以太网", "169.254.10.20", false, true)]);
        assert_eq!(addresses.len(), 1);
    }

    #[test]
    fn deduplicates_same_name_and_ip() {
        let addresses = build_addresses(vec![
            raw("以太网", "192.168.1.10", false, true),
            raw("以太网", "192.168.1.10", false, true),
        ]);
        assert_eq!(addresses.len(), 1);
        assert_eq!(addresses[0].id, "以太网|192.168.1.10");
    }

    #[test]
    fn keeps_duplicate_ip_on_different_interfaces() {
        let addresses = build_addresses(vec![
            raw("以太网", "192.168.1.10", false, true),
            raw("Wi-Fi", "192.168.1.10", false, true),
        ]);
        assert_eq!(addresses.len(), 2);
        assert_ne!(addresses[0].id, addresses[1].id);
    }

    #[test]
    fn sorts_up_non_loopback_ipv4_first() {
        let addresses = build_addresses(vec![
            raw("Zeta", "192.168.1.30", false, false),
            raw("Loopback", "::1", true, true),
            raw("Wi-Fi", "2001:db8::2", false, true),
            raw("以太网", "192.168.1.10", false, true),
        ]);
        let names: Vec<&str> = addresses.iter().map(|item| item.name.as_str()).collect();
        assert_eq!(names, vec!["以太网", "Wi-Fi", "Loopback", "Zeta"]);
    }

    #[test]
    fn marks_family_and_loopback() {
        let addresses = build_addresses(vec![raw("Loopback", "::1", true, true)]);
        assert_eq!(addresses[0].family, AddressFamily::Ipv6);
        assert!(addresses[0].is_loopback);
    }

    #[test]
    fn detects_ipv6_link_local_range() {
        assert!(is_ipv6_link_local(IpAddr::V6(Ipv6Addr::new(
            0xfe80, 0, 0, 0, 0, 0, 0, 1
        ))));
        assert!(!is_ipv6_link_local(IpAddr::V6(Ipv6Addr::new(
            0x2001, 0xdb8, 0, 0, 0, 0, 0, 1
        ))));
        assert!(!is_ipv6_link_local("192.168.1.1".parse().unwrap()));
    }

    #[test]
    fn enumerates_the_local_machine() {
        let addresses = enumerate().expect("enumerate interfaces");
        assert!(!addresses.is_empty(), "expected at least one local address");
        let ids: HashSet<&str> = addresses.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids.len(), addresses.len(), "ids must be unique");
        assert!(
            addresses.iter().any(|item| item.is_loopback),
            "loopback must always be selectable"
        );
    }
}
