# Tasks

## 1. 工程脚手架

- [x] 1.1 创建 Vue 3 + TypeScript + Vite 前端骨架：`package.json`（`dev`/`build`/`preview`/`tauri` 脚本、依赖版本按 design.md D1）、`vite.config.ts`（端口 1420、`strictPort: true`）、`tsconfig*.json`、`index.html`、`src/main.ts`、`src/App.vue` 占位页与 `.gitignore`；执行 `pnpm install` 后 `pnpm dev` 能打开占位页，`pnpm build` 通过。
- [x] 1.2 初始化 Tauri 2 后端：执行 `pnpm tauri init --ci --app-name "Proxy RS" --window-title "Proxy RS · SOCKS5 代理" --frontend-dist ../dist --dev-url http://localhost:1420 --before-dev-command "pnpm dev" --before-build-command "pnpm build"`，将窗口设为 980×720、最小 860×640、可缩放、居中，生成图标；`pnpm tauri dev` 能打开原生窗口并加载占位页。
- [x] 1.3 在 `src-tauri/Cargo.toml` 加入 `tokio`、`tokio-util`、`if-addrs`、`thiserror`、`serde`、`tauri-plugin-clipboard-manager` 依赖，注册剪贴板插件并把 `capabilities/default.json` 收敛为 `core:default` + `clipboard-manager:allow-write-text`；`cargo check` 与 `pnpm tauri dev` 均通过。
- [x] 1.4 写 README 基础章节：技术栈、`pnpm install`/`pnpm tauri dev`/`pnpm tauri build`、Windows 防火墙与无认证风险提示；按 README 命令逐条执行成功。

## 2. 后端模型、错误与网卡枚举

- [x] 2.1 实现 `model.rs`：`Transport`、`ProxyConfig`、`ProxyStatus`、`InterfaceAddress`，统一 `serde(rename_all = "camelCase")`，实现 `ProxyConfig::validate`（端口 1–65535、至少一种协议、IP 字面量）与 endpoint 格式化（IPv6 带方括号）；`cargo test` 覆盖合法/非法端口、零协议、IPv6 端点。
- [x] 2.2 实现 `error.rs`：`AppError { code, message }`、`thiserror` 转换与 `Serialize`，错误码与 design.md D7 表格一致；单测断言每个变体序列化后的 `code`。
- [x] 2.3 实现 `interfaces.rs`：从 `if_addrs` 映射、过滤 IPv6 链路本地、按 (运行状态, 回环, 地址族, 网卡名, IP) 排序、按 `name|ip` 去重并生成 `id`；纯函数单测覆盖排序、去重、过滤，`cargo test` 通过；真实枚举在 dev 启动日志中打印条数做冒烟。
- [x] 2.4 为 `interfaces.rs` 与 `model.rs` 补充 rustdoc（说明过滤与校验规则）；`cargo doc --no-deps` 无警告。

## 3. SOCKS5 协议内核

- [x] 3.1 实现 `socks5.rs` 握手与请求解析：版本校验、方法列表（仅接受 `0x00`，否则 `0xFF`）、命令/保留位/ATYP（IPv4、IPv6、域名长度 1–255）、截断与超长处理；单测覆盖正常、无 `0x00`、非法版本、非法 ATYP、截断请求。
- [x] 3.2 实现回复编码与 REP 映射（成功、拒绝、网络/主机不可达、命令不支持、地址类型不支持）；单测断言 IPv4/IPv6/错误回复的字节序列。
- [x] 3.3 实现 `tcp.rs`：域名解析、10s 超时、同族源 IP 绑定与失败回退、`copy_bidirectional` 转发；集成测试启动本地回声服务，验证 `CONNECT` 成功、上游拒绝返回 `0x05`、未启用 TCP 时返回 `0x07`。
- [x] 3.4 实现 UDP 请求/响应头编解码与 `FRAG != 0` 丢弃逻辑；单测覆盖 IPv4/IPv6/域名、零长度负载、截断包与分片包。
- [x] 3.5 实现 `udp.rs` 关联：在所选 IP 上绑定中继、回复实际地址、锁定控制连接对端 IP 与首个数据报端口、按地址族懒创建出站套接字、控制连接关闭/取消/空闲 5 分钟结束；集成测试用 `UdpSocket` 完成一次请求-响应，并验证控制连接关闭后中继端口释放、未启用 UDP 时返回 `0x07`。
- [x] 3.6 会话隔离与超时：握手 30s 超时、单会话错误只关闭自身；集成测试发送非法字节后确认监听仍可接受新连接。

