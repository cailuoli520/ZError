<template>
  <div class="app-header">
    <div class="header-left">
      <div class="app-logo">
        <img
          src="/icons/app-icon.png"
          alt="ZError Logo"
          class="app-logo-img"
          width="20"
          height="20"
          draggable="false"
        />
      </div>
      <div class="app-title">ZError</div>
    </div>
    
    <div class="header-center">
      <div v-if="props.activeTab !== 'questions'" class="tutorial-stepper">
        <div class="step" :class="{ completed: isStep1Completed, active: !isStep1Completed }" @click="$emit('guide-to', 'model-settings')">
          <div class="step-indicator">
            <svg v-if="isStep1Completed" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="12" height="12"><polyline points="20 6 9 17 4 12"></polyline></svg>
            <span v-else>1</span>
          </div>
          <span class="step-text">配置 AI 模型</span>
          <div class="step-tooltip">
            <strong>步骤 1：配置 AI 模型</strong><br>
            前往"设置 &gt; 模型设置"，选择一个 AI 平台并填写您的 API Key。<br>
            <a data-v-f7451f5a="" href="https://docs.zerror.cc/get-apiKey" target="_blank" rel="noopener noreferrer" class="api-doc-link">如何获取Api Key?</a><br><br>
            <strong>必须完成：</strong><br>
            • 在填写 API Key 的平台下，至少选择<strong>一个文本模型</strong>。<br>
            • （可选）还可以选择一个视觉和总结模型。
          </div>
        </div>
        <div class="step-connector" :class="{ completed: isStep1Completed }"></div>
        <div class="step" :class="{ completed: isStep1Completed && isStep2Completed, active: isStep1Completed && !isStep2Completed }" @click="$emit('guide-to', 'home')">
          <div class="step-indicator">
            <svg v-if="isStep1Completed && isStep2Completed" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="12" height="12"><polyline points="20 6 9 17 4 12"></polyline></svg>
            <span v-else>2</span>
          </div>
          <span class="step-text">服务已就绪</span>
          <div class="step-tooltip">
            <strong>步骤 2：服务已就绪</strong><br>
            后端服务运行中即可自动完成，<br>可在"首页"查看服务状态。
          </div>
        </div>
        <div class="step-connector" :class="{ completed: isStep1Completed && isStep2Completed }"></div>
        <div class="step" :class="{ completed: isStep1Completed && isStep2Completed && isStep3Completed, active: isStep1Completed && isStep2Completed && !isStep3Completed }" @click="$emit('guide-to', 'ocs-config')">
          <div class="step-indicator">
            <svg v-if="isStep1Completed && isStep2Completed && isStep3Completed" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="12" height="12"><polyline points="20 6 9 17 4 12"></polyline></svg>
            <span v-else>3</span>
          </div>
          <span class="step-text">让OCS连接题库</span>
          <div class="step-tooltip">
            <strong>步骤 3：让OCS连接题库</strong><br>
            在OCS题库配置中，配置题库
          </div>
        </div>
      </div>
    </div>

    <div class="header-right">
      <button
        class="logout-btn"
        type="button"
        title="退出登录"
        @click="handleLogout"
      >
        <svg class="logout-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"></path>
          <polyline points="16 17 21 12 16 7"></polyline>
          <line x1="21" y1="12" x2="9" y2="12"></line>
        </svg>
        <span class="logout-text">退出登录</span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, onUnmounted } from 'vue'
import { useModelConfig } from '../services/modelConfig'
import { api, clearAdminToken } from '../services/api'
import { bus } from '../services/bus'

const props = defineProps<{
  activeTab?: string
}>()

defineEmits<{
  'guide-to': [target: string]
}>()

interface ServiceStatus {
  status: string
  version: string
  uptime_secs: number
  last_ocs_contact_at: string | null
}

const { settings: modelSettings, platforms: computedPlatforms, selectedTextModels, selectedTextModel } = useModelConfig()

