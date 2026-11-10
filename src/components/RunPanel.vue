<script setup lang="ts">
import { Button as AButton, Tooltip as ATooltip } from 'ant-design-vue'
import { CheckOutlined, CopyOutlined } from '@ant-design/icons-vue'

defineProps<{
  visible: boolean
  endpoint: string | null
  uptime: string
  copied: boolean
}>()

defineEmits<{ copy: [] }>()
</script>

<template>
  <Transition name="run-panel">
    <section
      v-if="visible && endpoint"
      class="run-panel"
      aria-live="polite"
      aria-label="运行中的代理端点"
    >
      <div class="run-main">
        <span class="run-dot" aria-hidden="true" />
        <code class="run-endpoint">{{ endpoint }}</code>
      </div>
      <div class="run-meta">
        <ATooltip :title="copied ? '已复制' : '复制端点'">
          <AButton
            class="copy-button"
            type="text"
            html-type="button"
            :aria-label="copied ? '已复制端点' : '复制端点'"
            @click="$emit('copy')"
          >
            <CheckOutlined v-if="copied" />
            <CopyOutlined v-else />
          </AButton>
        </ATooltip>
        <span class="run-time">运行 {{ uptime }}</span>
      </div>
    </section>
  </Transition>
</template>

<style scoped>
.run-panel {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  padding: 12px 16px;
  border: 1px solid rgba(11, 122, 110, 0.16);
  border-radius: 18px;
  background:
    linear-gradient(135deg, rgba(255, 255, 255, 0.94), rgba(236, 255, 249, 0.9));
  box-shadow: 0 12px 30px rgba(11, 31, 36, 0.06);
}

.run-main {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 10px;
}

.run-dot {
  width: 8px;
  height: 8px;
  flex: 0 0 auto;
  border-radius: 50%;
  background: linear-gradient(135deg, var(--stop-a), var(--stop-b));
  box-shadow: 0 0 0 4px rgba(255, 107, 87, 0.12);
}

.run-endpoint {
  overflow: hidden;
  color: var(--ink-900);
  font-family: 'Outfit Variable', ui-monospace, 'Cascadia Code', monospace;
  font-size: 13.5px;
  font-weight: 600;
  letter-spacing: 0.01em;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.run-meta {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 10px;
}

.copy-button {
  color: var(--teal-600);
}

.run-time {
  color: var(--ink-600);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.run-panel-enter-active,
.run-panel-leave-active {
  transition:
    opacity 220ms var(--ease-out),
    transform 220ms var(--ease-out);
}

.run-panel-enter-from,
.run-panel-leave-to {
  opacity: 0;
  transform: translateY(6px);
}

@media (max-width: 620px) {
  .run-endpoint {
    font-size: 12px;
  }
}
</style>
