/**
 * 题库数据访问层：REST 客户端（原 Tauri invoke 已全部替换）。
 * 返回体沿用服务端的 snake_case 字段。
 */
import { api, ADMIN } from './api'

export interface Folder {
  id: number
  name: string
  parent_id?: number | null
  created_at?: string
}

export interface AIResponse {
  id: number
  question: string
  options?: string
  answer?: string
  question_type: string
  folder_id: number
  folder_name?: string
  create_time: string
  is_ai?: boolean
  is_pending_correction?: boolean
}

export interface PaginatedAIResponses {
  items: AIResponse[]
  total: number
}

export interface SearchResult {
  items: AIResponse[]
  terms: string[]
}

export interface NewQuestionInput {
  content: string
  options?: string | null
  answer?: string | null
  questionType?: string | null
  folderId: number
  isAi?: boolean
}

const Q = `${ADMIN}/questions`
const F = `${ADMIN}/folders`

const normalizeItem = (r: any): AIResponse => ({
  ...r,
  question_type: r.question_type ?? '',
  create_time: r.create_time ?? '',
})

class DatabaseService {
  /** 兼容旧接口：无需连接 */
  async connect(): Promise<void> {}
  async ensureConnection(): Promise<void> {}

  async getFolders(): Promise<Folder[]> {
    try {
      const folders = await api.get<any[]>(F)
      return folders.map(f => ({
        id: f.id,
        name: f.name,
        parent_id: f.parent_id === 0 ? null : f.parent_id,
        created_at: f.create_time,
      }))
    } catch (error) {
      console.error('获取文件夹失败:', error)
      return []
    }
  }

  async getAIResponses(folderId?: number): Promise<AIResponse[]> {
    try {
      const items = await api.get<any[]>(`${Q}/export`, { folderId, recursive: false })
      return items.map(normalizeItem)
    } catch (error) {
      console.error('获取题目失败:', error)
      return []
    }
  }

  async getPaginatedQuestions(params: {
    folderId?: number
    pendingCorrectionOnly?: boolean
    page: number
    pageSize: number
    sortOrder?: 'desc' | 'asc'
    recursive?: boolean
  }): Promise<PaginatedAIResponses> {
    const { folderId, pendingCorrectionOnly = false, page, pageSize, sortOrder = 'desc', recursive = false } = params
    try {
      const res = await api.get<PaginatedAIResponses>(Q, {
        folderId,
        recursive,
        pendingOnly: pendingCorrectionOnly,
        page,
        pageSize,
        sortOrder,
      })
      return { items: (res.items || []).map(normalizeItem), total: res.total || 0 }
    } catch (error) {
      console.error('分页获取题目失败:', error)
      return { items: [], total: 0 }
    }
  }

  /** 文件夹及其所有子文件夹的题目 */
  async getQuestionsFromFolderAndSubfolders(folderId: number): Promise<AIResponse[]> {
    try {
      const items = await api.get<any[]>(`${Q}/export`, { folderId, recursive: true })
      return items.map(normalizeItem)
    } catch (error) {
      console.error('获取文件夹及子文件夹题目失败:', error)
      return []
    }
  }

  async getPendingCorrectionQuestions(): Promise<AIResponse[]> {
    try {
      const items = await api.get<any[]>(`${Q}/export`, { pendingOnly: true })
      return items.map(normalizeItem)
    } catch (error) {
      console.error('获取待修正题目失败:', error)
      return []
    }
  }

  async getPendingCorrectionQuestionCount(): Promise<number> {
    try {
      const res = await api.get<{ count: number }>(`${Q}/pending-count`)
      return res.count || 0
    } catch (error) {
      console.error('获取待修正题目数量失败:', error)
      return 0
    }
  }

  async setQuestionPendingCorrection(questionId: number, pending: boolean): Promise<void> {
    await api.post(`${Q}/${questionId}/pending`, { pending })
  }

  async getFolderQuestionCount(folderId: number): Promise<number> {
    try {
      const res = await api.get<{ count: number }>(`${F}/${folderId}/count`)
      return res.count || 0
    } catch (error) {
      console.error('获取文件夹题目数量失败:', error)
      return 0
    }
  }

  /** 模糊搜索（服务端分词），返回结果与高亮词 */
  async searchQuestions(searchTerm: string, folderId?: number): Promise<SearchResult> {
    try {
      const res = await api.get<SearchResult>(`${Q}/search`, { q: searchTerm, folderId })
      return { items: (res.items || []).map(normalizeItem), terms: res.terms || [] }
    } catch (error) {
      console.error('搜索题目失败:', error)
      return { items: [], terms: [] }
    }
  }

