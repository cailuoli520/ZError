<template>
  <div class="general-settings">
    <div class="network-group">
      <div class="group-title">显示设置</div>
    <div class="setting-item">
      <div class="setting-info">
        <h3 class="setting-title">主题模式</h3>
        <p class="setting-description">选择您偏好的界面主题</p>
      </div>
      <div class="setting-control">
        <ThemeSwitch v-model="localSettings.theme" @update:modelValue="handleThemeChange" />
      </div>
    </div>
    </div>

    <div class="network-group">
      <div class="group-title">保存设置</div>
    <div class="setting-item">
      <div class="setting-info">
        <h3 class="setting-title">题目保存文件夹</h3>
        <p class="setting-description">
          AI返回答案的题目将保存至：
          <button
            v-if="settings.questionSaveDir"
            type="button"
            class="selected-folder-link"
            :disabled="!canOpenQuestionSaveFolder"
            @click="openQuestionSaveFolder"
          >
            {{ settings.questionSaveDir }}
          </button>
          <span v-else class="unset-hint">默认文件夹</span>
        </p>

      </div>
      <div class="setting-control">
        <button class="folder-action-btn select-folder-btn" @click="selectQuestionSaveDir">
          <svg t="1774430153065" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg" width="16" height="16">
            <path d="M418.688 133.333333a122.666667 122.666667 0 0 1 93.013333 42.666667H785.066667a186.666667 186.666667 0 0 1 186.581333 181.162667l0.085333 5.504v17.92h-0.256c0.170667 1.429333 0.256 2.88 0.256 4.373333V789.333333a186.666667 186.666667 0 0 1-186.666666 186.666667H238.933333A186.666667 186.666667 0 0 1 52.266667 789.333333V320a186.666667 186.666667 0 0 1 186.666666-186.666667z m0 74.666667H238.933333A112 112 0 0 0 126.933333 320v469.333333c0 61.866667 50.133333 112 112 112h546.133334c61.866667 0 112-50.133333 112-112V422.272H615.04a122.666667 122.666667 0 0 1-113.834667-76.992l-1.834666-4.842667-35.413334-100.416a48 48 0 0 0-45.269333-32.021333zM448 688a37.333333 37.333333 0 0 1 3.072 74.538667L448 762.666667h-170.666667a37.333333 37.333333 0 0 1-3.072-74.538667L277.333333 688h170.666667z m337.066667-437.333333H546.88l22.912 64.917333a48 48 0 0 0 41.685333 31.914667l3.562667 0.128 281.024-0.021334a112.021333 112.021333 0 0 0-106.389333-96.853333l-4.608-0.085333z" fill="currentColor"/>
          </svg>
          选择文件夹
        </button>
      </div>
    </div>

    <div class="setting-item">
      <div class="setting-info">
        <h3 class="setting-title">AI回答添加到本地题库</h3>
        <p class="setting-description">开启后，AI 新答出的题目会写入本地题库；关闭则只返回答案，不入库。</p>
      </div>
      <div class="setting-control">
        <Toggle v-model="settings.autoAddToQuestionBank" variant="default" size="medium"
          @change="handleSettingChange('autoAddToQuestionBank', $event)" />
      </div>
    </div>
    </div>

    <div class="network-group">
      <div class="group-title">AI设置</div>
    <div class="setting-item">

      <div class="setting-info">
        <h3 class="setting-title">模型最长响应时间</h3>
        <p class="setting-description">AI 模型单次请求超时（秒）。超时后中止该次调用并可按「失败自动重试次数」重试；整题最长等待 ≈ 本值 × (1+重试) + 缓冲。默认 40 秒。</p>
      </div>
      <div class="setting-control timeout-control">
        <input
          type="number"
          class="form-input"
          :value="settings.modelResponseTimeout ?? 40"
          min="5"
          max="600"
          step="5"
          @change="handleSettingChange('modelResponseTimeout', Number(($event.target as HTMLInputElement).value))"
        />
        <span class="timeout-unit">秒</span>
      </div>
    </div>
    <div class="setting-item">
      <div class="setting-info">
        <h3 class="setting-title">失败自动重试次数</h3>
        <p class="setting-description">仅在「只有一个基础模型」、总结模型失败、或视觉模型调用失败时自动重试。0 表示不重试，默认 2 次。</p>
      </div>
      <div class="setting-control timeout-control">
        <input
          type="number"
          class="form-input"
          :value="settings.modelRetryCount ?? 2"
          min="0"
          max="10"
          step="1"
          @change="handleSettingChange('modelRetryCount', Math.max(0, Math.min(10, Number(($event.target as HTMLInputElement).value) || 0)))"
        />
        <span class="timeout-unit">次</span>
      </div>
    </div>
    </div>

    <div class="network-group">
      <div class="group-title">公网题库访问</div>
    <div class="setting-item">
      <div class="setting-info">
        <h3 class="setting-title">查询接口需要令牌</h3>
        <p class="setting-description">开启后，OCS 访问 /query 必须携带查询令牌；关闭则任何人知道地址即可查询（不推荐）。</p>
      </div>
      <div class="setting-control">
        <Toggle v-model="settings.publicQueryRequireToken" variant="default" size="medium"
          @change="handleSettingChange('publicQueryRequireToken', $event)" />
      </div>
    </div>

    <div class="setting-item token-item">
      <div class="setting-info">
        <h3 class="setting-title">查询令牌</h3>
        <p class="setting-description">供 OCS 题库配置使用，每个令牌可单独吊销。首页「OCS 配置」会使用第一个令牌。</p>
        <ul class="token-list" v-if="queryTokens.length">
          <li v-for="user in queryTokens" :key="user.id" class="token-row">
            <input class="form-input token-name" v-model="user.name" placeholder="名称" @change="saveTokens" />
            <code class="token-value" :title="user.token">{{ maskToken(user.token) }}</code>
            <button class="folder-action-btn" type="button" @click="copyToken(user.token)">复制</button>
            <button class="folder-action-btn danger" type="button" @click="removeToken(user.id)">删除</button>
          </li>
        </ul>
        <p v-else class="unset-hint">尚未添加查询令牌</p>
      </div>
      <div class="setting-control">
        <button class="folder-action-btn select-folder-btn" type="button" @click="addToken">新增令牌</button>
      </div>
    </div>

    <div class="setting-item">
      <div class="setting-info">
        <h3 class="setting-title">管理员令牌</h3>
        <p class="setting-description">用于登录管理后台与调用管理接口。修改后需用新令牌重新登录。</p>
      </div>
      <div class="setting-control timeout-control">
        <input type="password" class="form-input admin-token-input" v-model="adminTokenDraft" placeholder="新的管理员令牌" autocomplete="new-password" />
        <button class="folder-action-btn select-folder-btn" type="button" :disabled="!canSaveAdminToken" @click="saveAdminToken">保存</button>
      </div>
    </div>
    </div>
  </div>

  <FolderPickerDialog
    :visible="showFolderPicker"
    :initial-folder-id="settings.questionSaveFolderId"
    @cancel="showFolderPicker = false"
    @confirm="handleFolderConfirm"
  />
