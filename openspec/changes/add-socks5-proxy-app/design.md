# Design

## Context

仓库当前只有 OpenSpec 骨架，没有应用代码。目标平台以 Windows 10/11 为主，开发环境已具备 Rust 1.98、Node 24、pnpm 11.9；未安装全局 `cargo-tauri`，因此统一使用 `@tauri-apps/cli`（2.12.x）作为构建入口。稳定的 Tauri 2（2.12.5）与 Ant Design Vue 4.2.6 均可从 npm / crates.io 获取。

约束：

- 单窗口、本地运行的桌面应用，不依赖服务端。
- 代理必须精确绑定用户所选网卡 IP，端口冲突要能明确反馈。
- 无认证 SOCKS5，需支持 TCP `CONNECT` 与可选 `UDP ASSOCIATE`。
- 界面要求“年轻有活力”，蓝绿色主操作、红黄色运行激活态，使用 Ant Design Vue。

需求见 `proposal.md` 与 `specs/`，本文只说明实现方式。

## Goals / Non-Goals

**Goals:**

- 提供可直接 `pnpm tauri dev` 运行、可打包的 Windows 桌面应用。
- 代理内核与 UI 解耦：Rust 端负责协议与套接字，前端只通过类型化 IPC 调用。
- 启停状态单一事实源在后端，前端通过事件同步，重载窗口不丢状态。
- 协议层可单元测试，关键路径（CONNECT、UDP 关停、端口释放）有集成测试。
- 视觉上形成明确品牌感，避免“默认 Ant Design 表格后台”的观感。

**Non-Goals:**

- 不做认证、ACL、按域名分流、流量统计、限速、系统代理接管。
- 不做多实例、多用户、远程管理。
- 不做 IPv6 链路本地地址绑定（Windows 下缺少 scope id，见 Decisions）。
- 不做自动更新、安装包签名、托盘常驻。

## Decisions

### D1. 技术栈与工程结构

- 前端：Vue 3.5 + TypeScript 5.9 + Vite 8 + Ant Design Vue 4.2。
- 后端：Tauri 2.12 + Tokio 1 + `if-addrs` 0.14 + `thiserror` 2 + `tokio-util` 0.7（`CancellationToken`）。
- 包管理：pnpm；Tauri CLI 通过 devDependency 使用，不要求全局安装。
- 字体：`@fontsource-variable/outfit` 提供拉丁字母与数字，中文回退到 `PingFang SC` / `Microsoft YaHei`，保持离线可用。

```text
proxy-rs/
├─ package.json / pnpm-lock.yaml
├─ index.html / vite.config.ts / tsconfig*.json
├─ src/
│  ├─ main.ts / App.vue
│  ├─ api/ipc.ts              # invoke 包装与事件订阅
│  ├─ composables/useProxy.ts # 表单、状态、启动/停止
│  ├─ lib/validation.ts       # 端口/协议纯函数校验
│  ├─ lib/format.ts           # 端点、运行时长格式化
│  ├─ components/             # BrandHeader / ProxyForm / TransportChips / RunPanel
│  ├─ styles/tokens.css       # 设计令牌、渐变、动效
│  └─ types.ts                # 与 Rust 对齐的 IPC 类型
└─ src-tauri/
   ├─ tauri.conf.json / capabilities/default.json / icons/
   └─ src/
      ├─ main.rs / lib.rs
      ├─ commands.rs          # list_interfaces / start_proxy / stop_proxy / get_status
      ├─ model.rs             # ProxyConfig、ProxyStatus、Transport、InterfaceAddress
      ├─ error.rs             # AppError { code, message }
      ├─ interfaces.rs        # 枚举、去重、排序
      └─ proxy/
         ├─ mod.rs            # ProxyManager、accept 循环、状态广播
         ├─ socks5.rs         # 握手/请求/回复编解码
         ├─ tcp.rs            # CONNECT、源地址绑定、双向转发
         └─ udp.rs            # UDP ASSOCIATE 与中继
```

