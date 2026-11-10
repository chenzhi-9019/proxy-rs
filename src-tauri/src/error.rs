use std::fmt::Display;

use serde::Serialize;

/// 前端可解析的统一错误结构。
///
/// `code` 是稳定的机器可读标识，`message` 是面向用户的中文说明。
#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct AppError {
    pub code: String,
    pub message: String,
}

impl AppError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }

    /// 端口不是 1–65535 的整数。
    pub fn invalid_port(port: i64) -> Self {
        Self::new(
            "invalid_port",
            format!("端口 {port} 不合法，应为 1–65535 之间的整数"),
        )
    }

    /// 所选地址不属于本机，或不是可绑定的单播地址。
    pub fn invalid_ip(ip: impl Display) -> Self {
        Self::new(
            "invalid_ip",
            format!("地址 {ip} 不在本机网卡上，或不是可绑定的单播地址"),
        )
    }

    /// 没有勾选任何传输协议。
    pub fn no_transport() -> Self {
        Self::new("no_transport", "请至少选择一种传输协议（TCP 或 UDP）")
    }

    /// 已有代理实例在运行。
    pub fn already_running() -> Self {
        Self::new("already_running", "代理已在运行，请先停止当前实例")
    }

    /// 监听套接字绑定失败。
    pub fn bind_failed(endpoint: impl Display, reason: impl Display) -> Self {
        Self::new(
            "bind_failed",
            format!("无法在 {endpoint} 上启动代理：{reason}"),
        )
    }

    /// 网卡枚举失败。
    pub fn interface_error(reason: impl Display) -> Self {
        Self::new("interface_error", format!("读取本机网卡失败：{reason}"))
    }

    /// 未归类的内部错误。
    pub fn internal(reason: impl Display) -> Self {
        Self::new("internal", format!("代理内部错误：{reason}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::internal(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code_of(error: &AppError) -> &str {
        &error.code
    }

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(code_of(&AppError::invalid_port(0)), "invalid_port");
        assert_eq!(code_of(&AppError::invalid_ip("1.2.3.4")), "invalid_ip");
        assert_eq!(code_of(&AppError::no_transport()), "no_transport");
        assert_eq!(code_of(&AppError::already_running()), "already_running");
        assert_eq!(
            code_of(&AppError::bind_failed("127.0.0.1:1080", "in use")),
            "bind_failed"
        );
        assert_eq!(
            code_of(&AppError::interface_error("nope")),
            "interface_error"
        );
        assert_eq!(code_of(&AppError::internal("boom")), "internal");
    }

    #[test]
    fn serializes_as_code_and_message() {
        let value = serde_json::to_value(AppError::no_transport()).expect("serialize");
        assert_eq!(value["code"], "no_transport");
        assert!(
            value["message"]
                .as_str()
                .is_some_and(|m| m.contains("至少"))
        );
    }
}
