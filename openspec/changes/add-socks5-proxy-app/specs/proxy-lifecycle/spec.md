# Spec Delta

## Purpose

定义代理从启动、运行、停止到异常退出的完整生命周期，使用户始终清楚代理是否在监听、监听在哪里，以及操作失败的具体原因。

## ADDED Requirements

### Requirement: Validated start

系统 SHALL 在监听前校验：所选 IP 属于本机网卡、端口为 1–65535 的整数、至少启用一种传输协议。任一校验失败 MUST 拒绝启动、不创建监听套接字，并返回指明字段与原因的错误。

#### Scenario: Valid configuration starts listening

- **WHEN** 用户提交属于本机网卡的 IP、可用端口 1080，并勾选 TCP
- **THEN** 系统在该 IP 与端口上建立监听
- **AND** 返回状态 `running` 与监听端点

#### Scenario: Invalid port rejected

- **WHEN** 端口为 `0`、`65536`、非数字或为空
- **THEN** 启动被拒绝且不建立监听
- **AND** 错误信息指明端口字段及合法范围

#### Scenario: Foreign IP rejected

- **WHEN** 提交的 IP 不属于本机任何网卡
- **THEN** 启动被拒绝且不建立监听
- **AND** 错误信息指明该地址不可用

#### Scenario: No transport selected

- **WHEN** 未勾选 TCP 与 UDP 中任一协议
- **THEN** 启动被拒绝并提示至少选择一种协议

### Requirement: Single active proxy instance

同一应用进程 SHALL 最多同时运行一个代理实例。运行中再次发起启动时 MUST 被拒绝并保留现有实例，不得静默停止或替换。

#### Scenario: Start while running

- **WHEN** 代理处于 `running` 状态时再次收到启动请求
- **THEN** 请求返回“代理已在运行”类错误
- **AND** 现有监听与连接不受影响

### Requirement: Bind failure is reported

当监听地址被占用、无权限或不可绑定时，系统 MUST 保持 `stopped` 状态并返回包含端点与底层原因的可读错误。

#### Scenario: Port already in use

- **WHEN** 所选端口已被其他进程占用
- **THEN** 启动失败，状态保持 `stopped`
- **AND** 错误信息包含该端口与占用/不可用原因

### Requirement: Stop releases the endpoint

用户停止代理时，系统 SHALL 关闭监听端口、终止现有 TCP 会话与 UDP 关联，并在完成后返回 `stopped`；释放后的端口 MUST 可被立即重新绑定。

#### Scenario: Stop then rebind

- **WHEN** 运行中的代理被停止
- **THEN** 状态变为 `stopped` 且原端口不再监听
- **AND** 使用同一地址与端口可以立即重新启动成功

### Requirement: Status query and change events

系统 SHALL 提供当前代理状态查询，并在状态变化时推送事件。运行状态 MUST 包含端点、启动时间与已启用的传输协议。

#### Scenario: Status after restart of the UI

- **WHEN** 前端重新加载后查询状态
- **THEN** 返回与后端一致的 `running` 或 `stopped` 状态
- **AND** `running` 时包含端点、启动时间与协议集合

#### Scenario: Stop event delivered

- **WHEN** 任意原因导致代理从运行变为停止
- **THEN** 前端收到状态事件并切回未运行界面

### Requirement: Exit cleanup

应用窗口关闭或进程退出时，系统 MUST 停止代理并释放监听端口，不遗留后台监听进程。

#### Scenario: Quit application

- **WHEN** 用户在代理运行中关闭应用
- **THEN** 代理停止且端口被释放
- **AND** 应用退出后该端口可被其他程序使用