不使用 Pinia / Vue Router：单页单状态，`useProxy` 组合式函数足够，减少依赖与心智负担。

### D2. 自研最小 SOCKS5 内核，而不是直接套 `fast-socks5`

选择基于 Tokio 自实现 RFC 1928 的无认证子集，理由：

- 需要按用户勾选动态禁用 `CONNECT` 或 `UDP ASSOCIATE`，库的服务器级配置难以精细到单命令。
- 需要为上游 TCP 套接字绑定到所选网卡源 IP；库通常只管监听地址。
- 只需无认证 + CONNECT + UDP ASSOCIATE，协议面小，自研代码量可控（约 600–800 行），且可写精确单测。

放弃 `fast-socks5 1.0`：功能完整但控制粒度不匹配，引入后仍需旁路改造。风险由协议编解码单测与真实客户端联调覆盖。

### D3. 单实例生命周期与取消模型

```rust
pub struct AppState {
    inner: tokio::sync::Mutex<Option<ProxyRuntime>>,
}

struct ProxyRuntime {
    config: ProxyConfig,
    endpoint: SocketAddr,
    started_at: SystemTime,
    cancel: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}
```

- `start_proxy` 在锁内完成：校验 → 重新枚举确认 IP 属本机 → `TcpListener::bind` → 存 `ProxyRuntime` → 释放锁 → 发射 `running` 事件。
- `stop_proxy` 在锁内取出 runtime，`cancel.cancel()`，`timeout(3s, task).await`，随后发射 `stopped`。锁只在这段有界时间内持有，保证启动/停止串行，不会出现“停止未完成就能再启动”的竞态。
- accept 循环与每个会话都 `select!` 监听同一个 `CancellationToken`；停止时监听套接字随任务退出而释放，已建立的 TCP 与 UDP 中继同时被取消。
- 监听任务意外退出时，清空状态并发 `stopped` 事件，附 `message` 说明原因。
- 应用退出：在 `RunEvent::ExitRequested` 中 `block_on(stop_inner())`（2 秒上限），确保端口释放；同时前端在 `beforeunload` 中尽力调用一次停止作为双保险。

不引入 actor 或后台任务管理器：只有一个代理实例，互斥锁 + 取消令牌已足够。

### D4. 网卡枚举与端点标识

使用 `if_addrs::get_if_addrs()`：

- 过滤 IPv6 链路本地地址（`fe80::/10`，`Interface::is_link_local` 且为 V6）；这类地址在 Windows 上绑定需要 scope id，而 `if-addrs` 不提供，强行展示只会产生不可用选项。
- 保留回环、虚拟网卡、IPv4 链路本地地址；排序键：`oper_status == Up` 优先 → 非回环优先 → IPv4 优先 → 网卡名 → IP。
- 在列表首位插入通用项 `0.0.0.0`（名称“所有网卡”），用于监听全部 IPv4 网卡；它不是真实网卡地址，但后端允许作为通配绑定地址。
- `id = "{interface_name}|{ip}"`，并做去重；同一网卡多地址产生多个条目。
- 展示名由前端生成：`{网卡名} · {IP}`，回环追加“回环”标签，IPv6 追加“IPv6”；通配项显示为 `0.0.0.0 · 所有网卡`。
- 默认选中第一个非回环、非通配 IPv4；没有时选第一个非回环地址；再没有才选回环。通配地址不会被自动选中。

`ProxyConfig.interface_name` 仅用于错误信息与状态回显；后端以 IP 为准并再次确认它仍属于本机，避免网卡重命名造成误绑定。

### D5. 绑定与出站

