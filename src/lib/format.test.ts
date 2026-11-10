import { describe, expect, it } from 'vitest'

import {
  filterInterfaceOption,
  formatEndpoint,
  formatUptime,
  groupInterfaces,
  normalizeError,
} from './format'
import type { InterfaceAddress } from '../types'

const interfaces: InterfaceAddress[] = [
  { id: '以太网|192.168.1.23', name: '以太网', ip: '192.168.1.23', family: 'ipv4', isLoopback: false },
  { id: 'Wi-Fi|192.168.31.8', name: 'Wi-Fi', ip: '192.168.31.8', family: 'ipv4', isLoopback: false },
  { id: 'Loopback|127.0.0.1', name: 'Loopback', ip: '127.0.0.1', family: 'ipv4', isLoopback: true },
]

describe('formatEndpoint', () => {
  it('formats ipv4 endpoints', () => {
    expect(formatEndpoint('127.0.0.1', 1080)).toBe('socks5://127.0.0.1:1080')
  })

  it('brackets ipv6 endpoints', () => {
    expect(formatEndpoint('::1', 1080)).toBe('socks5://[::1]:1080')
  })
})

describe('formatUptime', () => {
  it('returns zeros without a start time', () => {
    expect(formatUptime(null)).toBe('00:00:00')
  })

  it('formats hours, minutes and seconds', () => {
    expect(formatUptime(1_000, 1_000 + 3_723_000)).toBe('01:02:03')
  })

  it('never returns negative durations', () => {
    expect(formatUptime(2_000, 1_000)).toBe('00:00:00')
  })
})

describe('normalizeError', () => {
  it('passes through strings', () => {
    expect(normalizeError('端口被占用')).toBe('端口被占用')
  })

  it('reads message from error payloads', () => {
    expect(normalizeError({ code: 'bind_failed', message: '无法绑定' })).toBe('无法绑定')
  })

  it('falls back for unknown values', () => {
    expect(normalizeError(null)).toBe('操作失败，请稍后重试')
    expect(normalizeError({})).toBe('操作失败，请稍后重试')
  })
})

describe('groupInterfaces', () => {
  it('groups by interface name and keeps order', () => {
    const groups = groupInterfaces(interfaces)
    expect(groups.map((group) => group.label)).toEqual(['以太网', 'Wi-Fi', 'Loopback'])
    expect(groups[0].options[0]).toEqual({
      label: '192.168.1.23 · IPv4',
      value: '以太网|192.168.1.23',
    })
    expect(groups[2].options[0].label).toContain('回环')
  })
})

describe('filterInterfaceOption', () => {
  it('matches name, ip and family', () => {
    const option = { value: '以太网|192.168.1.23' }
    expect(filterInterfaceOption('以太', option, interfaces)).toBe(true)
    expect(filterInterfaceOption('192.168', option, interfaces)).toBe(true)
    expect(filterInterfaceOption('ipv4', option, interfaces)).toBe(true)
    expect(filterInterfaceOption('10.0.0.1', option, interfaces)).toBe(false)
  })

  it('matches loopback by Chinese label', () => {
    expect(filterInterfaceOption('回环', { value: 'Loopback|127.0.0.1' }, interfaces)).toBe(true)
  })
})
