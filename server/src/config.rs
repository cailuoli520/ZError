//! 运行时配置（环境变量）与持久化设置（settings.json / model_config.json）。
//!
//! AppSettings / ModelSettings 的字段名与前端 `settings.ts` / `modelConfig.ts` 保持一致，
//! 以便管理后台直接读写；未知字段通过 `serde(flatten)` 的 `extra` 透传保存。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

/// 环境变量驱动的运行时配置
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub data_dir: PathBuf,
    pub bind: SocketAddr,
    pub admin_token_env: Option<String>,
    pub public_url: Option<String>,
    pub trust_proxy: bool,
}

impl RuntimeConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let data_dir = std::env::var("ZERROR_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data"));
        let bind: SocketAddr = std::env::var("ZERROR_BIND")
            .unwrap_or_else(|_| "0.0.0.0:3000".to_string())
            .parse()
            .map_err(|e| anyhow::anyhow!("ZERROR_BIND 格式错误: {e}"))?;
        let admin_token_env = std::env::var("ZERROR_ADMIN_TOKEN")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let public_url = std::env::var("ZERROR_PUBLIC_URL")
            .ok()
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty());
        let trust_proxy = std::env::var("ZERROR_TRUST_PROXY")
            .map(|v| !matches!(v.trim().to_lowercase().as_str(), "0" | "false" | "no"))
            .unwrap_or(true);
        Ok(Self {
            data_dir,
            bind,
            admin_token_env,
            public_url,
            trust_proxy,
        })
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("airesponses.db")
    }
    pub fn settings_path(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }
    pub fn model_config_path(&self) -> PathBuf {
        self.data_dir.join("model_config.json")
    }
    pub fn image_cache_dir(&self) -> PathBuf {
        self.data_dir.join("image_cache")
    }
}

// ---------------------------------------------------------------------------
// AppSettings（settings.json）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserConfig {
    pub id: String,
    pub name: String,
    pub token: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MultiUserConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub users: Vec<UserConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_true")]
    pub auto_save: bool,
    #[serde(default = "default_true")]
    pub auto_add_to_question_bank: bool,
    /// 文本模型最长响应时间（秒）
    #[serde(default = "default_timeout")]
    pub model_response_timeout: u64,
    /// 失败自动重试次数（仅单模型 / 总结模型 / 视觉模型）
    #[serde(default = "default_retry")]
    pub model_retry_count: u64,
    #[serde(default = "default_difficulty")]
    pub default_difficulty: String,
    #[serde(default = "default_items_per_page")]
    pub items_per_page: u32,
    #[serde(default = "default_true")]
    pub show_explanation: bool,
    #[serde(default)]
    pub suppress_no_model_warning: bool,
    #[serde(default)]
    pub question_save_folder_id: Option<i64>,
    /// 管理员令牌
    #[serde(default)]
    pub admin_token: String,
    /// 公网 /query 是否必须携带令牌
    #[serde(default = "default_true")]
    pub public_query_require_token: bool,
    /// 查询令牌（每个 user.token 可访问 /query）
    #[serde(default)]
    pub multi_user: MultiUserConfig,
    /// 未知字段透传
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_theme() -> String {
    "light".into()
}
fn default_language() -> String {
    "zh-CN".into()
}
fn default_true() -> bool {
    true
}
fn default_timeout() -> u64 {
    40
}
fn default_retry() -> u64 {
    2
}
fn default_difficulty() -> String {
    "medium".into()
}
fn default_items_per_page() -> u32 {
    20
}

impl Default for AppSettings {
    fn default() -> Self {
        serde_json::from_str("{}").expect("AppSettings 默认值")
    }
}

