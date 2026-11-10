/** 与 Rust IPC 结构一一对应的前端类型（Rust 侧统一 camelCase）。 */

export type Transport = 'tcp' | 'udp'

export type AddressFamily = 'ipv4' | 'ipv6'

export interface InterfaceAddress {
  id: string
  name: string
  ip: string
  family: AddressFamily
  isLoopback: boolean
}

export interface ProxyConfig {
  interfaceName: string
  ip: string
  port: number
  protocols: Transport[]
}

export type ProxyState = 'stopped' | 'running'

export interface ProxyStatus {
  state: ProxyState
  endpoint: string | null
  config: ProxyConfig | null
  startedAt: number | null
  message: string | null
}

export interface AppErrorPayload {
  code: string
  message: string
}
