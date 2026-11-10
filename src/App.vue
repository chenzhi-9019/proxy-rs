<script setup lang="ts">
import { computed } from 'vue'
import {
  Alert as AAlert,
  ConfigProvider as AConfigProvider,
} from 'ant-design-vue'
import zhCN from 'ant-design-vue/es/locale/zh_CN'

import ActionButton from './components/ActionButton.vue'
import BrandHeader from './components/BrandHeader.vue'
import ProxyForm from './components/ProxyForm.vue'
import RunPanel from './components/RunPanel.vue'
import { useProxy } from './composables/useProxy'
import { theme } from './theme'

const {
  interfaces,
  endpointId,
  port,
  protocols,
  status,
  loadingInterfaces,
  starting,
  stopping,
  formDisabled,
  canStart,
  running,
  portError,
  lanWarning,
  uptime,
  error,
  interfaceNotice,
  transportHint,
  copied,
  handlePrimary,
  toggleProtocol,
  refreshInterfaces,
  copyEndpoint,
  clearError,
} = useProxy()

const primaryMode = computed<'start' | 'starting' | 'stop' | 'stopping'>(() => {
  if (running.value) {
    return stopping.value ? 'stopping' : 'stop'
  }
  return starting.value ? 'starting' : 'start'
})

const actionDisabled = computed(() =>
  running.value ? stopping.value : !canStart.value,
)
</script>

<template>
  <AConfigProvider :locale="zhCN" :theme="theme">
    <div class="app-shell">
      <div class="app-glow app-glow--mint" aria-hidden="true" />
      <div class="app-glow app-glow--sky" aria-hidden="true" />
      <div class="app-glow app-glow--lemon" aria-hidden="true" />

      <main class="app-content">
        <BrandHeader :running="running" :stopping="stopping" />

        <section class="console-card" aria-label="代理配置">
          <ProxyForm
            v-model:endpoint-id="endpointId"
            v-model:port="port"
            v-model:protocols="protocols"
            :interfaces="interfaces"
            :loading="loadingInterfaces"
            :disabled="formDisabled"
            :running="running"
            :port-error="portError"
            :transport-hint="transportHint"
            :notice="interfaceNotice"
            :show-warning="lanWarning"
            @refresh="refreshInterfaces"
            @toggle-protocol="toggleProtocol"
          />

          <Transition name="alert">
            <AAlert
              v-if="error"
              class="error-alert"
              type="error"
              show-icon
              closable
              :message="error"
              @close="clearError"
            />
          </Transition>

          <ActionButton
            :mode="primaryMode"
            :disabled="actionDisabled"
            @click="handlePrimary"
          />
        </section>

        <RunPanel
          :visible="running"
          :endpoint="status.endpoint"
          :uptime="uptime"
          :copied="copied"
          @copy="copyEndpoint"
        />

        <footer class="app-footer">
          <span class="footer-dot" aria-hidden="true" />
          仅绑定所选网卡 · 无认证 SOCKS5 · 不修改系统代理
        </footer>
      </main>
    </div>
  </AConfigProvider>
</template>

<style scoped>
.app-shell {
  position: relative;
  min-height: 100vh;
  overflow-x: hidden;
}

.app-shell::before {
  content: '';
  position: fixed;
  inset: 0;
  z-index: 0;
  background-image: radial-gradient(
    circle at 1px 1px,
    rgba(11, 31, 36, 0.055) 1px,
    transparent 0
  );
  background-size: 24px 24px;
  mask-image: radial-gradient(
    ellipse 70% 55% at 50% 30%,
    rgba(0, 0, 0, 0.5),
    transparent 72%
  );
  pointer-events: none;
}

.app-content {
  position: relative;
  z-index: 1;
  display: flex;
  width: min(680px, calc(100% - 48px));
  min-height: 100vh;
  flex-direction: column;
  gap: 15px;
  margin: 0 auto;
  padding: 22px 0 10px;
}

.console-card {
  display: flex;
  flex-direction: column;
  gap: 15px;
  padding: 22px 28px 24px;
  border: 1px solid rgba(255, 255, 255, 0.72);
  border-radius: 24px;
  background: var(--card);
  box-shadow:
    0 24px 60px rgba(11, 31, 36, 0.08),
    0 2px 6px rgba(11, 31, 36, 0.03);
  backdrop-filter: blur(20px);
  animation: card-in 480ms var(--ease-out) both;
}

@keyframes card-in {
  from {
    opacity: 0;
    transform: translateY(14px) scale(0.995);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

.error-alert {
  margin: 0;
}

.error-alert :deep(.ant-alert-message) {
  font-size: 12.5px;
  line-height: 1.5;
}

.app-footer {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  margin-top: auto;
  padding-top: 2px;
  color: var(--ink-400);
  font-size: 11px;
  letter-spacing: 0.03em;
}

.footer-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--teal-500);
}

.alert-enter-active,
.alert-leave-active {
  transition:
    opacity 200ms var(--ease-out),
    transform 200ms var(--ease-out);
}

.alert-enter-from,
.alert-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

@media (max-width: 900px) {
  .app-content {
    width: min(680px, calc(100% - 36px));
    padding-top: 22px;
  }

  .console-card {
    padding: 24px;
  }
}

@media (max-height: 700px) {
  .app-content {
    gap: 11px;
    padding: 12px 0 8px;
  }

  .console-card {
    gap: 11px;
    padding: 18px 22px 20px;
  }

  .app-footer {
    display: none;
  }
}
</style>