impl AppSettings {
    /// 模型等待预算：(静默超时秒, 绝对上限秒)，与上游 model_wait_budget_secs 一致
    pub fn model_wait_budget_secs(&self, has_url: bool) -> (u64, u64) {
        let timeout = self.model_response_timeout.clamp(5, 600);
        let retries = self.model_retry_count.min(10);
        let inactivity = timeout.saturating_add(20).max(30);
        let absolute = timeout
            .saturating_mul(retries + 1)
            .saturating_add(if has_url { 90 } else { 60 })
            .max(inactivity);
        (inactivity, absolute)
    }

    pub fn model_timeout_secs(&self) -> u64 {
        self.model_response_timeout.clamp(5, 600)
    }

    pub fn model_retry_count(&self) -> u64 {
        self.model_retry_count.min(10)
    }
}

// ---------------------------------------------------------------------------
// ModelSettings（model_config.json）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ApiProtocol {
    #[default]
    #[serde(rename = "openai-chat")]
    OpenAiChat,
    #[serde(rename = "openai-response")]
    OpenAiResponse,
    #[serde(rename = "anthropic")]
    Anthropic,
    /// 已废弃：服务端不执行 JS，按 openai-chat 处理
    #[serde(rename = "custom")]
    Custom,
}

impl ApiProtocol {
    /// 服务端实际使用的协议（custom → openai-chat）
    pub fn effective(self) -> ApiProtocol {
        match self {
            ApiProtocol::Custom => ApiProtocol::OpenAiChat,
            other => other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AIModel {
    pub id: String,
    /// 实际发给供应商的模型名；缺省回落 id
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub platform_id: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default = "default_top_p")]
    pub top_p: f64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// text | vision | summary
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub is_remote: Option<bool>,
    #[serde(default)]
    pub enable_thinking: Option<bool>,
    #[serde(default)]
    pub thinking_off_enable_thinking_false: Option<bool>,
    #[serde(default)]
    pub thinking_off_thinking_type_disabled: Option<bool>,
    /// none | minimal
    #[serde(default)]
    pub thinking_off_responses_effort: Option<String>,
    /// low | medium | high | max
    #[serde(default)]
    pub thinking_effort: Option<String>,
    #[serde(default)]
    pub api_protocol: Option<ApiProtocol>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_max_tokens() -> u32 {
    4096
}
fn default_temperature() -> f64 {
    0.7
}
fn default_top_p() -> f64 {
    0.9
}
fn default_category() -> String {
    "text".into()
}

impl AIModel {
    pub fn wire_model_id(&self) -> &str {
        self.model_id
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&self.id)
    }

    pub fn protocol(&self) -> ApiProtocol {
        self.api_protocol.unwrap_or_default().effective()
    }

    pub fn label(&self) -> &str {
        if !self.display_name.trim().is_empty() {
            &self.display_name
        } else if !self.name.trim().is_empty() {
            &self.name
        } else {
            &self.id
        }
    }

    /// 思考模式自动探测（与前端 modelConfig.ts 一致）：未显式设置时按名称推断
    pub fn thinking_enabled(&self) -> bool {
        if let Some(v) = self.enable_thinking {
            return v;
        }
        let hay = format!("{} {}", self.id, self.name).to_lowercase();
        hay.contains("reasoner")
            || hay.contains("-r1")
            || hay.contains("thinking")
            || hay.contains("deepseek-r1")
            || hay.starts_with("o1")
            || hay.starts_with("o3")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AIPlatform {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub models: Vec<AIModel>,
    #[serde(default)]
    pub custom_headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub is_remote: Option<bool>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalModelSettings {
    /// 毫秒（前端历史字段，服务端以 AppSettings.modelResponseTimeout 为准）
    #[serde(default = "default_global_timeout")]
    pub timeout: u64,
    #[serde(default = "default_global_retry")]
    pub retry_count: u64,
    #[serde(default)]
    pub enable_logging: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_global_timeout() -> u64 {
    30000
}
fn default_global_retry() -> u64 {
    3
}

impl Default for GlobalModelSettings {
    fn default() -> Self {
        serde_json::from_str("{}").expect("GlobalModelSettings 默认值")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSettings {
    #[serde(default)]
    pub selected_text_model: Option<String>,
    #[serde(default)]
    pub selected_text_models: Vec<String>,
    #[serde(default)]
    pub selected_summary_model: Option<String>,
    #[serde(default)]
    pub selected_summary_models: Vec<String>,
    #[serde(default)]
    pub selected_vision_model: Option<String>,
    #[serde(default)]
    pub platforms: Vec<AIPlatform>,
    #[serde(default)]
    pub global_settings: GlobalModelSettings,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Default for ModelSettings {
    fn default() -> Self {
        serde_json::from_str("{}").expect("ModelSettings 默认值")
    }
}

/// 已解析出平台的模型引用
#[derive(Debug, Clone)]
pub struct ResolvedModel {
    pub platform: AIPlatform,
    pub model: AIModel,
}

impl ModelSettings {
    pub fn find_model(&self, model_id: &str) -> Option<ResolvedModel> {
        for p in &self.platforms {
            if let Some(m) = p.models.iter().find(|m| m.id == model_id) {
                let mut platform = p.clone();
                platform.models = Vec::new();
                return Some(ResolvedModel {
                    platform,
                    model: m.clone(),
                });
            }
        }
        None
    }

    fn resolve_enabled(&self, ids: &[String]) -> Vec<ResolvedModel> {
        ids.iter()
            .filter_map(|id| self.find_model(id))
            .filter(|r| r.platform.enabled && r.model.enabled)
            .collect()
    }

    /// 选中的文本模型（≤5，兼容旧的单值字段）
    pub fn selected_text_models(&self) -> Vec<ResolvedModel> {
        let mut ids = self.selected_text_models.clone();
        if ids.is_empty() {
            if let Some(single) = &self.selected_text_model {
                ids.push(single.clone());
            }
        }
        ids.truncate(5);
        self.resolve_enabled(&ids)
    }

    pub fn selected_summary_models(&self) -> Vec<ResolvedModel> {
        let mut ids = self.selected_summary_models.clone();
        if ids.is_empty() {
            if let Some(single) = &self.selected_summary_model {
                ids.push(single.clone());
            }
        }
        self.resolve_enabled(&ids)
    }

    pub fn selected_vision_model(&self) -> Option<ResolvedModel> {
        let id = self.selected_vision_model.as_deref()?;
        self.resolve_enabled(&[id.to_string()]).into_iter().next()
    }
}

// ---------------------------------------------------------------------------
// JSON 文件读写（原子写入）
// ---------------------------------------------------------------------------

pub fn read_json_file<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> T {
    match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            tracing::warn!("解析 {} 失败，使用默认值: {}", path.display(), e);
            T::default()
        }),
        Err(_) => T::default(),
    }
}

pub fn write_json_file<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_string_pretty(value)?;
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// 生成随机令牌（32 字节十六进制）
pub fn generate_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_settings_defaults() {
        let s = AppSettings::default();
        assert_eq!(s.model_response_timeout, 40);
        assert!(s.public_query_require_token);
        assert_eq!(s.model_wait_budget_secs(false), (60, 180));
    }

    #[test]
    fn model_settings_roundtrip_keeps_unknown_fields() {
        let raw = r#"{"platforms":[{"id":"p1","baseUrl":"https://x","apiKey":"k","models":[{"id":"m1","category":"text","apiProtocol":"custom","jsCode":"xx"}]}],"selectedTextModels":["m1"],"deletedPredefinedPlatforms":["a"]}"#;
        let ms: ModelSettings = serde_json::from_str(raw).unwrap();
        assert_eq!(ms.selected_text_models().len(), 1);
        assert_eq!(ms.selected_text_models()[0].model.protocol(), ApiProtocol::OpenAiChat);
        let out = serde_json::to_value(&ms).unwrap();
        assert!(out.get("deletedPredefinedPlatforms").is_some());
        assert!(out["platforms"][0]["models"][0].get("jsCode").is_some());
    }
}
