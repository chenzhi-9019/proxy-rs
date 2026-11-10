import type { InterfaceAddress, Transport } from '../types'

export interface PortValidation {
  ok: boolean
  value?: number
  message?: string
}

/** 端口必须是 1–65535 的十进制整数；空值、空格、小数与非数字均非法。 */
export function validatePort(raw: string): PortValidation {
  if (raw.length === 0) {
    return { ok: false, message: '请输入端口号' }
  }
  if (!/^\d+$/.test(raw)) {
    return { ok: false, message: '端口必须为整数' }
  }
  const value = Number(raw)
  if (!Number.isSafeInteger(value) || value < 1 || value > 65535) {
    return { ok: false, message: '端口范围为 1–65535' }
  }
  return { ok: true, value }
}

/** 勾选/取消协议；取消最后一个已选项时保持原选择并返回 blocked。 */
export function toggleTransport(
  current: Transport[],
  target: Transport,
): { next: Transport[]; blocked: boolean } {
  if (current.includes(target)) {
    if (current.length === 1) {
      return { next: [...current], blocked: true }
    }
    return { next: current.filter((item) => item !== target), blocked: false }
  }
  const next = [...current, target].sort(
    (left, right) => transportOrder(left) - transportOrder(right),
  )
  return { next, blocked: false }
}

function transportOrder(transport: Transport): number {
  return transport === 'tcp' ? 0 : 1
}

/** 表单是否满足启动条件。 */
export function isFormValid(
  interfaceId: string,
  port: string,
  protocols: Transport[],
): boolean {
  return interfaceId.length > 0 && validatePort(port).ok && protocols.length > 0
}

/** 默认选中第一个非回环 IPv4；否则第一个非回环地址；再否则第一项。 */
export function pickDefaultInterface(
  interfaces: InterfaceAddress[],
): InterfaceAddress | undefined {
  return (
    interfaces.find((item) => !item.isLoopback && item.family === 'ipv4') ??
    interfaces.find((item) => !item.isLoopback) ??
    interfaces[0]
  )
}

export interface SelectionResolution {
  id: string
  notice: string
}

/**
 * 刷新网卡列表后决定选中项：
 * - 原选中项仍存在则保留；
 * - 原选中项消失则清空并提示重新选择；
 * - 首次加载则选择默认项，空列表给出提示。
 */
export function resolveInterfaceSelection(
  previousId: string,
  interfaces: InterfaceAddress[],
): SelectionResolution {
  if (previousId && interfaces.some((item) => item.id === previousId)) {
    return { id: previousId, notice: '' }
  }
  if (previousId) {
    return { id: '', notice: '原网卡地址已不存在，请重新选择网卡/IP' }
  }
  const fallback = pickDefaultInterface(interfaces)
  return {
    id: fallback?.id ?? '',
    notice: fallback ? '' : '未发现可用网卡，请检查网络适配器',
  }
}
