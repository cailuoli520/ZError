/**
 * 应用内事件总线（替代原 Tauri 事件 emit/listen）。
 */
import mitt from 'mitt'

export type BusEvents = {
  /** 打开导入对话框（携带已解析的题目） */
  'open-import-dialog': { items: any[] }
  /** 题库数据变更，需刷新 */
  'refresh-data': void
  /** 新题目已添加 */
  'new-question-added': void
  /** 主题变更 */
  'theme-changed': { theme: string }
  /** 登录成功 */
  'auth-login': void
  /** 会话失效 */
  'auth-logout': void
}

export const bus = mitt<BusEvents>()