</template>

<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import { useSettings } from '../../services/settings'
import { setAdminToken } from '../../services/api'
import { bus } from '../../services/bus'

import { useSettingsManager } from '../../composables/useSettingsManager'

import { useTheme } from '../../composables/useTheme'
import ThemeSwitch from './GeneralSettings/ThemeSwitch.vue'
import Toggle from '../../components/Toggle.vue'
import FolderPickerDialog from '../../components/FolderPickerDialog.vue'

const emit = defineEmits<{
  'open-question-folder': [folderId: number]
}>()

// 设置管理
const { settings, saveSettings, setSetting } = useSettingsManager()
const canOpenQuestionSaveFolder = computed(() => settings.value.questionSaveFolderId !== null && settings.value.questionSaveFolderId !== undefined)


const handleSettingChange = (key: keyof import('../../services/settings').AppSettings, value: any) => {
  setSetting(key, value)
}


const showFolderPicker = ref(false)

const selectQuestionSaveDir = () => {
  showFolderPicker.value = true
}

const openQuestionSaveFolder = () => {
  const folderId = settings.value.questionSaveFolderId
  if (folderId === null || folderId === undefined) return
  emit('open-question-folder', folderId)
}

const handleFolderConfirm = async (folderId: number, folderName: string, folderPath: string) => {

  showFolderPicker.value = false
  setSetting('questionSaveDir', folderPath || folderName)
  setSetting('questionSaveFolderId', folderId)
  await saveSettings()
}

