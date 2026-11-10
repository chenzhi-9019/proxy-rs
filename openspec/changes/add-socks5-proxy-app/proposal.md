# Proposal

## Why

当前仓库只有一个空目录，缺少一个可在桌面端一键启用的轻量 SOCKS5 代理工具。开发与测试人员通常需要手写命令或依赖第三方工具，难以精确指定“哪块网卡的哪个 IP、哪个端口、启用哪种传输协议”。本变更提供一个 Tauri + Vue 3 的桌面应用，把网卡选择、端口校验与协议开关收敛到一块年轻、有活力的单页界面中，点击“开始代理”即在本机创建无认证 SOCKS5 服务，降低搭建本地代理的门槛。

## What Changes

- 新建 Tauri 2 + Vue 3 + Vite + TypeScript 桌面应用骨架，UI 使用 Ant Design Vue 4。
- 新增网卡枚举能力：启动界面时读取本机所有网卡及其 IPv4/IPv6 地址，按“网卡 · IP”聚合展示为下拉选项，并支持手动刷新。
- 新增 SOCKS5 代理服务：仅支持无认证（method `0x00`），按所选网卡 IP 与端口监听；TCP 走 `CONNECT`，UDP 走 `UDP ASSOCIATE`，未勾选的协议对应命令返回 `Command not supported`。
- 新增代理生命周期控制：开始、停止、查询状态；运行中通过 Tauri 事件推送运行状态与监听端点，窗口关闭时优雅停止监听。
- 新增单页控制台界面：三行输入（网卡/IP 下拉、端口号、TCP/UDP 多选且至少一个），蓝绿色“开始代理”按钮，运行中切换为红黄色“停止代理”激活态，并展示运行端点、运行时长与错误提示。
- 端口与协议在前端表单和后端命令两层校验；端口范围 1–65535，被占用或不可绑定时返回可读错误。
- 明确不做：用户名/密码认证、系统全局代理设置、流量统计与限速、按域名/规则分流、开机自启、多实例并行代理。

## Capabilities

### New Capabilities

- `network-interfaces`: 枚举本机网卡与地址，并向前端提供可选择、可刷新、带回环标识的网卡 IP 列表。
- `proxy-lifecycle`: 代理的启动、停止、状态查询与事件通知，包含参数校验、绑定失败处理与退出清理。
- `socks5-service`: 无认证 SOCKS5 服务行为，覆盖 TCP `CONNECT`、可选 `UDP ASSOCIATE`、按所选协议拒绝命令，以及从所选源 IP 出站。
- `proxy-console-ui`: 单页控制台的交互与视觉规则，覆盖三行表单、至少一个传输协议、端口校验反馈、开始/停止按钮状态与整体主题。

### Modified Capabilities

- 无。

## Impact

- 新增前端工程：`package.json`、`vite.config.ts`、`index.html`、`src/`（Vue 3 + TypeScript + Ant Design Vue）。
- 新增 Rust 工程：`src-tauri/`（Tauri 2、Tokio、`if-addrs`、`thiserror`），含 SOCKS5 协议实现、网卡枚举、命令与状态管理。
- 新增依赖：`vue`、`vite`、`ant-design-vue`、`@ant-design/icons-vue`、`@tauri-apps/api`、`@tauri-apps/cli`；Rust 侧 `tauri`、`tokio`、`if-addrs`、`thiserror`、`serde`。
- 影响本机网络：应用会在所选 IP 上监听端口；选择非回环地址时，同一局域网内可无认证访问，界面需给出显式风险提示。
- 主要目标平台为 Windows 10/11（当前开发环境），网卡枚举与套接字绑定使用跨平台实现，macOS/Linux 保持可编译。
