import { describe, expect, it } from 'vitest'

import {
  isFormValid,
  pickDefaultInterface,
  resolveInterfaceSelection,
  toggleTransport,
  validatePort,
} from './validation'
import type { InterfaceAddress } from '../types'

describe('validatePort', () => {
  it('accepts valid ports', () => {
    expect(validatePort('1080')).toEqual({ ok: true, value: 1080 })
    expect(validatePort('1')).toEqual({ ok: true, value: 1 })
    expect(validatePort('65535')).toEqual({ ok: true, value: 65535 })
  })

  it('rejects empty input', () => {
    expect(validatePort('')).toMatchObject({ ok: false, message: '请输入端口号' })
  })

  it('rejects out-of-range ports', () => {
    expect(validatePort('0').message).toBe('端口范围为 1–65535')
    expect(validatePort('65536').message).toBe('端口范围为 1–65535')
    expect(validatePort('99999').message).toBe('端口范围为 1–65535')
  })

  it('rejects non-integers including spaces and decimals', () => {
    for (const raw of ['abc', '10.5', ' 1080', '1080 ', '1e3', '+80', '-1']) {
      expect(validatePort(raw).message, raw).toBe('端口必须为整数')
    }
  })
})

describe('toggleTransport', () => {
  it('adds protocols in fixed order', () => {
    expect(toggleTransport(['tcp'], 'udp')).toEqual({
      next: ['tcp', 'udp'],
      blocked: false,
    })
  })

  it('removes a protocol when more than one remains', () => {
    expect(toggleTransport(['tcp', 'udp'], 'tcp')).toEqual({
      next: ['udp'],
      blocked: false,
    })
  })

  it('blocks removing the last protocol', () => {
    expect(toggleTransport(['tcp'], 'tcp')).toEqual({
      next: ['tcp'],
      blocked: true,
    })
  })
})

describe('pickDefaultInterface', () => {
  const base: InterfaceAddress = {
    id: 'a',
    name: 'A',
    ip: '127.0.0.1',
    family: 'ipv4',
    isLoopback: true,
  }
  const wildcard: InterfaceAddress = {
    id: 'all|0.0.0.0',
    name: '所有网卡',
    ip: '0.0.0.0',
    family: 'ipv4',
    isLoopback: false,
  }

  it('prefers a non-loopback ipv4 interface', () => {
    const result = pickDefaultInterface([
      wildcard,
      base,
      { ...base, id: 'b', ip: '::1', family: 'ipv6' },
      { ...base, id: 'c', ip: '192.168.1.2', isLoopback: false },
    ])
    expect(result?.id).toBe('c')
  })

  it('falls back to a non-loopback address', () => {
    const result = pickDefaultInterface([
      wildcard,
      base,
      { ...base, id: 'b', ip: '2001:db8::1', family: 'ipv6', isLoopback: false },
    ])
    expect(result?.id).toBe('b')
  })

  it('never defaults to the wildcard address when a real one exists', () => {
    expect(pickDefaultInterface([wildcard, base])?.id).toBe('a')
  })

  it('falls back to the first item', () => {
    expect(pickDefaultInterface([base])?.id).toBe('a')
    expect(pickDefaultInterface([])).toBeUndefined()
  })
})

describe('isFormValid', () => {
  it('requires endpoint, port and a protocol', () => {
    expect(isFormValid('a', '1080', ['tcp'])).toBe(true)
    expect(isFormValid('', '1080', ['tcp'])).toBe(false)
    expect(isFormValid('a', '0', ['tcp'])).toBe(false)
    expect(isFormValid('a', '1080', [])).toBe(false)
  })
})

describe('resolveInterfaceSelection', () => {
  const interfaces: InterfaceAddress[] = [
    { id: 'eth|192.168.1.2', name: 'eth', ip: '192.168.1.2', family: 'ipv4', isLoopback: false },
    { id: 'lo|127.0.0.1', name: 'lo', ip: '127.0.0.1', family: 'ipv4', isLoopback: true },
  ]

  it('keeps an existing selection', () => {
    expect(resolveInterfaceSelection('eth|192.168.1.2', interfaces)).toEqual({
      id: 'eth|192.168.1.2',
      notice: '',
    })
  })

  it('clears a vanished selection with a notice', () => {
    expect(resolveInterfaceSelection('gone|10.0.0.1', interfaces)).toEqual({
      id: '',
      notice: '原网卡地址已不存在，请重新选择网卡/IP',
    })
  })

  it('picks the default on first load', () => {
    expect(resolveInterfaceSelection('', interfaces)).toEqual({
      id: 'eth|192.168.1.2',
      notice: '',
    })
  })

  it('notices when no interface is available', () => {
    expect(resolveInterfaceSelection('', [])).toEqual({
      id: '',
      notice: '未发现可用网卡，请检查网络适配器',
    })
  })
})
