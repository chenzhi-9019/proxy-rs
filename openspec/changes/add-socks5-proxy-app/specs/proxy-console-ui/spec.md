# Spec Delta

## Purpose

定义单页代理控制台的交互与视觉契约，让用户在三次输入内完成配置，并清晰感知未运行、运行中、失败三种状态。

## ADDED Requirements

### Requirement: Three-row configuration form

控制台 SHALL 只包含三行配置输入：网卡/IP 下拉选择、端口号输入、TCP/UDP 多选；三行在默认窗口尺寸（980×720）下无需滚动即可完整显示，且每行具有可见标签。

#### Scenario: Default layout

- **WHEN** 应用以默认尺寸打开
- **THEN** 三行输入与开始按钮全部在首屏可见
- **AND** 每行标签分别说明网卡/IP、端口、传输协议

### Requirement: Port validation feedback

端口输入 SHALL 仅接受 1–65535 的整数，并在输入非法时给出就地错误提示且禁用开始按钮；合法时 MUST 清除该字段错误。空值、小数、非数字、越界值均视为非法。

#### Scenario: Out-of-range port

- **WHEN** 用户输入 `0` 或 `65536`
- **THEN** 端口字段显示范围错误
- **AND** 开始按钮不可用

#### Scenario: Non-integer port

- **WHEN** 用户输入 `10.5`、`abc` 或带空格的文本
- **THEN** 端口字段显示“必须为整数”类错误

#### Scenario: Valid port

- **WHEN** 用户输入 `1080`
- **THEN** 端口字段无错误，表单重新参与可用性判断

### Requirement: At least one transport

传输协议行 SHALL 提供 TCP 与 UDP 两个可多选项。用户取消勾选最后一个已选项时，界面 MUST 保留至少一个选中项并给出提示，表单不得出现零协议状态。

#### Scenario: Both selected

- **WHEN** 用户同时勾选 TCP 与 UDP
- **THEN** 两项均显示为选中态
- **AND** 启动参数包含两种协议

#### Scenario: Prevent empty selection

- **WHEN** 仅 TCP 被选中且用户尝试取消 TCP
- **THEN** TCP 保持选中
- **AND** 显示“至少选择一种协议”的就地提示

### Requirement: Idle start control

未运行且表单合法时，主按钮 SHALL 以蓝绿色为主视觉并显示“开始代理”；点击后进入启动中状态，按钮在请求完成前不可重复点击。

#### Scenario: Start from idle

- **WHEN** 表单合法且代理未运行
- **THEN** 主按钮为蓝绿色渐变并显示“开始代理”
- **AND** 点击后显示加载态，直到启动成功或失败

#### Scenario: Invalid form blocks start

- **WHEN** 存在未选择网卡、非法端口等错误
- **THEN** 主按钮为禁用态
- **AND** 点击不会发起启动请求

### Requirement: Active running control

代理运行期间，主按钮 MUST 切换为红黄色激活态并显示“停止代理”；配置区锁定为不可编辑，界面展示监听端点与持续更新的运行时长，点击按钮停止代理。

#### Scenario: Enter running state

- **WHEN** 启动成功
- **THEN** 主按钮变为红黄色“停止代理”
- **AND** 三行配置不可编辑，状态区显示端点与运行时长

#### Scenario: Stop from running

- **WHEN** 用户点击“停止代理”
- **THEN** 界面先显示停止中，随后回到未运行的蓝绿色开始态
- **AND** 配置区恢复可编辑

#### Scenario: Copy endpoint

- **WHEN** 用户点击端点旁的复制按钮
- **THEN** 完整端点文本被写入剪贴板并提示复制成功

### Requirement: Feedback for failures

启动失败、停止失败与状态事件异常 SHALL 以可见提示呈现，包含可读原因；字段级错误 MUST 定位到对应输入行，不得仅写入控制台。

#### Scenario: Start failure

- **WHEN** 端口被占用导致启动失败
- **THEN** 界面显示错误提示并包含端口与原因
- **AND** 界面保持未运行的开始态，可修改配置后重试

### Requirement: LAN exposure warning

当所选 IP 不是回环地址时，界面 MUST 显示无认证代理可被局域网访问的风险提示；选择回环地址时该提示隐藏。

#### Scenario: Non-loopback selected

- **WHEN** 用户选择 `192.168.x.x` 等非回环地址
- **THEN** 在三行输入附近显示醒目的无认证暴露提示

#### Scenario: Loopback selected

- **WHEN** 用户选择 `127.0.0.1` 或 `::1`
- **THEN** 风险提示不显示

### Requirement: Interface refresh and selection retention

网卡/IP 下拉 SHALL 提供刷新操作并显示加载状态；刷新后若原选中项仍存在，MUST 保持选中；若已消失，MUST 清空选择并提示重新选择。

#### Scenario: Refresh keeps selection

- **WHEN** 刷新后所选网卡地址仍在本机
- **THEN** 下拉保持原选中项

#### Scenario: Selected address disappears

- **WHEN** 刷新后所选地址已不存在
- **THEN** 选择被清空
- **AND** 提示用户重新选择网卡/IP

### Requirement: Energetic visual identity

界面 MUST 呈现年轻、有活力的视觉风格：蓝绿色渐变用于主操作，红黄色渐变仅用于运行中的激活操作，二者在任意状态下都不得互换。正文与背景对比度 MUST 满足 WCAG AA，交互元素 MUST 有可见键盘焦点。

#### Scenario: Visual state distinction

- **WHEN** 代理从未运行切换到运行中
- **THEN** 主按钮颜色从蓝绿色系变为红黄色系，无第三种强调色混用

#### Scenario: Keyboard operation

- **WHEN** 用户使用 Tab 与 Enter 操作表单和主按钮
- **THEN** 每个可交互元素获得可见焦点
- **AND** 可完成选择、启动与停止流程

### Requirement: Responsive min-size layout

窗口缩至最小尺寸（860×640）时，三行输入与主按钮 SHALL 仍可操作，内容不得被裁切或产生横向滚动。

#### Scenario: Minimum window size

- **WHEN** 用户把窗口缩到 860×640
- **THEN** 表单与按钮完整可用
- **AND** 页面不出现横向滚动条