- 监听：`TcpListener::bind(SocketAddr::new(config.ip, config.port))`。选择具体网卡时只绑定该单播地址；选择 `0.0.0.0` 时监听全部 IPv4 网卡，界面必须展示局域网暴露警告。
- 上游 TCP：按目标地址族创建 `TcpSocket`；当地址族与所选源 IP 一致、源 IP 非回环且非 `0.0.0.0` 时，`bind(SocketAddr::new(source_ip, 0))`；绑定失败记录并回退到未绑定套接字，不让请求失败（对应 `socks5-service` 的 “Egress bound to selected source address”）。
- UDP 中继绑定 `0.0.0.0` 时，成功回复使用控制连接实际使用的本地地址（例如 `127.0.0.1`），避免返回不可连接的 `0.0.0.0`。
- 域名通过 `tokio::net::lookup_host` 解析，逐个尝试，连接超时 10s；全部失败映射为 host unreachable。
- 控制连接与中继均设置 `TCP_NODELAY`；`copy_bidirectional` 处理半关闭。
- 监听端口不做 `SO_REUSEADDR`，确保“端口被占用”能被真实检测并反馈。

### D6. UDP ASSOCIATE

- 收到命令后，在所选源 IP 上绑定 `UdpSocket`（端口 0），成功回复使用实际 `local_addr()`。
- 关联与 TCP 控制连接同生命周期：控制连接 EOF、取消令牌触发或空闲 5 分钟即结束；结束后释放中继端口。
- 客户端身份：只接受来自控制连接对端 IP 的数据报；在同一 IP 下锁定首个数据报的源端口，忽略其他来源（防开放中继）。
- 每关联维护按地址族懒创建的出站 UDP 套接字（v4/v6 各一），同族时绑定所选源 IP；用于向任意目标 `send_to` 并接收回包。
- 数据报解析：至少 4 字节，`RSV == 0`、`FRAG == 0`；`FRAG != 0` 静默丢弃。回复头写入来源地址与端口，地址使用 IP 形式。
- 域名的目标每次解析（UDP 无连接语义），失败丢弃该包，不关闭关联。

### D7. IPC 契约

Rust 侧统一 `#[serde(rename_all = "camelCase")]`，前端 `types.ts` 手工对齐；命令错误返回 `{ code, message }`。

```ts
type Transport = 'tcp' | 'udp';

interface InterfaceAddress {
  id: string;
  name: string;
  ip: string;
  family: 'ipv4' | 'ipv6';
  isLoopback: boolean;
}

interface ProxyConfig {
  interfaceName: string;
  ip: string;
  port: number;
  protocols: Transport[];
}

interface ProxyStatus {
  state: 'stopped' | 'running';
  endpoint: string | null;     // socks5://ip:port
  config: ProxyConfig | null;  // running 时回显
  startedAt: number | null;    // epoch ms
  message: string | null;      // 异常停止原因
}
```

| 命令 | 入参 | 返回 | 错误码 |
| --- | --- | --- | --- |
| `list_network_interfaces` | — | `InterfaceAddress[]` | `interface_error` |
| `start_proxy` | `ProxyConfig` | `ProxyStatus` | `invalid_port` / `invalid_ip` / `no_transport` / `already_running` / `bind_failed` / `internal` |
| `stop_proxy` | — | `ProxyStatus` | `internal` |
| `get_proxy_status` | — | `ProxyStatus` | `internal` |

事件名 `proxy://status`，载荷同 `ProxyStatus`；前端挂载时先 `get_proxy_status`，再订阅事件，避免窗口重载丢失状态。`endpoint` 由后端格式化为 `socks5://{ip}:{port}`（IPv6 加方括号），前端只负责展示与复制。

### D8. 前端结构与交互

