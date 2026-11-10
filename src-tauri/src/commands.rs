//! Tauri IPC 命令。所有命令只做参数转发，业务逻辑位于 `proxy` 与 `interfaces`。

use tauri::State;

use crate::error::AppError;
use crate::interfaces;
use crate::model::{InterfaceAddress, ProxyConfig, ProxyStatus};
use crate::proxy::ProxyManager;

/// 应用全局状态：只有一个代理管理器。
pub struct AppState {
    pub manager: ProxyManager,
}

/// 枚举本机可绑定的网卡地址。
#[tauri::command]
pub async fn list_network_interfaces() -> Result<Vec<InterfaceAddress>, AppError> {
    interfaces::enumerate()
}

/// 启动代理。
#[tauri::command]
pub async fn start_proxy(
    state: State<'_, AppState>,
    config: ProxyConfig,
) -> Result<ProxyStatus, AppError> {
    state.manager.start(config).await
}

/// 停止代理。
#[tauri::command]
pub async fn stop_proxy(state: State<'_, AppState>) -> Result<ProxyStatus, AppError> {
    state.manager.stop().await
}

/// 查询当前代理状态。
#[tauri::command]
pub async fn get_proxy_status(state: State<'_, AppState>) -> Result<ProxyStatus, AppError> {
    Ok(state.manager.status().await)
}
