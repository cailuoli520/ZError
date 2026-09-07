//! AI 层公共类型。协议实现见 `protocol.rs`，编排见 `engine.rs`，图片处理见 `image.rs`。

pub mod engine;
pub mod image;
pub mod protocol;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::config::{ApiProtocol, ResolvedModel};

/// 多模态消息内容片段
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    /// data: URL 或 http(s) URL
    #[serde(rename = "image_url")]
    ImageUrl {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl MessageContent {
    /// 拼接出纯文本（图片片段忽略）
    pub fn plain_text(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Parts(parts) => parts
                .iter()
                .filter_map(|p| match p {
                    ContentPart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

/// role: system | user | assistant
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    pub content: MessageContent,
}

impl ChatMessage {
    pub fn system(text: impl Into<String>) -> Self {
        Self { role: "system".into(), content: MessageContent::Text(text.into()) }
    }
    pub fn user(text: impl Into<String>) -> Self {
        Self { role: "user".into(), content: MessageContent::Text(text.into()) }
    }
    pub fn assistant(text: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: MessageContent::Text(text.into()) }
    }
    pub fn user_parts(parts: Vec<ContentPart>) -> Self {
        Self { role: "user".into(), content: MessageContent::Parts(parts) }
    }
}

/// 思考模式配置（由 AIModel 字段推导）
#[derive(Debug, Clone)]
pub struct ThinkingConfig {
    pub enabled: bool,
    /// low | medium | high | max（enabled 时）
    pub effort: String,
    /// enabled=false 时 openai-chat 是否发送 enable_thinking:false（默认 true）
    pub off_enable_thinking_false: bool,
    /// enabled=false 时 openai-chat 是否发送 thinking:{type:"disabled"}（默认 false）
    pub off_thinking_type_disabled: bool,
    /// enabled=false 时 openai-response 的 reasoning.effort：none | minimal（默认 minimal）
    pub off_responses_effort: String,
}

/// 单次模型调用配置（已把平台与模型信息展平）
#[derive(Debug, Clone)]
pub struct ModelCallConfig {
    pub protocol: ApiProtocol,
    pub base_url: String,
    pub api_key: String,
    pub custom_headers: HashMap<String, String>,
    pub model_id: String,
    pub temperature: f64,
    pub top_p: f64,
    pub max_tokens: u32,
    pub thinking: ThinkingConfig,
}

impl From<&ResolvedModel> for ModelCallConfig {
    fn from(r: &ResolvedModel) -> Self {
        let m = &r.model;
        Self {
            protocol: m.protocol(),
            base_url: r.platform.base_url.trim().trim_end_matches('/').to_string(),
            api_key: r.platform.api_key.trim().to_string(),
            custom_headers: r.platform.custom_headers.clone().unwrap_or_default(),
            model_id: m.wire_model_id().to_string(),
            temperature: m.temperature,
            top_p: m.top_p,
            max_tokens: m.max_tokens,
            thinking: ThinkingConfig {
                enabled: m.thinking_enabled(),
                effort: m.thinking_effort.clone().unwrap_or_else(|| "medium".into()),
                off_enable_thinking_false: m.thinking_off_enable_thinking_false.unwrap_or(true),
                off_thinking_type_disabled: m.thinking_off_thinking_type_disabled.unwrap_or(false),
                off_responses_effort: m
                    .thinking_off_responses_effort
                    .clone()
                    .unwrap_or_else(|| "minimal".into()),
            },
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ModelCallInput {
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
}

/// 归一化后的增量块
#[derive(Debug, Clone, Default, Serialize)]
pub struct ModelChunk {
    pub content: String,
    pub reasoning_content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ModelCallError {
    #[error("HTTP {status}: {message}")]
    Http { status: u16, message: String },
    #[error("请求失败: {0}")]
    Transport(String),
    #[error("模型响应超时")]
    Timeout,
    #[error("响应解析失败: {0}")]
    Parse(String),
    #[error("配置错误: {0}")]
    Config(String),
}

/// 一次完整调用的聚合结果
#[derive(Debug, Clone, Default, Serialize)]
pub struct ModelCallOutput {
    pub content: String,
    pub reasoning_content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<serde_json::Value>,
    pub elapsed_ms: u64,
}