- `useProxy.ts` 持有：接口列表、表单（endpointId、port 字符串、protocols）、status、loading/starting/stopping、error。
- 端口使用 `a-input`（字符串）而不是 `InputNumber`：能对 `abc`、`10.5`、空串给出明确字段错误，且不会被组件静默 clamp。纯函数 `validatePort(raw)` 返回 `{ ok, message }`，前端与后端各自实现同一规则。
- 三行输入用 `a-form` 的 `layout="vertical"` 与 `required` 标记；协议用两个自定义卡片式 `a-checkbox`（不用原生 `Checkbox.Group` 外观），取消最后一个时由 change 处理函数回滚并给出行内提示。
- 主按钮不使用 antd 默认实心色，而是通过 `.action-button` 覆盖背景为渐变；运行态追加 `.is-active`，保证蓝绿→红黄切换只发生在一处。
- 状态只来自后端：启动成功后用返回的 `ProxyStatus` 写入；停止中先置 `stopping` 作乐观 UI，事件到达后对齐。
- 运行时长由 `startedAt` 计算，`setInterval(1000)` 更新，卸载时清理。
- 错误呈现：字段错误在对应行；启动/停止失败用卡片内 `a-alert`（`type="error"`），不依赖全局 message；复制成功用按钮内图标切换 `CheckOutlined` 1.5s 反馈。
- 剪贴板使用 `@tauri-apps/plugin-clipboard-manager`，避免 WebView2 下 `navigator.clipboard` 权限差异。
- 无认证风险提示：选中非回环地址时显示常驻 `a-alert type="warning"`；运行中仍然显示，直到停止且改选回环。

### D9. 视觉设计规范（年轻、有活力）

整体方向：**“薄荷实验室”**——浅色画布 + 高饱和渐变主操作 + 大圆角 + 数字编号步骤感；避免传统管理后台的密集表格与灰蓝配色。视觉只出现在装饰层和主按钮，表单本身保持安静、可读。

**色彩令牌**

| 令牌 | 值 | 用途 |
| --- | --- | --- |
| `--canvas` | `#F3FBF9` | 窗口底色 |
| `--ink-900` | `#0B1F24` | 标题 |
| `--ink-600` | `#4B6470` | 正文/辅助 |
| `--ink-400` | `#7C949E` | 占位、说明 |
| `--card` | `rgba(255,255,255,0.82)` | 主卡片，配 `backdrop-filter: blur(20px)` |
| `--border` | `rgba(11,31,36,0.08)` | 卡片/控件描边 |
| `--teal-600` | `#0B7A6E` | antd `colorPrimary`、焦点环 |
| `--start-a` / `--start-b` | `#34E7C4` / `#35B8F5` | 开始按钮渐变 |
| `--stop-a` / `--stop-b` | `#FF6B57` / `#FFC93C` | 运行中停止按钮渐变 |
| `--on-start` | `#06262E` | 开始按钮文字（深色，保证对比度） |
| `--on-stop` | `#3B1200` | 停止按钮文字 |
| `--warn-bg` / `--warn-text` | `#FFF7E6` / `#8A4B00` | 无认证暴露提示 |
| `--focus` | `#0F766E` | 2px 焦点环 |

按钮采用深色文字压在亮渐变上（而非白字），对比度均超过 WCAG AA 4.5:1；hover 增加 6% 亮度与外发光，active 轻微下压 1px。红黄只用于运行中主按钮、状态点与运行计时，不与蓝绿混用在同一次要控件上。

**版式与布局**

