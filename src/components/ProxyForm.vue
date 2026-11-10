<script setup lang="ts">
import { computed } from 'vue'
import {
  Alert as AAlert,
  Button as AButton,
  Checkbox as ACheckbox,
  Input as AInput,
  Select as ASelect,
} from 'ant-design-vue'
import {
  ApiOutlined,
  CheckOutlined,
  ReloadOutlined,
  SendOutlined,
} from '@ant-design/icons-vue'

import {
  filterInterfaceOption,
  groupInterfaces,
} from '../lib/format'
import type { InterfaceAddress, Transport } from '../types'

const props = defineProps<{
  interfaces: InterfaceAddress[]
  loading: boolean
  disabled: boolean
  running: boolean
  portError: string
  transportHint: string
  notice: string
  showWarning: boolean
}>()

const emit = defineEmits<{
  refresh: []
  'toggle-protocol': [transport: Transport]
}>()

const endpointId = defineModel<string>('endpointId', { required: true })
const port = defineModel<string>('port', { required: true })
const protocols = defineModel<Transport[]>('protocols', { required: true })

const groupedOptions = computed(() => groupInterfaces(props.interfaces))
const selected = computed(() =>
  props.interfaces.find((item) => item.id === endpointId.value),
)

const warningMessage = computed(() => {
  const address = selected.value
    ? `${selected.value.ip}${port.value ? `:${port.value}` : ''}`
    : '所选地址'
  return `无认证代理 · ${address} 对同一局域网内的设备可见，请仅在可信网络使用`
})

function filterOption(
  input: string,
  option?: { value?: unknown },
): boolean {
  return filterInterfaceOption(input, option, props.interfaces)
}

function hasProtocol(transport: Transport): boolean {
  return protocols.value.includes(transport)
}

function toggle(transport: Transport): void {
  emit('toggle-protocol', transport)
}
</script>

<template>
  <div
    class="proxy-form"
    :class="{ 'is-locked': disabled, 'is-running': running }"
  >
    <section class="form-row">
      <div class="row-heading">
        <span class="step-index" aria-hidden="true">01</span>
        <div class="row-title">
          <label for="endpoint-select">网卡与地址</label>
          <p>代理将只监听这个 IP</p>
        </div>
        <AButton
          class="refresh-button"
          type="text"
          html-type="button"
          :loading="loading"
          :disabled="disabled || loading"
          @click="emit('refresh')"
        >
          <ReloadOutlined />
          刷新
        </AButton>
      </div>

      <ASelect
        id="endpoint-select"
        v-model:value="endpointId"
        class="field-select"
        :options="groupedOptions"
        :filter-option="filterOption"
        :disabled="disabled"
        :loading="loading"
        show-search
        placeholder="选择网卡与 IP"
        :not-found-content="loading ? '正在读取网卡…' : '没有匹配的网卡地址'"
        aria-label="网卡与地址"
      />

      <p v-if="selected" class="field-hint">
        已选：{{ selected.name }} · {{ selected.ip }}
        <span v-if="selected.isLoopback" class="loopback-tag">回环</span>
      </p>
      <p v-if="notice" class="field-hint is-error" role="alert">
        {{ notice }}
      </p>
    </section>

    <section class="form-row">
      <div class="row-heading">
        <span class="step-index" aria-hidden="true">02</span>
        <div class="row-title">
          <label for="proxy-port">端口</label>
          <p>1–65535，建议 1080 及以上</p>
        </div>
      </div>

      <AInput
        id="proxy-port"
        v-model:value="port"
        class="field-input"
        inputmode="numeric"
        autocomplete="off"
        :maxlength="5"
        placeholder="1080"
        :disabled="disabled"
        :status="portError ? 'error' : undefined"
        :aria-invalid="Boolean(portError)"
        aria-describedby="port-help"
      />
      <p
        id="port-help"
        class="field-hint"
        :class="{ 'is-error': portError }"
        aria-live="polite"
      >
        {{ portError || '端口会与所选 IP 一起组成 SOCKS5 端点' }}
      </p>
    </section>

    <section class="form-row form-row-last">
      <div class="row-heading">
        <span class="step-index" aria-hidden="true">03</span>
        <div class="row-title">
          <span id="transport-label">传输协议</span>
          <p>至少保留一种，可同时启用</p>
        </div>
      </div>

      <div
        class="transport-grid"
        role="group"
        aria-labelledby="transport-label"
      >
        <ACheckbox
          class="transport-chip"
          :class="{ selected: hasProtocol('tcp'), 'is-disabled': disabled }"
          :checked="hasProtocol('tcp')"
          :disabled="disabled"
          @change="toggle('tcp')"
        >
          <span class="chip-icon" aria-hidden="true">
            <ApiOutlined />
          </span>
          <span class="chip-copy">
            <strong>TCP</strong>
            <small>网页 / SSH 等连接</small>
          </span>
          <span class="chip-check" aria-hidden="true">
            <CheckOutlined />
          </span>
        </ACheckbox>

        <ACheckbox
          class="transport-chip"
          :class="{ selected: hasProtocol('udp'), 'is-disabled': disabled }"
          :checked="hasProtocol('udp')"
          :disabled="disabled"
          @change="toggle('udp')"
        >
          <span class="chip-icon" aria-hidden="true">
            <SendOutlined />
          </span>
          <span class="chip-copy">
            <strong>UDP</strong>
            <small>DNS / 游戏等数据报</small>
          </span>
          <span class="chip-check" aria-hidden="true">
            <CheckOutlined />
          </span>
        </ACheckbox>
      </div>

      <p
        v-if="transportHint"
        class="field-hint is-error"
        role="alert"
      >
        {{ transportHint }}
      </p>

      <AAlert
        v-if="showWarning"
        class="lan-alert"
        type="warning"
        show-icon
        :message="warningMessage"
      />
    </section>
  </div>
