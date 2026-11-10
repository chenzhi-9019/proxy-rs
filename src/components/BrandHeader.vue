<script setup lang="ts">
import { ThunderboltFilled } from '@ant-design/icons-vue'

import StatusPill from './StatusPill.vue'

defineProps<{
  running: boolean
  stopping: boolean
}>()
</script>

<template>
  <header class="brand">
    <div class="brand-mark" aria-hidden="true">
      <ThunderboltFilled />
    </div>
    <div class="brand-copy">
      <h1>Proxy <span>RS</span></h1>
      <p>SOCKS5 代理控制台</p>
    </div>
    <StatusPill :running="running" :stopping="stopping" />
  </header>
</template>

<style scoped>
.brand {
  display: flex;
  align-items: center;
  gap: 14px;
  min-height: 52px;
}

.brand-mark {
  display: grid;
  width: 48px;
  height: 48px;
  flex: 0 0 auto;
  place-items: center;
  border-radius: 15px;
  background: linear-gradient(135deg, var(--start-a), var(--start-b));
  color: #04262b;
  font-size: 22px;
  box-shadow:
    0 10px 24px rgba(14, 165, 160, 0.22),
    inset 0 1px 0 rgba(255, 255, 255, 0.55);
}

.brand-copy {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.brand-copy h1 {
  margin: 0;
  color: var(--ink-900);
  font-family: 'Outfit Variable', 'PingFang SC', 'Microsoft YaHei', sans-serif;
  font-size: 33px;
  font-weight: 700;
  letter-spacing: -0.035em;
  line-height: 1.05;
}

.brand-copy h1 span {
  color: var(--teal-600);
}

.brand-copy p {
  margin: 0;
  color: var(--ink-400);
  font-size: 11.5px;
  font-weight: 500;
  letter-spacing: 0.2em;
}

@media (max-width: 900px) {
  .brand-copy h1 {
    font-size: 29px;
  }

  .brand-copy p {
    letter-spacing: 0.14em;
  }
}
</style>
