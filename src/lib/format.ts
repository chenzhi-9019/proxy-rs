import type { InterfaceAddress } from '../types'

export interface InterfaceSelectGroup {
  label: string
  options: Array<{ label: string; value: string }>
}

/** 把网卡地址按网卡名分组，供下拉框使用。 */
export function groupInterfaces(interfaces: InterfaceAddress[]): InterfaceSelectGroup[] {
  const groups = new Map<string, InterfaceSelectGroup>()
  for (const item of interfaces) {
    let group = groups.get(item.name)
    if (!group) {
      group = { label: item.name, options: [] }
      groups.set(item.name, group)
    }
    group.options.push({ label: optionLabel(item), value: item.id })
  }
  return [...groups.values()]
}

function optionLabel(item: InterfaceAddress): string {
  if (isWildcardInterface(item)) {
    return '0.0.0.0 · 所有网卡'
  }
  const family = item.family === 'ipv4' ? 'IPv4' : 'IPv6'
  const loopback = item.isLoopback ? ' · 回环' : ''
  return `${item.ip} · ${family}${loopback}`
}

/** `0.0.0.0` 是通用的“监听所有网卡”选项，不是真实网卡地址。 */
export function isWildcardInterface(item: InterfaceAddress): boolean {
  return item.ip === '0.0.0.0'
}

/** 按名称、IP 或地址族过滤下拉项。 */
export function filterInterfaceOption(
  input: string,
  option: { value?: unknown } | undefined,
  interfaces: InterfaceAddress[],
): boolean {
  const query = input.trim().toLowerCase()
  if (!query) {
    return true
  }
  const item = interfaces.find((entry) => entry.id === option?.value)
  if (!item) {
    return false
  }
  return (
    item.name.toLowerCase().includes(query) ||
    item.ip.toLowerCase().includes(query) ||
    item.family.includes(query) ||
    (item.isLoopback && ('回环'.includes(query) || 'loopback'.includes(query)))
  )
}

/** 后端返回的 endpoint 优先；此函数用于前端兜底与测试。 */
export function formatEndpoint(ip: string, port: number): string {
  return ip.includes(':') ? `socks5://[${ip}]:${port}` : `socks5://${ip}:${port}`
}

/** 把毫秒时长格式化为 `HH:MM:SS`。 */
export function formatUptime(startedAt: number | null, now: number = Date.now()): string {
  if (!startedAt) {
    return '00:00:00'
  }
  const totalSeconds = Math.max(0, Math.floor((now - startedAt) / 1000))
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  return [hours, minutes, seconds].map((value) => String(value).padStart(2, '0')).join(':')
}

/** 把 Tauri 命令错误或任意异常归一为可展示的中文文本。 */
export function normalizeError(error: unknown): string {
  if (typeof error === 'string' && error.trim()) {
    return error
  }
  if (error && typeof error === 'object') {
    const message = (error as { message?: unknown }).message
    if (typeof message === 'string' && message.trim()) {
      return message
    }
  }
  return '操作失败，请稍后重试'
}
