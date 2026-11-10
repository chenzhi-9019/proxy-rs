/**
 * 仅用于浏览器 dev 模式的内存后端，让界面在没有 Tauri 运行时也能完整演示。
 * 生产构建不会包含该模块（由 import.meta.env.DEV 分支裁掉）。
 */
import type {
  InterfaceAddress,
  ProxyConfig,
  ProxyStatus,
} from '../types'

const interfaces: InterfaceAddress[] = [
  {
    id: '以太网|192.168.1.23',
    name: '以太网',
    ip: '192.168.1.23',
    family: 'ipv4',
    isLoopback: false,
  },
  {
    id: 'Wi-Fi|192.168.31.8',
    name: 'Wi-Fi',
    ip: '192.168.31.8',
    family: 'ipv4',
    isLoopback: false,
  },
  {
    id: '以太网 2|10.8.0.6',
    name: '以太网 2',
    ip: '10.8.0.6',
    family: 'ipv4',
    isLoopback: false,
  },
  {
    id: 'Loopback|127.0.0.1',
    name: 'Loopback',
    ip: '127.0.0.1',
    family: 'ipv4',
    isLoopback: true,
  },
  {
    id: 'Loopback|::1',
    name: 'Loopback',
    ip: '::1',
    family: 'ipv6',
    isLoopback: true,
  },
]

let status: ProxyStatus = {
  state: 'stopped',
  endpoint: null,
  config: null,
  startedAt: null,
  message: null,
}

const listeners = new Set<(value: ProxyStatus) => void>()

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, milliseconds)
  })
}

function notify(): void {
  for (const listener of listeners) {
    listener(status)
  }
}

function endpointFor(config: ProxyConfig): string {
  return config.ip.includes(':')
    ? `socks5://[${config.ip}]:${config.port}`
    : `socks5://${config.ip}:${config.port}`
}

export async function listInterfaces(): Promise<InterfaceAddress[]> {
  await delay(160)
  return [...interfaces]
}

export async function startProxy(config: ProxyConfig): Promise<ProxyStatus> {
  await delay(650)
  if (status.state === 'running') {
    throw { code: 'already_running', message: '代理已在运行，请先停止当前实例' }
  }
  if (config.port === 1) {
    throw {
      code: 'bind_failed',
      message: `无法在 ${config.ip}:${config.port} 上启动代理：端口被占用`,
    }
  }
  status = {
    state: 'running',
    endpoint: endpointFor(config),
    config: { ...config, protocols: [...config.protocols] },
    startedAt: Date.now(),
    message: null,
  }
  notify()
  return status
}

export async function stopProxy(): Promise<ProxyStatus> {
  await delay(320)
  status = {
    state: 'stopped',
    endpoint: null,
    config: null,
    startedAt: null,
    message: null,
  }
  notify()
  return status
}

export async function getProxyStatus(): Promise<ProxyStatus> {
  return status
}

export function onStatus(handler: (value: ProxyStatus) => void): () => void {
  listeners.add(handler)
  return () => {
    listeners.delete(handler)
  }
}
