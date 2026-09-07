//! 全局共享状态。

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;

use crate::config::{AppSettings, ModelSettings, RuntimeConfig};
use crate::db::Db;
use crate::logger::RequestLogger;

pub struct AppStateInner {
    pub runtime: RuntimeConfig,
    pub db: Db,
    pub settings: RwLock<AppSettings>,
    pub model_settings: RwLock<ModelSettings>,
    pub logger: RequestLogger,
    pub http: reqwest::Client,
    pub started_at: Instant,
    /// 最近一次 OCS HEAD / 探测的 unix 秒；0 表示从未
    pub last_ocs_contact_at: AtomicI64,
}

pub type AppState = Arc<AppStateInner>;

impl AppStateInner {
    pub fn settings(&self) -> AppSettings {
        self.settings.read().clone()
    }

    pub fn model_settings(&self) -> ModelSettings {
        self.model_settings.read().clone()
    }

    /// 保存 AppSettings（内存 + 文件）
    pub fn save_settings(&self, new: AppSettings) -> anyhow::Result<AppSettings> {
        crate::config::write_json_file(&self.runtime.settings_path(), &new)?;
        *self.settings.write() = new.clone();
        Ok(new)
    }

    /// 保存 ModelSettings（内存 + 文件）
    pub fn save_model_settings(&self, new: ModelSettings) -> anyhow::Result<ModelSettings> {
        crate::config::write_json_file(&self.runtime.model_config_path(), &new)?;
        *self.model_settings.write() = new.clone();
        Ok(new)
    }

    pub fn touch_ocs_contact(&self) {
        self.last_ocs_contact_at
            .store(chrono::Utc::now().timestamp(), Ordering::Relaxed);
    }

    pub fn last_ocs_contact(&self) -> Option<i64> {
        let v = self.last_ocs_contact_at.load(Ordering::Relaxed);
        (v > 0).then_some(v)
    }
}
