import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import {
  copyText,
  getProxyStatus,
  listInterfaces,
  onProxyStatus,
  startProxy as invokeStartProxy,
  stopProxy as invokeStopProxy,
} from '../api/ipc'
import { formatUptime, normalizeError } from '../lib/format'
import {
  isFormValid,
  resolveInterfaceSelection,
  toggleTransport,
  validatePort,
} from '../lib/validation'
import type {
  InterfaceAddress,
  ProxyConfig,
  ProxyStatus,
  Transport,
} from '../types'

const EMPTY_STATUS: ProxyStatus = {
  state: 'stopped',
  endpoint: null,
  config: null,
  startedAt: null,
  message: null,
}

export function useProxy() {
  const interfaces = ref<InterfaceAddress[]>([])
  const endpointId = ref('')
  const port = ref('1080')
  const protocols = ref<Transport[]>(['tcp'])

  const status = ref<ProxyStatus>({ ...EMPTY_STATUS })
  const loadingInterfaces = ref(false)
  const starting = ref(false)
  const stopping = ref(false)
  const error = ref('')
  const interfaceNotice = ref('')
  const transportHint = ref('')
  const copied = ref(false)
  const now = ref(Date.now())

  let timer: number | undefined
  let copiedTimer: number | undefined
  let unlisten: (() => void) | undefined
  let disposed = false

  const selectedInterface = computed(() =>
    interfaces.value.find((item) => item.id === endpointId.value),
  )
  const portValidation = computed(() => validatePort(port.value))
  const portError = computed(() => (portValidation.value.ok ? '' : portValidation.value.message ?? ''))
  const running = computed(() => status.value.state === 'running')
  const startingUp = computed(() => starting.value)
  const stoppingDown = computed(() => stopping.value)
  const busy = computed(
    () => loadingInterfaces.value || starting.value || stopping.value,
  )
  const formDisabled = computed(() => running.value || busy.value)
  const formValid = computed(() =>
    isFormValid(endpointId.value, port.value, protocols.value),
  )
  const canStart = computed(
    () => !running.value && !busy.value && formValid.value,
  )
  const lanWarning = computed(
    () => !!selectedInterface.value && !selectedInterface.value.isLoopback,
  )
  const uptime = computed(() => formatUptime(status.value.startedAt, now.value))
  const statusLabel = computed(() => {
    if (stopping.value) return '停止中'
    if (running.value) return '运行中'
    return '未运行'
  })

  function setNow(): void {
    now.value = Date.now()
  }

  function startTimer(): void {
    stopTimer()
    setNow()
    timer = window.setInterval(setNow, 1000)
  }

  function stopTimer(): void {
    if (timer !== undefined) {
      window.clearInterval(timer)
      timer = undefined
    }
  }

  function applyStatus(next: ProxyStatus): void {
    status.value = next
    if (next.state === 'running' && next.config) {
      port.value = String(next.config.port)
      protocols.value = [...next.config.protocols]
      const match =
        interfaces.value.find(
          (item) =>
            item.ip === next.config?.ip &&
            item.name === next.config?.interfaceName,
        ) ?? interfaces.value.find((item) => item.ip === next.config?.ip)
      endpointId.value =
        match?.id ?? `${next.config.interfaceName}|${next.config.ip}`
      interfaceNotice.value = ''
      startTimer()
    } else {
      stopTimer()
      if (next.message) {
        error.value = next.message
      }
    }
  }

  async function loadInterfaces(manual = false): Promise<void> {
    loadingInterfaces.value = true
    if (manual) {
      interfaceNotice.value = ''
    }
    try {
      const list = await listInterfaces()
      if (disposed) return
      interfaces.value = list
      const resolution = resolveInterfaceSelection(endpointId.value, list)
      endpointId.value = resolution.id
      interfaceNotice.value = resolution.notice
    } catch (cause) {
      if (!disposed) {
        interfaces.value = []
        error.value = normalizeError(cause)
      }
    } finally {
      if (!disposed) {
        loadingInterfaces.value = false
      }
    }
  }

  async function initialize(): Promise<void> {
    await loadInterfaces()
    try {
      applyStatus(await getProxyStatus())
    } catch (cause) {
      error.value = normalizeError(cause)
    }
    try {
      unlisten = await onProxyStatus(applyStatus)
    } catch {
      // 事件订阅失败不阻塞首屏；状态仍可通过重新加载恢复。
    }
  }

  async function startProxyAction(): Promise<void> {
    if (running.value || starting.value || stopping.value) return
    error.value = ''
    transportHint.value = ''
    if (!formValid.value) {
      if (!selectedInterface.value) {
        interfaceNotice.value = '请选择网卡/IP'
      }
      return
    }
    const selected = selectedInterface.value
    const validated = portValidation.value
    if (!selected || !validated.ok || validated.value === undefined) {
      return
    }

    starting.value = true
    try {
      const config: ProxyConfig = {
        interfaceName: selected.name,
        ip: selected.ip,
        port: validated.value,
        protocols: [...protocols.value],
      }
      applyStatus(await invokeStartProxy(config))
    } catch (cause) {
      error.value = normalizeError(cause)
    } finally {
      starting.value = false
    }
  }

  async function stopProxyAction(): Promise<void> {
    if (!running.value || stopping.value) return
    error.value = ''
    stopping.value = true
    try {
      applyStatus(await invokeStopProxy())
    } catch (cause) {
      error.value = normalizeError(cause)
    } finally {
      stopping.value = false
    }
  }

  function handlePrimary(): void {
    if (running.value) {
      void stopProxyAction()
    } else {
      void startProxyAction()
    }
  }

  function toggleProtocol(target: Transport): void {
    if (formDisabled.value) return
    const result = toggleTransport(protocols.value, target)
    if (result.blocked) {
      transportHint.value = '至少选择一种协议'
      return
    }
    protocols.value = result.next
    transportHint.value = ''
  }

  function refreshInterfaces(): void {
    if (formDisabled.value) return
    void loadInterfaces(true)
  }

  async function copyEndpoint(): Promise<void> {
    if (!status.value.endpoint) return
    try {
      await copyText(status.value.endpoint)
      copied.value = true
      if (copiedTimer !== undefined) {
        window.clearTimeout(copiedTimer)
      }
      copiedTimer = window.setTimeout(() => {
        copied.value = false
      }, 1500)
    } catch (cause) {
      error.value = normalizeError(cause)
    }
  }

  function clearError(): void {
    error.value = ''
  }

  onMounted(() => {
    void initialize()
  })

  watch(endpointId, (value) => {
    if (value) {
      interfaceNotice.value = ''
    }
  })

  onBeforeUnmount(() => {
    disposed = true
    stopTimer()
    if (copiedTimer !== undefined) {
      window.clearTimeout(copiedTimer)
    }
    unlisten?.()
  })

  return {
    interfaces,
    endpointId,
    port,
    protocols,
    status,
    loadingInterfaces,
    starting: startingUp,
    stopping: stoppingDown,
    busy,
    formDisabled,
    formValid,
    canStart,
    running,
    selectedInterface,
    portError,
    lanWarning,
    uptime,
    statusLabel,
    error,
    interfaceNotice,
    transportHint,
    copied,
    handlePrimary,
    toggleProtocol,
    refreshInterfaces,
    copyEndpoint,
    clearError,
  }
}