- 窗口 980×720，最小 860×640，居中；页面外边距 32/24px，内容最大宽 680px 居中。
- 背景：`--canvas` 上叠三团径向渐变光斑（薄荷 `#C9FFF1`、天蓝 `#D6E9FF`、柠檬 `#FFF3C4`），`filter: blur(60px)`，20s 缓慢漂移；`prefers-reduced-motion` 下静止。
- 顶部品牌行：44px 圆角方形 logo（蓝绿渐变 + 白色闪电），标题 `Proxy RS`（Outfit 700，34px），副标题“SOCKS5 代理控制台”（13px，字距 .08em）；右侧状态胶囊。
- 主卡片：白 82% + 模糊，24px 圆角，1px 边框，阴影 `0 24px 60px rgba(11,31,36,.08)`，内边距 28px。
- 三行以 `01 / 02 / 03` 编号呈现，编号用 Outfit 700、12px、teal 色；标签 13px/600，说明文字 12px。
- 控件高度 44px，圆角 12px；下拉与输入聚焦时 2px `--focus` 环。
- 协议 chip：两个等宽卡片（高 56px），内含图标 + 名称 + 小字说明（TCP “网页/SSH 等连接”、UDP “DNS/游戏等数据报”）；选中态为薄荷色底 + teal 描边 + 右上角勾选徽标；未选中为白底灰边。
- 主按钮：全宽 52px，16px/700，圆角 16px，蓝绿渐变；运行中同一位置换成红黄渐变“停止代理”，并加 8px 红色柔光。启动中显示 `loading` 与“正在启动…”。
- 运行信息条：端点等宽字体 + 复制图标 + 分隔线 + 计时（tabular-nums）；停止后淡出。
- 空状态/错误：`a-alert` 放在第三行与按钮之间，不影响三行结构。
- 动效：180–240ms，`cubic-bezier(.22,1,.36,1)`；仅用于按钮、卡片入场、状态点脉冲；不使用弹跳或持续闪烁。运行状态点为 8px 红黄渐变圆点 + 1.8s 扩散环。

**窗口线框**

```text
┌──────────────────────────────────────────────────────┐
│  ▣  Proxy RS                            ● 未运行      │
│     SOCKS5 代理控制台                                 │
│                                                      │
│  ╭────────────────────────────────────────────────╮  │
│  │ 01  网卡与地址                         [刷新]  │  │
│  │  [ 以太网 · 192.168.1.23 · IPv4        ▾ ]    │  │
│  │                                                │  │
│  │ 02  端口                                       │  │
│  │  [ 1080            ]  建议使用 1080 及以上     │  │
│  │                                                │  │
│  │ 03  传输协议                                   │  │
│  │  [✓ TCP 连接]      [  UDP 数据报 ]             │  │
│  │                                                │  │
│  │  ⚠ 无认证代理，局域网内设备均可访问…           │  │
│  │  [            ⚡ 开始代理                     ] │  │
│  ╰────────────────────────────────────────────────╯  │
│                                                      │
│  socks5://192.168.1.23:1080   ⧉   运行 00:12:34      │
└──────────────────────────────────────────────────────┘
```

运行中：标题右侧胶囊变为红黄“运行中”，卡片内三行置灰只读，按钮变为红黄“停止代理”，底部运行信息条高亮。

**Ant Design Vue 主题**

```ts
// App.vue 内的 ConfigProvider :theme
{
  token: {
    colorPrimary: '#0B7A6E',
    colorInfo: '#0B7A6E',
    colorSuccess: '#0E9384',
    colorWarning: '#D97706',
    colorError: '#DC2626',
    colorTextBase: '#0B1F24',
    colorBgBase: '#F3FBF9',
    borderRadius: 12,
    fontFamily: '"Outfit Variable", "PingFang SC", "Microsoft YaHei", sans-serif',
    fontSize: 14,
  },
  components: {
    Button: { controlHeightLG: 52, borderRadiusLG: 16, fontWeight: 700 },
    Select: { controlHeight: 44 },
    Input: { controlHeight: 44 },
  },
}
```

主题只设语义色，渐变、光斑、chip 选中态写在 `styles/tokens.css`，避免散落内联样式。Ant Design 的 `zhCN` locale 通过 `a-config-provider :locale` 注入。

### D10. 测试与验证

