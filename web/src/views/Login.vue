<template>
  <div class="login-page">
    <form class="login-card" @submit.prevent="handleSubmit">
      <div class="login-brand">
        <img src="/icons/app-icon.png" alt="ZError Logo" class="login-logo" width="40" height="40" draggable="false" />
        <div class="login-title">ZError 管理后台</div>
      </div>
      <p class="login-desc">请输入管理员令牌以继续</p>
      <input
        v-model="token"
        class="login-input"
        type="password"
        placeholder="管理员令牌"
        autocomplete="current-password"
        :disabled="submitting"
        autofocus
      />
      <div v-if="errorMessage" class="login-error">{{ errorMessage }}</div>
      <button class="login-submit" type="submit" :disabled="submitting || !token.trim()">
        {{ submitting ? '登录中…' : '登录' }}
      </button>
    </form>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { login } from '../services/api'
import { bus } from '../services/bus'

const token = ref('')
const submitting = ref(false)
const errorMessage = ref('')

const handleSubmit = async () => {
  const value = token.value.trim()
  if (!value || submitting.value) return
  submitting.value = true
  errorMessage.value = ''
  try {
    const result = await login(value)
    if (result.success && result.role === 'admin') {
      token.value = ''
      bus.emit('auth-login')
    } else {
      errorMessage.value = result.message || '令牌无效或权限不足'
    }
  } catch (error) {
    console.error('登录失败:', error)
    errorMessage.value = '无法连接服务器，请稍后重试'
  } finally {
    submitting.value = false
  }
}
</script>

<style scoped>
.login-page {
  height: 100%;
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-tertiary, #f4f4f4);
}

.login-card {
  width: 340px;
  padding: 32px 28px;
  border-radius: 12px;
  background: var(--bg-primary, #fff);
  border: 1px solid var(--border-color, #e2e8f0);
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.08);
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.login-brand {
  display: flex;
  align-items: center;
  gap: 10px;
}

.login-logo {
  width: 40px;
  height: 40px;
  object-fit: contain;
}

.login-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--text-primary, #2d3748);
}

.login-desc {
  margin: 0;
  font-size: 13px;
  color: var(--text-secondary, #718096);
}

.login-input {
  width: 100%;
  box-sizing: border-box;
  padding: 10px 12px;
  font-size: 14px;
  border-radius: 8px;
  border: 1px solid var(--border-color, #e2e8f0);
  background: var(--bg-secondary, #fff);
  color: var(--text-primary, #2d3748);
}

.login-input:focus {
  border-color: var(--color-primary, #007aff);
}

.login-error {
  font-size: 12px;
  color: #e53e3e;
}

.login-submit {
  padding: 10px 12px;
  font-size: 14px;
  border-radius: 8px;
  border: none;
  background: var(--color-primary, #007aff);
  color: #fff;
  cursor: pointer;
}

.login-submit:hover:not(:disabled) {
  background: var(--color-primary-hover, #0056b3);
}

.login-submit:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
</style>
