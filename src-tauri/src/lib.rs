#![allow(linker_messages)]

use std::time::Duration;

use tauri::{Emitter, Manager};

pub mod commands;
pub mod error;
pub mod interfaces;
pub mod model;
pub mod proxy;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState {
            manager: proxy::ProxyManager::new(),
        })
        .setup(|app| {
            #[cfg(debug_assertions)]
            match interfaces::enumerate() {
                Ok(addresses) => eprintln!("[proxy-rs] 发现 {} 个可绑定网卡地址", addresses.len()),
                Err(error) => eprintln!("[proxy-rs] 网卡枚举失败：{}", error.message),
            }

            let handle = app.handle().clone();
            let mut events = app.state::<AppState>().manager.subscribe();
            tauri::async_runtime::spawn(async move {
                while let Ok(status) = events.recv().await {
                    let _ = handle.emit("proxy://status", status);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_network_interfaces,
            commands::start_proxy,
            commands::stop_proxy,
            commands::get_proxy_status
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            let state = app_handle.state::<AppState>();
            tauri::async_runtime::block_on(async {
                let _ = tokio::time::timeout(Duration::from_secs(2), state.manager.stop()).await;
            });
        }
    });
}