// Tutorial Stepper Logic
const isStep1Completed = computed(() => {
  if (!computedPlatforms.value || computedPlatforms.value.length === 0) {
    return false;
  }

  // 获取当前选中的所有文本模型ID（兼容不同的配置方式）
  let selectedTextModelIds = new Set<string>();
  if (selectedTextModels.value && selectedTextModels.value.length > 0) {
    // selectedTextModels.value 是 AIModel 数组，取 id
    selectedTextModels.value.forEach(m => selectedTextModelIds.add(m.id));
  } else if (selectedTextModel.value) {
    selectedTextModelIds.add(selectedTextModel.value.id);
  }

  // 找到所有配置了有效 apiKey 的平台
  const configuredPlatforms = computedPlatforms.value.filter(p => p.apiKey && p.apiKey.trim() !== '');

  // 判断：在这些配置了 apiKey 的平台中，是否至少有一个平台，其拥有的某个文本模型正处于被选中状态
  return configuredPlatforms.some(platform =>
    platform.models?.some(model => model.category === 'text' && selectedTextModelIds.has(model.id))
  );
})

// 步骤 2：后端服务可达（GET /api/status 成功）
const isStep2Completed = ref(false)
// 步骤 3：OCS 已连接（/api/status 的 last_ocs_contact_at 非空）
const isStep3Completed = ref(false)

const STATUS_POLL_INTERVAL = 15000
let statusTimer: ReturnType<typeof setInterval> | null = null

const refreshServiceStatus = async () => {
  try {
    const status = await api.get<ServiceStatus>('/api/status')
    isStep2Completed.value = true
    if (status?.last_ocs_contact_at) {
      isStep3Completed.value = true
    }
  } catch (error) {
    isStep2Completed.value = false
    console.warn('获取服务状态失败:', error)
  }
}

const startStatusPolling = () => {
  refreshServiceStatus()
  if (!statusTimer) {
    statusTimer = setInterval(() => {
      // 两步都完成后无需继续轮询
      if (isStep2Completed.value && isStep3Completed.value) {
        stopStatusPolling()
        return
      }
      refreshServiceStatus()
    }, STATUS_POLL_INTERVAL)
  }
}

const stopStatusPolling = () => {
  if (statusTimer) {
    clearInterval(statusTimer)
    statusTimer = null
  }
}

onMounted(() => {
  startStatusPolling()
})

onUnmounted(() => {
  stopStatusPolling()
})

const handleLogout = () => {
  clearAdminToken()
  bus.emit('auth-logout')
}

</script>

<style scoped>
.app-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 40px;
  background: var(--bg-primary, #f4f4f4);
  color: var(--text-primary, #2d3748);
  user-select: none;
  position: relative;
  z-index: 1000;
}

/* macOS 原生标题栏适配：stepper 与系统红绿灯同一行 */
.app-header.macos-header {
  height: 40px;
  padding-left: 80px; /* 给 macOS 红绿灯按钮留空间 */
}

.header-left {
  margin-left: 12px;
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 0 0 auto;
  color:  #ffbd42;
}

.app-logo {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  flex: 0 0 auto;
  overflow: visible;
}

.app-logo-img {
  width: 20px;
  height: 20px;
  display: block;
  object-fit: contain;
}

.app-logo svg {
  color: rgba(255, 255, 255, 0.9);
}

.app-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary, #2d3748);
}

.header-center {
  flex: 1;
  display: flex;
  justify-content: right;
  align-items: center;
  min-width: 0;
  pointer-events: none;
}








.header-right {
  display: flex;
  align-items: center;
  gap: 1px;
  flex: 0 0 auto;
  padding-right: 4px;
  -webkit-app-region: no-drag;
  pointer-events: auto;
}

.header-right--macos {
  padding-right: 12px;
}