- Rust 单测：端口校验、握手方法选择、请求编解码（IPv4/IPv6/域名、截断、非法 ATYP）、回复编码、UDP 头编解码与 FRAG 丢弃、网卡排序去重。
- Rust 集成测试：在本机临时端口启动代理，用 `tokio` 客户端完成无认证握手 + `CONNECT` 到本地回声服务；停止后立即重新绑定同一端口；UDP 关联可用 `UdpSocket` 手工构造请求/回包验证。测试入口直接调用 `proxy::start_for_test`（`cfg(test)` 可见），不经过 Tauri。
- 前端单测（Vitest）：`validatePort`、`toggleTransport`（至少保留一项）、`formatEndpoint`、`formatUptime`、状态事件归并。
- 手工验收：`pnpm tauri dev`；`curl --socks5-hostname 127.0.0.1:1080 https://example.com`；Windows 防火墙提示下允许后从同网段设备连接；关闭窗口后 `Test-NetConnection` 确认端口释放。
- 构建门槛：`pnpm build`（`vue-tsc --noEmit` + Vite）、`cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`、`pnpm tauri build --debug --no-bundle`。

### D11. Tauri 配置与安全

- `productName: "Proxy RS"`，`identifier: "dev.proxyrs.app"`，窗口标题 `Proxy RS · SOCKS5 代理`，尺寸见 D9，`resizable: true`。
- 能力最小化：仅 `core:default` 与 `clipboard-manager:allow-write-text`；不开启 shell、文件系统、HTTP 等插件。
- 生产 CSP：`default-src 'self'; connect-src ipc: http://ipc.localhost; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:`；开发模式保留 Vite HMR 所需连接。
- 默认不监听 `0.0.0.0`，但允许用户在明确选择通用地址时监听全部 IPv4 网卡；该状态显示更强的无认证暴露警告，并提示本机使用 `127.0.0.1`、其他设备使用本机局域网 IP。
- 关闭 devtools 的 release 构建；不接收任何远程配置。

## Risks / Trade-offs

- [Windows 防火墙首次监听弹窗] → 在 README/界面错误中提示“允许专用网络访问”；未允许时从其他设备连接会超时，错误信息建议检查防火墙。
- [无认证 + 非回环地址 = 局域网开放代理] → 常驻高风险提示、默认选择非回环但明确展示端点；`0.0.0.0` 需要用户主动选择且提示监听所有网卡；不做自动 UPnP/端口映射；用户可改选回环。
- [IPv6 链路本地地址被排除] → 避免展示必然绑定失败的选择；需要的用户可使用全局 IPv6 或 IPv4。
- [自研 SOCKS5 协议细节出错] → 单测覆盖编解码，集成测试跑真实握手与转发；保留切换到成熟库的模块边界（`proxy/socks5.rs` 只做协议编解码）。
- [绑定源 IP 可能失败（网卡休眠、地址变更、回环源连外网）] → 捕获失败并回退默认路由，同时记录调试信息，不影响请求。
- [UDP 单套接字中继在多目标场景下的回包归属] → 依赖 `recv_from` 的来源地址重新封装响应头；若目标大量并发，单关联仍可工作，但吞吐受限，符合轻量工具定位。
- [Ant Design 渐变覆盖受组件内部样式影响] → 渐变只作用于自定义 class 的 `background-image`，用 `:deep` 限定作用域，并实测 hover/active/disabled 三态。
- [Vite 8 与 Tauri devUrl 端口冲突] → `strictPort: true` 固定 1420，冲突时直接报错而不是漂移端口。
- [TypeScript 7 与 vue-tsc 兼容性风险] → 固定 TypeScript 5.9.x，与 vue-tsc 3.x 的 peer 范围一致。

## Migration Plan

绿地项目，无数据迁移。

1. 按 tasks 顺序创建前端与 `src-tauri`，在 Windows 上完成 `pnpm tauri dev` 验证。
2. 首次发布使用 `pnpm tauri build` 生成 MSI/NSIS；若签名不可用，先在 README 标注未签名安装包。
3. 回滚：删除新增目录与依赖即可，应用不写入注册表、不修改系统代理设置；唯一副作用是用户手动允许过的防火墙入站规则，可在系统设置中移除。

## Open Questions

无。窗口是否常驻托盘、是否记录历史会话属于后续增强，不影响本变更的规格与任务拆分。
