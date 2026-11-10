<script setup lang="ts">
import { computed } from 'vue'
import { Button as AButton } from 'ant-design-vue'
import { PoweroffOutlined, ThunderboltFilled } from '@ant-design/icons-vue'

const props = defineProps<{
  mode: 'start' | 'starting' | 'stop' | 'stopping'
  disabled: boolean
}>()

const emit = defineEmits<{ click: [] }>()

const active = computed(() => props.mode === 'stop' || props.mode === 'stopping')
const loading = computed(
  () => props.mode === 'starting' || props.mode === 'stopping',
)
const label = computed(() => {
  switch (props.mode) {
    case 'starting':
      return '正在启动…'
    case 'stopping':
      return '正在停止…'
    case 'stop':
      return '停止代理'
    default:
      return '开始代理'
  }
})
</script>

<template>
  <AButton
    class="action-button"
    :class="{ 'is-active': active }"
    :loading="loading"
    :disabled="disabled"
    html-type="button"
    @click="emit('click')"
  >
    <template #icon>
      <PoweroffOutlined v-if="active" />
      <ThunderboltFilled v-else />
    </template>
    {{ label }}
  </AButton>
</template>