  async searchQuestionsByTitle(searchTerm: string, folderId?: number): Promise<AIResponse[]> {
    return (await this.searchQuestions(searchTerm, folderId)).items
  }

  /** 分词（供搜索高亮） */
  async segmentText(text: string): Promise<string[]> {
    try {
      return await api.get<string[]>(`${ADMIN}/segment`, { text })
    } catch {
      return []
    }
  }

  async getFolderPath(folderId: number): Promise<{ id: number; name: string }[]> {
    try {
      return await api.get<{ id: number; name: string }[]>(`${F}/${folderId}/path`)
    } catch (error) {
      console.error('获取文件夹路径失败:', error)
      return []
    }
  }

  async getFolderStats(): Promise<{ folderId: number; folderName: string; questionCount: number }[]> {
    try {
      const stats = await api.get<any[]>(`${F}/stats`)
      return stats.map(s => ({ folderId: s.folder_id, folderName: s.folder_name, questionCount: s.question_count }))
    } catch (error) {
      console.error('获取文件夹统计失败:', error)
      return []
    }
  }

  async copyQuestionToFolder(questionId: number, targetFolderId: number): Promise<void> {
    await api.post(`${Q}/${questionId}/copy`, { targetFolderId })
  }

  async moveQuestionToFolder(questionId: number, targetFolderId: number): Promise<void> {
    await api.post(`${Q}/${questionId}/move`, { targetFolderId })
  }

  async addQuestion(questionData: {
    content: string
    options?: string
    answer: string
    question_type?: string
    folderId: string | number
    isAi?: number | boolean
  }): Promise<AIResponse> {
    const folderId = typeof questionData.folderId === 'string' ? parseInt(questionData.folderId) : questionData.folderId
    const res = await api.post<{ id: number; item: AIResponse }>(Q, {
      content: questionData.content,
      options: questionData.options || null,
      answer: questionData.answer,
      questionType: questionData.question_type || null,
      folderId: Number.isFinite(folderId) ? folderId : 0,
      isAi: questionData.isAi === 1 || questionData.isAi === true,
    })
    return normalizeItem(res.item)
  }

  /** 批量导入（单事务） */
  async addQuestionsBulk(items: NewQuestionInput[]): Promise<{ inserted: number; ids: number[] }> {
    return api.post<{ inserted: number; ids: number[] }>(`${Q}/bulk`, {
      items: items.map(q => ({
        content: q.content,
        options: q.options || null,
        answer: q.answer || '',
        questionType: q.questionType || null,
        folderId: q.folderId,
        isAi: !!q.isAi,
      })),
    })
  }

  /** 更新题目：服务端为全量更新，缺省字段先读取原值 */
  async updateQuestion(
    questionId: number,
    updateData: { question?: string; options?: string | null; answer?: string; question_type?: string; folderId?: number },
    current?: Partial<AIResponse>
  ): Promise<void> {
    const base = current || {}
    await api.patch(`${Q}/${questionId}`, {
      content: updateData.question ?? base.question ?? '',
      options: updateData.options === undefined ? base.options ?? null : updateData.options,
      answer: updateData.answer ?? base.answer ?? '',
      questionType: updateData.question_type ?? base.question_type ?? null,
      folderId: updateData.folderId ?? base.folder_id ?? -1,
    })
  }

  async deleteQuestion(id: number): Promise<void> {
    await api.delete(`${Q}/${id}`)
  }

  async deleteQuestions(ids: number[]): Promise<void> {
    await api.post(`${Q}/batch-delete`, { ids })
  }

  async createFolder(name: string, parentId: number = 0): Promise<number> {
    const res = await api.post<{ id: number }>(F, { name, parentId })
    return res.id
  }

  async renameFolder(id: number, newName: string): Promise<void> {
    await api.patch(`${F}/${id}`, { name: newName })
  }

  async deleteFolder(id: number, deleteQuestions: boolean): Promise<void> {
    await api.delete(`${F}/${id}`, { deleteQuestions })
  }

  async clearFolderQuestions(id: number): Promise<void> {
    await api.post(`${F}/${id}/clear`)
  }

  async moveFolder(id: number, parentId: number, _position?: number): Promise<void> {
    await api.patch(`${F}/${id}`, { parentId })
  }
}

export const databaseService = new DatabaseService()