// ===== 公网题库访问 =====
const { flush: flushSettings } = useSettings()

const queryTokens = computed(() => settings.value.multiUser?.users || [])

const randomToken = () => {
  const bytes = new Uint8Array(16)
  crypto.getRandomValues(bytes)
  return Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('')
}

const maskToken = (token: string) => (token.length > 10 ? `${token.slice(0, 6)}…${token.slice(-4)}` : token)

const ensureMultiUser = () => {
  if (!settings.value.multiUser) {
    settings.value.multiUser = { enabled: true, users: [] }
  }
  settings.value.multiUser.enabled = true
}

const saveTokens = async () => {
  ensureMultiUser()
  await saveSettings()
  await flushSettings()
}

const addToken = async () => {
  ensureMultiUser()
  settings.value.multiUser.users.push({
    id: `u_${Date.now()}`,
    name: `令牌 ${settings.value.multiUser.users.length + 1}`,
    token: randomToken(),
    createdAt: new Date().toISOString(),
  })
  await saveTokens()
}

const removeToken = async (id: string) => {
  ensureMultiUser()
  settings.value.multiUser.users = settings.value.multiUser.users.filter(u => u.id !== id)
  await saveTokens()
}

const copyToken = async (token: string) => {
  try {
    await navigator.clipboard.writeText(token)
  } catch {
    window.prompt('请手动复制令牌', token)
  }
}

const adminTokenDraft = ref('')
const canSaveAdminToken = computed(() => adminTokenDraft.value.trim().length >= 8)
const saveAdminToken = async () => {
  const next = adminTokenDraft.value.trim()
  if (next.length < 8) return
  setSetting('adminToken', next)
  await saveSettings()
  await flushSettings()
  setAdminToken(next)
  adminTokenDraft.value = ''
  bus.emit('auth-login')
  alert('管理员令牌已更新')
}

// 主题管理
const { setTheme } = useTheme()

// 本地设置状态
const localSettings = ref({
  theme: 'auto' as 'light' | 'dark' | 'auto'
})

// 处理主题变更
const handleThemeChange = async (theme: 'light' | 'dark' | 'auto') => {
  localSettings.value.theme = theme
  setSetting('theme', theme)
  await setTheme(theme)
  await saveSettings()
}

// 同步本地设置
const syncLocalSettings = () => {
  localSettings.value = {
    theme: settings.value.theme || 'auto'
  }
}

onMounted(() => {
  syncLocalSettings()
})
</script>

<style scoped>
.setting-item {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  padding: 18px 0;
  border-bottom: 1px solid var(--border-color);
}

.setting-item:last-child {
  border-bottom: none;
}

.setting-info {
  flex: 1;
  margin-right: 18px;
}

.setting-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-primary);
  margin: 0 0 6px 0;
}

.setting-description {
  font-size: 11px;
  color: var(--text-secondary);
  margin: 0;
  line-height: 1.4;
}

.setting-control {
  flex-shrink: 0;
}

.prompt-editor-control {
  width: min(520px, 100%);
}

.system-prompt-textarea {
  width: 100%;
  min-height: 140px;
  padding: 10px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  background: var(--bg-primary);
  color: var(--text-primary);
  font-size: 12px;
  line-height: 1.5;
  resize: vertical;
  box-sizing: border-box;
}

.prompt-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 10px;
}

.folder-action-btn {
  --folder-btn-bg: var(--bg-secondary);
  --folder-btn-text: var(--text-primary);
  --folder-btn-border: var(--border-color);
  --folder-btn-hover-bg: var(--hover-bg);
  --folder-btn-hover-border: var(--border-secondary);
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 6px 12px;
  border-radius: 6px;
  border: 1px solid var(--folder-btn-border);
  background: var(--folder-btn-bg);
  color: var(--folder-btn-text);
  font-size: 12px;
  cursor: pointer;
  transition: background 0.2s ease, border-color 0.2s ease, opacity 0.2s ease !important;
}

