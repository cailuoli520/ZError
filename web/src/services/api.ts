/**
 * 管理后台 REST 客户端：统一附带管理员令牌，401 时清除令牌并回到登录页。
 * 所有服务层（settings / modelConfig / database）都通过这里访问后端。
 */

const TOKEN_KEY = 'zerror_admin_token'

export class ApiError extends Error {
  status: number
  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

export const getAdminToken = (): string => localStorage.getItem(TOKEN_KEY) || ''
export const setAdminToken = (token: string) => localStorage.setItem(TOKEN_KEY, token)
export const clearAdminToken = () => localStorage.removeItem(TOKEN_KEY)

type Listener = () => void
const unauthorizedListeners = new Set<Listener>()
/** 注册 401 回调（App.vue 用于切回登录页） */
export const onUnauthorized = (fn: Listener) => {
  unauthorizedListeners.add(fn)
  return () => unauthorizedListeners.delete(fn)
}

const notifyUnauthorized = () => {
  clearAdminToken()
  unauthorizedListeners.forEach(fn => fn())
}

export const buildQuery = (params: Record<string, unknown> = {}): string => {
  const sp = new URLSearchParams()
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined || v === null || v === '') continue
    sp.set(k, String(v))
  }
  const s = sp.toString()
  return s ? `?${s}` : ''
}

export async function apiFetch<T = unknown>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers || {})
  const token = getAdminToken()
  if (token) headers.set('Authorization', `Bearer ${token}`)
  if (init.body && !(init.body instanceof FormData) && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json')
  }
  const res = await fetch(path, { ...init, headers })
  if (res.status === 401) {
    notifyUnauthorized()
    throw new ApiError(401, '未授权，请重新登录')
  }
  const text = await res.text()
  let data: any = null
  if (text) {
    try {
      data = JSON.parse(text)
    } catch {
      data = text
    }
  }
  if (!res.ok) {
    const msg = (data && typeof data === 'object' && (data.message || data.error)) || `HTTP ${res.status}`
    throw new ApiError(res.status, String(msg))
  }
  return data as T
}

export const api = {
  get: <T = unknown>(path: string, params?: Record<string, unknown>) =>
    apiFetch<T>(`${path}${buildQuery(params)}`),
  post: <T = unknown>(path: string, body?: unknown) =>
    apiFetch<T>(path, { method: 'POST', body: body === undefined ? undefined : JSON.stringify(body) }),
  put: <T = unknown>(path: string, body?: unknown) =>
    apiFetch<T>(path, { method: 'PUT', body: JSON.stringify(body ?? {}) }),
  patch: <T = unknown>(path: string, body?: unknown) =>
    apiFetch<T>(path, { method: 'PATCH', body: JSON.stringify(body ?? {}) }),
  delete: <T = unknown>(path: string, params?: Record<string, unknown>) =>
    apiFetch<T>(`${path}${buildQuery(params)}`, { method: 'DELETE' }),
}

/** 管理接口前缀 */
export const ADMIN = '/api/admin'

/** SSE 地址（EventSource 无法设置请求头，令牌走查询参数） */
export const sseUrl = (path: string, params: Record<string, unknown> = {}) =>
  `${path}${buildQuery({ ...params, token: getAdminToken() })}`

/** 登录：校验管理员令牌 */
export async function login(token: string): Promise<{ success: boolean; role?: string; name?: string; message?: string }> {
  const res = await fetch('/api/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ token }),
  })
  const data = await res.json().catch(() => ({ success: false, message: '登录失败' }))
  if (data.success && data.role === 'admin') setAdminToken(token)
  return data
}

/**
 * 读取 POST 型 SSE（fetch + ReadableStream），用于模型测试。
 * 回调收到 (eventName, data)。
 */
export async function postSse(
  path: string,
  body: unknown,
  onEvent: (event: string, data: any) => void,
  signal?: AbortSignal
): Promise<void> {
  const headers: Record<string, string> = { 'Content-Type': 'application/json', Accept: 'text/event-stream' }
  const token = getAdminToken()
  if (token) headers.Authorization = `Bearer ${token}`
  const res = await fetch(path, { method: 'POST', headers, body: JSON.stringify(body), signal })
  if (res.status === 401) {
    notifyUnauthorized()
    throw new ApiError(401, '未授权，请重新登录')
  }
  if (!res.ok || !res.body) {
    const text = await res.text().catch(() => '')
    throw new ApiError(res.status, text || `HTTP ${res.status}`)
  }
  const reader = res.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ''
  const flush = (frame: string) => {
    let event = 'message'
    const dataLines: string[] = []
    for (const line of frame.split('\n')) {
      if (line.startsWith('event:')) event = line.slice(6).trim()
      else if (line.startsWith('data:')) dataLines.push(line.slice(5).trimStart())
    }
    if (!dataLines.length) return
    const raw = dataLines.join('\n')
    let data: any = raw
    try {
      data = JSON.parse(raw)
    } catch {
      /* 纯文本 */
    }
    onEvent(event, data)
  }
  while (true) {
    const { value, done } = await reader.read()
    if (done) break
    buffer += decoder.decode(value, { stream: true })
    let idx: number
    while ((idx = buffer.indexOf('\n\n')) >= 0) {
      const frame = buffer.slice(0, idx).replace(/\r/g, '')
      buffer = buffer.slice(idx + 2)
      if (frame.trim()) flush(frame)
    }
  }
  if (buffer.trim()) flush(buffer.replace(/\r/g, ''))
}
