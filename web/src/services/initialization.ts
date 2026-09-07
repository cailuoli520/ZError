import { settingsManager } from './settings';
import { modelConfigManager } from './modelConfig';

/**
 * 应用初始化服务（登录成功后调用）
 */
export class InitializationService {
  private isInitialized = false;

  /**
   * 初始化应用：从服务端拉取设置与模型配置，并尝试同步远程平台目录
   */
  public async initialize(): Promise<void> {
    if (this.isInitialized) {
      return;
    }

    console.log('开始应用初始化...');

    await settingsManager.init();
    await modelConfigManager.init();

    try {
      await modelConfigManager.syncRemotePlatforms();
    } catch (error) {
      console.warn('同步远程平台目录失败:', error);
    }

    this.isInitialized = true;
    console.log('应用初始化完成');
  }

  /** 退出登录后重置，允许再次登录时重新初始化 */
  public reset(): void {
    this.isInitialized = false;
  }
}

export const initializationService = new InitializationService();
