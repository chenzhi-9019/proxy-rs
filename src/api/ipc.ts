import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'

import type {
  InterfaceAddress,
  ProxyConfig,
  ProxyStatus,
} from '../types'

const inTauri =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

/** 浏览器里没有 Tauri 后端，dev 模式使用内存 mock 方便视觉联调。 */
const useMock = import.meta.env.DEV && !inTauri

async function mockBackend() {
  return import('./mock')
}

export async function listInterfaces(): Promise<InterfaceAddress[]> {
  if (useMock) {
    return (await mockBackend()).listInterfaces()
  }
  return invoke<InterfaceAddress[]>('list_network_interfaces')
}

export async function startProxy(config: ProxyConfig): Promise<ProxyStatus> {
  if (useMock) {
    return (await mockBackend()).startProxy(config)
  }
  return invoke<ProxyStatus>('start_proxy', { config })
}

export async function stopProxy(): Promise<ProxyStatus> {
  if (useMock) {
    return (await mockBackend()).stopProxy()
  }
  return invoke<ProxyStatus>('stop_proxy')
}

export async function getProxyStatus(): Promise<ProxyStatus> {
  if (useMock) {
    return (await mockBackend()).getProxyStatus()
  }
  return invoke<ProxyStatus>('get_proxy_status')
}

export async function onProxyStatus(
  handler: (status: ProxyStatus) => void,
): Promise<UnlistenFn> {
  if (useMock) {
    return (await mockBackend()).onStatus(handler)
  }
  return listen<ProxyStatus>('proxy://status', (event) => handler(event.payload))
}

export async function copyText(text: string): Promise<void> {
  if (!useMock) {
    try {
      await writeText(text)
      return
    } catch {
      // WebView2 下极少数策略会拒绝插件写入，回退到浏览器 API。
    }
  }
  await fallbackCopy(text)
}

async function fallbackCopy(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text)
    return
  }
  const area = document.createElement('textarea')
  area.value = text
  area.setAttribute('readonly', '')
  area.style.position = 'fixed'
  area.style.opacity = '0'
  document.body.appendChild(area)
  area.select()
  document.execCommand('copy')
  document.body.removeChild(area)
}