.folder-action-btn:hover {
  background: var(--folder-btn-hover-bg);
  border-color: var(--folder-btn-hover-border);
}

.folder-action-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.open-folder-btn {
  --folder-btn-bg: var(--general-settings-open-folder-btn-bg);
  --folder-btn-text: var(--general-settings-open-folder-btn-text);
  --folder-btn-border: var(--general-settings-open-folder-btn-border);
  --folder-btn-hover-bg: var(--general-settings-open-folder-btn-hover-bg);
  --folder-btn-hover-border: var(--general-settings-open-folder-btn-hover-border);
}

.select-folder-btn {
  --folder-btn-bg: var(--general-settings-select-folder-btn-bg);
  --folder-btn-text: var(--general-settings-select-folder-btn-text);
  --folder-btn-border: var(--general-settings-select-folder-btn-border);
  --folder-btn-hover-bg: var(--general-settings-select-folder-btn-hover-bg);
  --folder-btn-hover-border: var(--general-settings-select-folder-btn-hover-border);
}



/* 按钮阴影扩散动画 */
.btn-ripple {
  position: relative;
  overflow: visible;
  /* 确保动画不受全局 transition 干扰 */
  transition: none !important;
  animation: ripple-spread 0.6s cubic-bezier(0.23, 1, 0.32, 1);
}

@keyframes ripple-spread {
  0% {
    box-shadow: 0 0 0 0 rgba(102, 126, 234, 0.4);
  }
  100% {
    box-shadow: 0 0 0 15px rgba(102, 126, 234, 0);
  }
}

.network-group {
  background: var(--network-group-bg);
  border: 1px solid var(--border-color);
  border-radius: 14px;
  padding: 0 14px;
  margin: 12px 0;
  box-shadow: 0 2px 12px rgba(0,0,0,.05);
}

.group-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);
  padding: 10px 0 2px 0;
  letter-spacing: 0.03em;
}

.setting-control .toggle-default {
  --toggle-active: #48bb78;
}

.network-group .setting-item:last-child {
  border-bottom: none;
}

.timeout-control {
  display: flex;
  align-items: center;
  gap: 4px;
}

.timeout-input {
  width: 80px;
  padding: 5px 8px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  font-size: 12px;
  color: var(--text-primary);
  background: var(--bg-primary);
  text-align: center;
  outline: none;
  transition: border-color 0.2s;
}

.timeout-input::-webkit-inner-spin-button,
.timeout-input::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

.timeout-input {
  -moz-appearance: textfield;
}

.timeout-input:focus {
  border-color: var(--color-primary, #667eea);
}

.timeout-unit {
  font-size: 12px;
  color: var(--text-secondary);
}

.input-group {
  display: flex;
  align-items: center;
  gap: 6px;
}

.form-input[type=number] {
  text-align: center;
  -moz-appearance: textfield;
}

.form-input::-webkit-inner-spin-button,
.form-input::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
}


.input-suffix {
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
}

.setting-title-with-help {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-bottom: 6px;
}

.setting-title-with-help .setting-title {
  margin: 0;
}

.help-button {
  background: none;
  border: none;
  cursor: pointer;
  padding: 1px;
  color: var(--text-secondary);
  display: flex;
  align-items: center;
}

.help-button:hover {
  color: var(--text-primary);
}

.selected-folder-name {
  color: var(--color-primary);
  font-weight: 500;
}

.selected-folder-link {
  padding: 0;
  border: none;
  background: transparent;
  color: var(--color-primary);
  font: inherit;
  font-weight: 500;
  text-align: left;
  cursor: pointer;
}

.selected-folder-link:hover {
  text-decoration: underline;
}

.selected-folder-link:disabled {
  opacity: 0.7;
  cursor: default;
  text-decoration: none;
}

.unset-hint {

  color: var(--text-secondary);
  font-style: italic;
}

.token-list { list-style: none; margin: 8px 0 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
.token-row { display: flex; align-items: center; gap: 8px; }
.token-name { width: 120px; }
.token-value { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; opacity: 0.85; }
.folder-action-btn.danger { color: #ef4444; }
.admin-token-input { width: 220px; }
</style>