## 4. 生命周期、IPC 与剪贴板

- [x] 4.1 实现 `proxy/mod.rs` 的 `ProxyManager`：绑定监听、accept 循环、`CancellationToken` 取消、状态与端点记录；测试覆盖启动后端口可连、重复启动返回 `already_running`、停止后立即用同端口重新启动成功。
- [x] 4.2 接入 Tauri 命令 `list_network_interfaces` / `start_proxy` / `stop_proxy` / `get_proxy_status`，用 `tauri::State` 管理单实例，状态变化通过 `proxy://status` 事件推送，异常退出携带 `message`；`cargo test` 通过，`pnpm tauri dev` 启动后可在 WebView 控制台调用并看到 running/stopped 载荷。
- [x] 4.3 在 `RunEvent::ExitRequested`（及窗口关闭路径）调用停止逻辑，2 秒上限；手工验证代理运行中关闭应用后，`Test-NetConnection 127.0.0.1 -Port <port>` 失败且端口可被其他进程立即绑定。
- [x] 4.4 注册 `tauri-plugin-clipboard-manager` 并封装前端 `copyText()`；`cargo check` 与 `pnpm tauri dev` 通过，手工点击复制后粘贴验证内容为完整端点。

## 5. 前端界面与状态

- [x] 5.1 落地设计令牌与全局主题：`styles/tokens.css`（颜色、渐变、阴影、动效、`.action-button` 两态）、Outfit 字体、背景光斑、`ConfigProvider` 主题与 `zhCN`；`pnpm build` 通过，980×720 截图中主按钮为蓝绿渐变且文字对比度达到 AA。
- [x] 5.2 实现纯函数 `lib/validation.ts`、`lib/format.ts`（端口校验、至少保留一种协议、端点/运行时长格式化、错误归一化）；Vitest 覆盖 design.md D10 列出的用例，`pnpm test` 通过。
- [x] 5.3 实现 `BrandHeader` 与状态胶囊（未运行灰、运行中红黄脉冲）、页面栅格与卡片入场动效；`pnpm build` 通过，截图确认品牌区、卡片与背景层次符合 design.md D9。
- [x] 5.4 实现 `ProxyForm`：网卡分组下拉（支持按名称/IP 搜索、刷新、加载态、选中保留/清除）、端口字符串输入与就地对齐错误、TCP/UDP 卡片式多选（取消最后一项时回滚并提示）、非回环地址警告；在 980×720 与 860×640 下无裁切、无横向滚动，Tab 焦点可见。
- [x] 5.5 实现主操作按钮三态（可用蓝绿、启动中 loading、运行中红黄“停止代理”）、运行信息条（端点、复制、计时）、错误 Alert 与停止中过渡；截图核对 idle/running/error 三态，蓝绿与红黄不混用。
- [x] 5.6 实现 `useProxy` 组合式函数：挂载时并行拉取网卡与状态、订阅 `proxy://status` 并清理、启动/停止调用与乐观状态、运行中锁定表单、窗口重载后从后端状态回填；`pnpm build` 通过，dev 模式下重载 WebView 后状态与端点保持正确。

## 6. 集成验收

- [x] 6.1 TCP 端到端：在 `127.0.0.1:1080` 启动，使用 `curl --socks5-hostname 127.0.0.1:1080 https://example.com` 成功；停止后端口释放，再启动同端口成功；记录命令输出与界面截图。
- [x] 6.2 UDP 端到端：启用 UDP 后用测试客户端完成 `UDP ASSOCIATE` 请求-响应；仅勾选 TCP 时同一客户端收到 `0x07`；确认停止后中继端口释放。
- [x] 6.3 视觉与可访问性检查：980×720 与 860×640 截图、键盘完成全流程、焦点环可见、`prefers-reduced-motion` 下光斑与脉冲停止、正文对比度达 AA。
- [x] 6.4 质量门槛：`cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`、`pnpm test`、`pnpm build`、`pnpm tauri build --debug --no-bundle` 全部通过，无未使用依赖与编译警告。
- [x] 6.5 补全 README 使用篇：界面三行说明、开始/停止、SOCKS5 客户端示例（curl/浏览器）、仅 TCP 与 TCP+UDP 差异、防火墙与安全边界、已知限制；按文档从零执行一遍安装与启动。

## Workflow follow-up

- 评审通过后归档变更，并按项目要求同步主 specs。
- 归档后复跑 `openspec validate` 与质量门槛，确认无回归。
