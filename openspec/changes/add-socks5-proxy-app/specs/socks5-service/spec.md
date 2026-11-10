# Spec Delta

## Purpose

规定无认证 SOCKS5 服务的对外协议行为，使标准 SOCKS5 客户端可以通过用户选定的网卡 IP 与端口完成 TCP 与 UDP 代理。

## ADDED Requirements

### Requirement: No-auth method negotiation

服务 SHALL 仅支持无认证方式（method `0x00`），不得要求用户名或密码。当客户端未提供 `0x00` 时，服务 MUST 回复 `0xFF`（无可接受的方法）并关闭连接。

#### Scenario: Accept no-auth client

- **WHEN** 客户端问候提供 `0x00`
- **THEN** 服务选择 `0x00` 并进入请求阶段

#### Scenario: Reject credential-only client

- **WHEN** 客户端仅提供用户名/密码方式
- **THEN** 服务回复 `0xFF` 并关闭该连接
- **AND** 不读取也不校验任何凭据

### Requirement: TCP CONNECT relay

启用 TCP 时，服务 SHALL 处理 `CONNECT` 请求，支持 IPv4、IPv6 与域名目标；成功时回复绑定地址并双向转发字节流，任一方向 EOF 后 MUST 正确半关闭，连接错误时按 SOCKS5 回复码返回失败。

#### Scenario: Successful IPv4 connect

- **WHEN** 客户端请求 `CONNECT` 到可达的 IPv4 目标
- **THEN** 服务回复成功及本地绑定地址
- **AND** 客户端与目标之间的双向数据完整转发

#### Scenario: Domain target

- **WHEN** 客户端以域名形式请求 `CONNECT`
- **THEN** 服务解析域名并按解析结果建立连接
- **AND** 解析失败时回复主机不可达类错误

#### Scenario: Upstream refused

- **WHEN** 目标端口拒绝连接
- **THEN** 服务回复连接被拒绝（`0x05`）
- **AND** 该会话关闭，其他会话不受影响

#### Scenario: TCP disabled

- **WHEN** 用户未启用 TCP 而客户端发送 `CONNECT`
- **THEN** 服务回复命令不支持（`0x07`）并关闭该会话

### Requirement: Egress bound to selected source address

服务 SHALL 在地址族匹配且所选源地址非回环时把上游 TCP 套接字绑定到该源 IP；当地址族不匹配、源地址为回环或绑定失败时 MUST 回退到系统默认路由，且不因此拒绝客户端请求。

#### Scenario: Same-family egress binding

- **WHEN** 监听与目标均为 IPv4，且所选源地址为非回环网卡 IP
- **THEN** 上游连接使用所选网卡 IP 作为源地址

#### Scenario: Family mismatch fallback

- **WHEN** 监听为 IPv4 而目标为 IPv6
- **THEN** 服务仍尝试建立连接并回退到系统默认路由
- **AND** 不因无法绑定源地址而直接拒绝请求

### Requirement: UDP ASSOCIATE relay

启用 UDP 时，服务 SHALL 处理 `UDP ASSOCIATE`，在所选网卡 IP 上创建中继套接字并在成功回复中返回其中继地址与端口；关联 MUST 与发起请求的 TCP 控制连接同生命周期，控制连接关闭即终止关联。未启用 UDP 时该命令 MUST 回复命令不支持（`0x07`）。

#### Scenario: Establish association

- **WHEN** 客户端在启用 UDP 时发送 `UDP ASSOCIATE`
- **THEN** 服务回复成功及 UDP 中继的 IP 与端口
- **AND** 中继仅接受来自该客户端 IP 的数据报

#### Scenario: Control connection closes

- **WHEN** 客户端关闭发起关联的 TCP 控制连接
- **THEN** 服务停止该 UDP 中继并释放其端口
- **AND** 不影响其他关联或 TCP 会话

#### Scenario: UDP disabled

- **WHEN** 用户未启用 UDP 而客户端发送 `UDP ASSOCIATE`
- **THEN** 服务回复命令不支持（`0x07`）并关闭会话

### Requirement: UDP datagram framing

UDP 中继 SHALL 按 SOCKS5 UDP 请求格式解析 `RSV`、`FRAG`、目标地址与端口，并在回包中附加来源地址头。`RSV` 非零或 `FRAG` 非零的数据报 MUST 被丢弃而不转发。

#### Scenario: Relay request and reply

- **WHEN** 客户端发送 `FRAG=0` 且目标合法的 UDP 数据报
- **THEN** 中继转发负载到目标
- **AND** 目标的响应以带来源地址头的 UDP 数据报返回客户端

#### Scenario: Fragmented datagram dropped

- **WHEN** 收到 `FRAG` 非零的数据报
- **THEN** 中继丢弃该数据报
- **AND** 关联保持可用

### Requirement: Unsupported commands

服务 SHALL 对 `BIND` 及其他未知命令回复命令不支持（`0x07`）并关闭该会话。

#### Scenario: BIND rejected

- **WHEN** 客户端发送 `BIND`
- **THEN** 服务回复 `0x07`

### Requirement: Session isolation

每个接入的 TCP 会话 MUST 独立处理；单个会话的协议错误、超时或上游失败不得终止监听或影响其他会话。

#### Scenario: Malformed request

- **WHEN** 某客户端发送非法版本或截断的请求
- **THEN** 服务关闭该会话
- **AND** 监听继续保持，新的客户端仍可连接
