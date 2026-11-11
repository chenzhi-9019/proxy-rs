# Spec Delta

## Purpose

让应用发现本机全部网卡及其 IP 地址，并把可绑定的端点以稳定、可读的形式提供给界面，作为代理监听地址的唯一来源。

## ADDED Requirements

### Requirement: Enumerate all local network interfaces

系统 SHALL 枚举本机所有网络接口及其 IPv4 与 IPv6 地址，包括回环与虚拟网卡，且不要求管理员权限。每个地址项 MUST 至少包含网卡展示名、IP 地址、地址族与是否回环的标识。IPv6 链路本地地址因缺少作用域标识无法可靠绑定，MUST NOT 作为可选项返回。

#### Scenario: Enumerate at startup

- **WHEN** 应用启动并请求网卡列表
- **THEN** 返回本机全部网卡的地址项，同一网卡有多个地址时返回多个独立地址项
- **AND** 每个地址项包含网卡名、IP、`IPv4`/`IPv6` 地址族与回环标识

#### Scenario: Interface without usable address

- **WHEN** 某网卡当前没有分配任何 IP 地址
- **THEN** 该网卡不出现在可选项列表中
- **AND** 其余网卡仍正常返回，不因单个网卡异常而整体失败

#### Scenario: Enumeration failure

- **WHEN** 系统无法读取网卡信息
- **THEN** 返回可读的失败结果
- **AND** 界面显示失败提示与重试入口，开始代理按钮保持禁用

### Requirement: Stable endpoint identity

每个可选端点 SHALL 具有由网卡名与 IP 地址共同组成的稳定标识；同一网卡同一地址在多次枚举之间 MUST 保持标识不变。

#### Scenario: Identity stable across refresh

- **WHEN** 用户刷新网卡列表且某地址仍然存在
- **THEN** 该地址的标识与刷新前一致
- **AND** 之前选中的地址仍可被识别为同一端点

#### Scenario: Duplicate addresses

- **WHEN** 两张网卡拥有相同的 IP 地址
- **THEN** 两个地址项都保留并以网卡名区分
- **AND** 标识不冲突

### Requirement: All-interfaces wildcard option

系统 SHALL 在可选地址列表中提供通用地址 `0.0.0.0`，展示为“所有网卡”，用于监听本机全部 IPv4 网卡。该项 MUST 与真实网卡项一样参与选择、刷新、状态回显，并标记为非回环。

#### Scenario: Select wildcard address

- **WHEN** 用户选择 `0.0.0.0 · 所有网卡`
- **THEN** 启动参数包含 `ip = 0.0.0.0`
- **AND** 代理监听本机全部 IPv4 网卡
- **AND** 界面提示本机客户端使用 `127.0.0.1`

#### Scenario: Default selection avoids wildcard

- **WHEN** 本机存在真实网卡地址
- **THEN** 首次加载默认选中真实非回环 IPv4 地址
- **AND** 不自动选择 `0.0.0.0`

### Requirement: On-demand refresh

界面 SHALL 允许用户在不重启应用的情况下重新枚举网卡；刷新 MUST 返回当前最新的地址集合。

#### Scenario: Refresh picks up network change

- **WHEN** 用户点击刷新且本机新增或移除了 IP 地址
- **THEN** 返回的列表反映最新状态
- **AND** 刷新过程中界面显示加载状态且开始按钮不可用
