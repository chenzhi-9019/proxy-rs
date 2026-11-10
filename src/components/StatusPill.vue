<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  running: boolean
  stopping: boolean
}>()

const label = computed(() => {
  if (props.stopping) return '停止中'
  return props.running ? '运行中' : '未运行'
})
</script>

<template>
  <span
    class="status-pill"
    :class="{ 'is-running': running, 'is-stopping': stopping }"
    role="status"
    aria-live="polite"
  >
    <span class="status-dot" aria-hidden="true" />
    <span>{{ label }}</span>
  </span>
</template>

<style scoped>
.status-pill {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  margin-left: auto;
  padding: 8px 14px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.72);
  color: var(--ink-600);
  font-size: 12.5px;
  font-weight: 600;
  letter-spacing: 0.02em;
  transition:
    background-color 200ms var(--ease-out),
    border-color 200ms var(--ease-out),
    color 200ms var(--ease-out);
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #a8b8be;
  box-shadow: 0 0 0 3px rgba(168, 184, 190, 0.18);
}

.is-running {
  border-color: rgba(217, 119, 6, 0.28);
  background: linear-gradient(135deg, rgba(255, 247, 230, 0.95), rgba(255, 240, 199, 0.9));
  color: #9a4a00;
}

.is-running .status-dot {
  position: relative;
  background: linear-gradient(135deg, var(--stop-a), var(--stop-b));
  box-shadow: 0 0 0 3px rgba(255, 107, 87, 0.16);
}

.is-running .status-dot::after {
  content: '';
  position: absolute;
  inset: -4px;
  border-radius: 50%;
  border: 1.5px solid rgba(255, 122, 89, 0.5);
  animation: status-pulse 1.8s ease-out infinite;
}

.is-stopping {
  border-color: rgba(217, 119, 6, 0.2);
  color: #a16207;
}

.is-stopping .status-dot {
  background: var(--stop-b);
  animation: dot-blink 1s ease-in-out infinite;
}

@keyframes status-pulse {
  0% {
    transform: scale(0.7);
    opacity: 0.75;
  }
  70% {
    transform: scale(1.55);
    opacity: 0;
  }
  100% {
    opacity: 0;
  }
}

@keyframes dot-blink {
  50% {
    opacity: 0.35;
  }
}

@media (prefers-reduced-motion: reduce) {
  .is-running .status-dot::after,
  .is-stopping .status-dot {
    animation: none;
  }
}
</style>