</template>

<style scoped>
.proxy-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.proxy-form.is-running .row-title p,
.proxy-form.is-running .field-hint:not(.is-error) {
  display: none;
}

.form-row {
  display: flex;
  flex-direction: column;
  gap: 9px;
  padding-bottom: 14px;
  border-bottom: 1px dashed var(--border);
}

.form-row-last {
  padding-bottom: 0;
  border-bottom: none;
}

.row-heading {
  display: flex;
  align-items: flex-start;
  gap: 10px;
}

.step-index {
  display: inline-grid;
  width: 26px;
  height: 26px;
  flex: 0 0 auto;
  place-items: center;
  border-radius: 9px;
  background: rgba(11, 122, 110, 0.08);
  color: var(--teal-600);
  font-family: 'Outfit Variable', sans-serif;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.02em;
}

.row-title {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 1px;
}

.row-title label,
.row-title > span {
  color: var(--ink-900);
  font-size: 13.5px;
  font-weight: 600;
  line-height: 1.3;
}

.row-title p {
  margin: 0;
  color: var(--ink-400);
  font-size: 11.5px;
  line-height: 1.4;
}

.refresh-button {
  height: 28px;
  margin-left: auto;
  padding: 0 10px;
  border-radius: 9px;
  background: rgba(11, 122, 110, 0.06);
  color: var(--teal-600);
  font-size: 12px;
  font-weight: 600;
  transition: background-color 180ms var(--ease-out);
}

.refresh-button:hover {
  background: rgba(11, 122, 110, 0.12);
}

.field-select,
.field-input {
  width: 100%;
}

.field-hint {
  margin: 0;
  color: var(--ink-400);
  font-size: 11.5px;
  line-height: 1.45;
}

.field-hint.is-error {
  color: var(--danger);
}

.loopback-tag {
  display: inline-block;
  margin-left: 6px;
  padding: 1px 6px;
  border-radius: 6px;
  background: rgba(11, 31, 36, 0.06);
  color: var(--ink-600);
  font-size: 10.5px;
  font-weight: 600;
}

.transport-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  margin-top: 2px;
}

.transport-chip {
  position: relative;
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 62px;
  margin: 0;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.72);
  transition:
    border-color 180ms var(--ease-out),
    background-color 180ms var(--ease-out),
    box-shadow 180ms var(--ease-out),
    transform 180ms var(--ease-out);
}

.transport-chip:hover {
  border-color: rgba(11, 122, 110, 0.28);
  transform: translateY(-1px);
}

.transport-chip.selected {
  border-color: rgba(11, 122, 110, 0.55);
  background: linear-gradient(
    135deg,
    rgba(52, 231, 196, 0.16),
    rgba(53, 184, 245, 0.1)
  );
  box-shadow: 0 8px 22px rgba(11, 122, 110, 0.1);
}

.transport-chip:focus-within {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.transport-chip :deep(.ant-checkbox) {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  opacity: 0;
}

.transport-chip :deep(.ant-checkbox + span) {
  padding: 0;
  width: 100%;
}

.chip-icon {
  display: grid;
  width: 34px;
  height: 34px;
  flex: 0 0 auto;
  place-items: center;
  border-radius: 11px;
  background: rgba(11, 122, 110, 0.08);
  color: var(--teal-600);
  font-size: 16px;
  transition:
    background 180ms var(--ease-out),
    color 180ms var(--ease-out);
}

.transport-chip.selected .chip-icon {
  background: linear-gradient(135deg, var(--start-a), var(--start-b));
  color: #04262b;
}

.chip-copy {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 1px;
}

.chip-copy strong {
  color: var(--ink-900);
  font-size: 13.5px;
  font-weight: 700;
  letter-spacing: 0.02em;
}

.chip-copy small {
  overflow: hidden;
  color: var(--ink-400);
  font-size: 10.5px;
  line-height: 1.35;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.chip-check {
  position: absolute;
  top: 9px;
  right: 9px;
  display: grid;
  width: 17px;
  height: 17px;
  place-items: center;
  border-radius: 50%;
  background: var(--teal-600);
  color: #fff;
  font-size: 9px;
  opacity: 0;
  transform: scale(0.6);
  transition:
    opacity 160ms var(--ease-out),
    transform 160ms var(--ease-out);
}

.transport-chip.selected .chip-check {
  opacity: 1;
  transform: scale(1);
}

.transport-chip.is-disabled {
  cursor: not-allowed;
  opacity: 0.66;
}

.transport-chip.is-disabled:hover {
  border-color: var(--border);
  transform: none;
}

.lan-alert {
  margin-top: 2px;
  border-radius: 12px;
}

.lan-alert :deep(.ant-alert-message) {
  color: #8a4b00;
  font-size: 11.5px;
  line-height: 1.5;
}

.proxy-form.is-locked .form-row {
  opacity: 0.86;
}

@media (max-width: 560px) {
  .transport-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-height: 700px) {
  .row-title p {
    display: none;
  }
}
</style>