.update-tip {
  display: inline-flex;
  align-items: center;
  max-width: min(360px, 42vw);
  margin-right: 6px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, #f59e0b 35%, var(--border-color, #e2e8f0));
  background: color-mix(in srgb, #f59e0b 12%, var(--bg-primary, #fff));
  overflow: hidden;
}

.update-tip--installing,
.update-tip--downloading {
  border-color: color-mix(in srgb, #3b82f6 40%, var(--border-color, #e2e8f0));
  background: color-mix(in srgb, #3b82f6 10%, var(--bg-primary, #fff));
}

.update-tip--ready-relaunch,
.update-tip--done {
  border-color: color-mix(in srgb, #22c55e 40%, var(--border-color, #e2e8f0));
  background: color-mix(in srgb, #22c55e 10%, var(--bg-primary, #fff));
}

.update-tip--error {
  border-color: color-mix(in srgb, #ef4444 40%, var(--border-color, #e2e8f0));
  background: color-mix(in srgb, #ef4444 10%, var(--bg-primary, #fff));
}

.update-tip-main {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  padding: 3px 10px 3px 6px;
  border: none;
  background: transparent;
  color: var(--text-primary, #2d3748);
  cursor: pointer;
  -webkit-app-region: no-drag;
}

.update-tip-icon-wrap {
  position: relative;
  width: 22px;
  height: 22px;
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: #575F76;
}

.update-tip-icon-wrap.is-progress {
  width: 26px;
  height: 26px;
}

.update-tip-icon {
  width: 14px;
  height: 14px;
  display: block;
  position: relative;
  z-index: 1;
}

.update-tip-icon-wrap.is-progress .update-tip-icon {
  width: 12px;
  height: 12px;
}

.update-tip-ring {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  transform: rotate(-90deg);
}

.update-tip-ring-bg,
.update-tip-ring-fg {
  fill: none;
  stroke-width: 2.5;
}

.update-tip-ring-bg {
  stroke: color-mix(in srgb, #3b82f6 22%, transparent);
}

.update-tip-ring-fg {
  stroke: #3b82f6;
  stroke-linecap: round;
  transition: stroke-dashoffset 0.2s ease;
}

.update-tip--available .update-tip-icon-wrap,
.update-tip--idle .update-tip-icon-wrap {
  color: #f59e0b;
}

.update-tip--downloading .update-tip-icon-wrap,
.update-tip--installing .update-tip-icon-wrap {
  color: #3b82f6;
}

.update-tip--done .update-tip-icon-wrap,
.update-tip--ready-relaunch .update-tip-icon-wrap {
  color: #22c55e;
}

.update-tip--error .update-tip-icon-wrap {
  color: #ef4444;
}

.update-tip-text {
  font-size: 12px;
  line-height: 1.2;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.window-control {
  border-radius: 0px;
  height: 40px;
  width: 46px;
  border: none;
  background: transparent;
  color: var(--text-primary, #2d3748);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background-color 0.2s ease;
}

.window-control-icon {
  width: 16px;
  height: 16px;
  display: block;
  flex: 0 0 auto;
}

.window-control:hover {
  background-color: rgba(158, 158, 158, 0.1);
}

.window-control.close:hover {
  background-color: #e74c3c;
  color: white;
}

.window-control.minimize:hover,
.window-control.maximize:hover {
  background-color: var(--bg-secondary, #e2e8f0);
}

/* 确保拖拽区域不会被按钮阻挡 */
.window-control {
  -webkit-app-region: no-drag;
}

@media (max-width: 900px) {

}

/* Tutorial Stepper Styles */
.tutorial-stepper {
  /* --- Stepper CSS Variables (Restored original colors) --- */
  --stepper-text-primary: var(--text-primary);
  --stepper-text-secondary: var(--text-secondary);
  --stepper-border: transparent;
  --stepper-active-bg: transparent;
  --stepper-active-text: var(--color-primary, #667eea);
  --stepper-completed-bg: #48bb78;
  --stepper-tooltip-bg: rgba(255, 255, 255, 0.45);
  --stepper-tooltip-border: rgba(194, 194, 194, 0.6);
  --stepper-tooltip-shadow: 0 8px 32px rgba(0, 0, 0, 0.12);
  --stepper-pulse-start: rgba(102, 126, 234, 0.5);
  --stepper-pulse-end: rgba(102, 126, 234, 0);
  --stepper-link-color: #ff9800;
  --stepper-link-hover: #e65100;
  --stepper-connector-bg: var(--border-color);

  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  pointer-events: auto;
  margin: 0 auto;
  padding: 2px 12px;
  border-radius: 16px;
}

[data-theme='dark'] .tutorial-stepper {
  /* --- Stepper CSS Variables (Restored original colors) --- */
  --stepper-text-primary: var(--text-primary);
  --stepper-text-secondary: var(--text-secondary);
  --stepper-border: transparent;
  --stepper-active-bg: transparent;
  --stepper-active-text: var(--color-primary, #667eea);
  --stepper-completed-bg: #48bb78;
  --stepper-tooltip-bg: rgba(30, 30, 30, 0.45);
  --stepper-tooltip-border: rgba(80, 80, 80, 0.6);
  --stepper-tooltip-shadow: 0 8px 32px rgba(0, 0, 0, 0.12);
  --stepper-pulse-start: rgba(102, 126, 234, 0.5);
  --stepper-pulse-end: rgba(102, 126, 234, 0);
  --stepper-link-color: #ff9800;
  --stepper-link-hover: #e65100;
  --stepper-connector-bg: #656565;
}

.step {
  position: relative;
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--stepper-text-secondary);
  transition: all 0.3s ease;
  cursor: help;
}

.step:not(.active):not(.completed) .step-indicator,
.step:not(.active):not(.completed) .step-text {
  opacity: 0.5;
}

@keyframes pulse-ring {
  0% { box-shadow: 0 0 0 0 var(--stepper-pulse-start); }
  70% { box-shadow: 0 0 0 5px var(--stepper-pulse-end); }
  100% { box-shadow: 0 0 0 0 var(--stepper-pulse-end); }
}

.step.active {
  background: var(--stepper-active-bg);
  padding: 3px 8px;
  border-radius: 12px;
}

.step.completed {
  /* opacity: 0.8; */
}

.step-indicator {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--stepper-border);
  color: var(--stepper-text-secondary);
  font-size: 10px;
  font-weight: 600;
  transition: all 0.3s ease;
}

.step.active .step-indicator {
  background: var(--stepper-active-text);
  color: white;
  animation: pulse-ring 2s infinite;
}

.step.completed .step-indicator {
  background: var(--stepper-completed-bg);
  color: white;
}

.step-text {
  font-size: 12px;
  font-weight: 500;
  color: var(--stepper-text-primary);
  white-space: nowrap;
}

.step.active .step-text {
  color: var(--stepper-active-text);
}

.step-tooltip {
  position: absolute;
  top: 130%;
  left: 50%;
  transform: translateX(-50%) translateY(10px);
  background: var(--stepper-tooltip-bg);
  backdrop-filter: blur(20px) saturate(120%);
  -webkit-backdrop-filter: blur(20px) saturate(120%);
  border: 1px solid var(--stepper-tooltip-border);
  box-shadow: var(--stepper-tooltip-shadow);
  padding: 10px 14px;
  border-radius: 8px;
  font-size: 12px;
  color: var(--stepper-text-primary);
  white-space: nowrap;
  opacity: 0;
  visibility: hidden;
  transition: all 0.2s ease;
  z-index: 100;
  line-height: 1.6;
  text-align: left;
}

.step-tooltip::before {
  content: '';
  position: absolute;
  top: -5px;
  left: 50%;
  transform: translateX(-50%) rotate(45deg);
  width: 8px;
  height: 8px;
  background: var(--stepper-tooltip-bg);
  backdrop-filter: blur(20px) saturate(120%);
  -webkit-backdrop-filter: blur(20px) saturate(120%);
  border-left: 1px solid var(--stepper-tooltip-border);
  border-top: 1px solid var(--stepper-tooltip-border);
}

.step-tooltip strong {
  font-size: 13px;
  color: var(--stepper-active-text);
}

.api-doc-link {
  color: var(--stepper-link-color);
  text-decoration: underline;
  margin-top: 4px;
  display: inline-block;
  font-weight: 500;
}

.api-doc-link:hover {
  color: var(--stepper-link-hover);
}

.step:hover .step-tooltip {
  opacity: 1;
  visibility: visible;
  transform: translateX(-50%) translateY(0);
}

.step-connector {
  width: 20px;
  height: 2px;
  background: var(--stepper-connector-bg);
  border-radius: 1px;
  transition: background 0.3s ease;
}

.step-connector.completed {
  background: var(--stepper-completed-bg);
}

@media (max-width: 768px) {
  .step-text {
    display: none;
  }
  .step-connector {
    width: 12px;
  }
}
.logout-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 10px;
  border: 1px solid var(--border-color, #e2e8f0);
  border-radius: 8px;
  background: var(--bg-secondary, #fff);
  color: var(--text-secondary, #718096);
  font-size: 12px;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.logout-btn:hover {
  background: var(--bg-tertiary, #f4f4f4);
  color: var(--text-primary, #2d3748);
}

.logout-icon {
  width: 14px;
  height: 14px;
  flex: 0 0 auto;
}

.logout-text {
  white-space: nowrap;
}
</style>
