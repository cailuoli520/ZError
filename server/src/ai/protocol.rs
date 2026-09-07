//! 三种供应商协议的请求构造与响应归一化（移植 modelProtocol.ts）。

use std::collections::HashMap;
use std::pin::Pin;
use std::time::{Duration, Instant};

use futures::Stream;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use serde_json::{json, Value};

use super::{ContentPart, MessageContent, ModelCallConfig, ModelCallError, ModelCallInput, ModelChunk, ModelCallOutput};
#[cfg(test)]
use super::ChatMessage;
use crate::config::ApiProtocol;

pub type ChunkStream = Pin<Box<dyn Stream<Item = Result<ModelChunk, ModelCallError>> + Send>>;

/// 拼接端点：baseUrl 去尾部斜杠 + 固定路径（与上游一致，不做 /v1 去重）
pub fn resolve_endpoint(base_url: &str, protocol: ApiProtocol) -> String {
    let base = base_url.trim().trim_end_matches('/');
    let path = match protocol.effective() {
        ApiProtocol::OpenAiChat | ApiProtocol::Custom => "/v1/chat/completions",
        ApiProtocol::OpenAiResponse => "/v1/responses",
        ApiProtocol::Anthropic => "/v1/messages",
    };
    // 兼容用户把 /v1 写进 baseUrl 的情况
    if base.ends_with("/v1") {
        format!("{}{}", &base[..base.len() - 3], path)
    } else {
        format!("{base}{path}")
    }
}

fn build_headers(cfg: &ModelCallConfig, extra: &[(&str, &str)]) -> Result<HeaderMap, ModelCallError> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    for (k, v) in extra {
        let name = HeaderName::from_bytes(k.as_bytes()).map_err(|e| ModelCallError::Config(e.to_string()))?;
        headers.insert(name, HeaderValue::from_str(v).map_err(|e| ModelCallError::Config(e.to_string()))?);
    }
    if !cfg.api_key.is_empty() {
        let value = HeaderValue::from_str(&format!("Bearer {}", cfg.api_key))
            .map_err(|_| ModelCallError::Config("API Key 含非法字符".into()))?;
        headers.insert(reqwest::header::AUTHORIZATION, value);
        if cfg.protocol.effective() == ApiProtocol::Anthropic {
            headers.insert(
                HeaderName::from_static("x-api-key"),
                HeaderValue::from_str(&cfg.api_key).map_err(|_| ModelCallError::Config("API Key 含非法字符".into()))?,
            );
        }
    }
    for (k, v) in &cfg.custom_headers {
        let Ok(name) = HeaderName::from_bytes(k.trim().as_bytes()) else {
            continue;
        };
        if let Ok(value) = HeaderValue::from_str(v) {
            headers.insert(name, value);
        }
    }
    Ok(headers)
}

fn content_to_openai(content: &MessageContent) -> Value {
    match content {
        MessageContent::Text(s) => Value::String(s.clone()),
        MessageContent::Parts(parts) => Value::Array(
            parts
                .iter()
                .map(|p| match p {
                    ContentPart::Text { text } => json!({"type": "text", "text": text}),
                    ContentPart::ImageUrl { url, detail } => {
                        let mut img = json!({"url": url});
                        if let Some(d) = detail {
                            img["detail"] = Value::String(d.clone());
                        }
                        json!({"type": "image_url", "image_url": img})
                    }
                })
                .collect(),
        ),
    }
}

fn content_to_responses(content: &MessageContent) -> Value {
    match content {
        MessageContent::Text(s) => json!([{"type": "input_text", "text": s}]),
        MessageContent::Parts(parts) => Value::Array(
            parts
                .iter()
                .map(|p| match p {
                    ContentPart::Text { text } => json!({"type": "input_text", "text": text}),
                    ContentPart::ImageUrl { url, detail } => json!({
                        "type": "input_image",
                        "image_url": url,
                        "detail": detail.clone().unwrap_or_else(|| "auto".into())
                    }),
                })
                .collect(),
        ),
    }
}

fn content_to_anthropic(content: &MessageContent) -> Value {
    match content {
        MessageContent::Text(s) => Value::String(s.clone()),
        MessageContent::Parts(parts) => Value::Array(
            parts
                .iter()
                .filter_map(|p| match p {
                    ContentPart::Text { text } => Some(json!({"type": "text", "text": text})),
                    ContentPart::ImageUrl { url, .. } => {
                        let rest = url.strip_prefix("data:")?;
                        let (media_type, data) = rest.split_once(";base64,")?;
                        Some(json!({
                            "type": "image",
                            "source": {"type": "base64", "media_type": media_type, "data": data}
                        }))
                    }
                })
                .collect(),
        ),
    }
}

/// 构造 (url, headers, body)
pub fn build_request(cfg: &ModelCallConfig, input: &ModelCallInput) -> Result<(String, HeaderMap, Value), ModelCallError> {
    if cfg.base_url.is_empty() {
        return Err(ModelCallError::Config("平台 baseUrl 为空".into()));
    }
    if cfg.model_id.is_empty() {
        return Err(ModelCallError::Config("模型 ID 为空".into()));
    }
    let protocol = cfg.protocol.effective();
    let url = resolve_endpoint(&cfg.base_url, protocol);
    let t = &cfg.thinking;

    match protocol {
        ApiProtocol::OpenAiChat | ApiProtocol::Custom => {
            let messages: Vec<Value> = input
                .messages
                .iter()
                .map(|m| json!({"role": m.role, "content": content_to_openai(&m.content)}))
                .collect();
            let mut payload = json!({
                "model": cfg.model_id,
                "messages": messages,
                "stream": input.stream,
                "temperature": cfg.temperature,
                "top_p": cfg.top_p,
                "max_tokens": cfg.max_tokens,
            });
            if t.enabled {
                payload["reasoning_effort"] = Value::String(t.effort.clone());
            } else {
                if t.off_enable_thinking_false {
                    payload["enable_thinking"] = Value::Bool(false);
                }
                if t.off_thinking_type_disabled {
                    payload["thinking"] = json!({"type": "disabled"});
                }
            }
            Ok((url, build_headers(cfg, &[])?, payload))
        }
        ApiProtocol::OpenAiResponse => {
            let items: Vec<Value> = input
                .messages
                .iter()
                .map(|m| json!({"role": m.role, "content": content_to_responses(&m.content)}))
                .collect();
            let effort = if t.enabled { t.effort.clone() } else { t.off_responses_effort.clone() };
            let payload = json!({
                "model": cfg.model_id,
                "input": items,
                "stream": input.stream,
                "reasoning": {"effort": effort},
            });
            Ok((url, build_headers(cfg, &[])?, payload))
        }
        ApiProtocol::Anthropic => {
            let mut system_parts: Vec<String> = Vec::new();
            let mut messages: Vec<Value> = Vec::new();
            for m in &input.messages {
                if m.role == "system" {
                    let text = m.content.plain_text();
                    if !text.is_empty() {
                        system_parts.push(text);
                    }
                    continue;
                }
                let role = if m.role == "assistant" { "assistant" } else { "user" };
                messages.push(json!({"role": role, "content": content_to_anthropic(&m.content)}));
            }
            let mut payload = json!({
                "model": cfg.model_id,
                "max_tokens": cfg.max_tokens,
                "messages": messages,
                "stream": input.stream,
            });
            if !system_parts.is_empty() {
                payload["system"] = Value::String(system_parts.join("\n\n"));
            }
            if t.enabled {
                let budget = (cfg.max_tokens / 2).clamp(1024, 16000);
                payload["thinking"] = json!({"type": "enabled", "budget_tokens": budget});
                payload["temperature"] = json!(1);
            } else {
                payload["temperature"] = json!(cfg.temperature);
            }
            Ok((url, build_headers(cfg, &[("anthropic-version", "2023-06-01")])?, payload))
        }
    }
}

/// 从响应体提取错误详情（error.message | message | error | 原文）
fn http_error_message(status: u16, body: &str) -> String {
    let body = body.trim();
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .and_then(|m| m.as_str().map(|s| s.to_string()))
                .or_else(|| v.get("message").and_then(|m| m.as_str().map(|s| s.to_string())))
                .or_else(|| {
                    v.get("error").map(|e| match e {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                })
        })
        .unwrap_or_else(|| body.to_string());
    if detail.is_empty() {
        format!("HTTP {status}")
    } else {
        format!("HTTP {status}: {detail}")
    }
}

fn pick_str<'a>(v: &'a Value, paths: &[&str]) -> Option<&'a str> {
    paths
        .iter()
        .filter_map(|p| v.pointer(p).and_then(|x| x.as_str()))
        .find(|s| !s.is_empty())
}

fn join_text_parts(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::String(s) => s.clone(),
                Value::Object(_) => {
                    let ty = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    if (ty == "text" || ty == "output_text") && item.get("text").and_then(|t| t.as_str()).is_some() {
                        item["text"].as_str().unwrap_or("").to_string()
                    } else {
                        item.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string()
                    }
                }
                _ => String::new(),
            })
            .collect(),
        _ => String::new(),
    }
}

/// chat 协议（流式 delta 或非流式 message）归一化
pub fn normalize_openai_chunk(payload: &Value) -> Option<ModelChunk> {
    let choice = payload.get("choices").and_then(|c| c.get(0)).cloned().unwrap_or(Value::Null);
    let delta = choice.get("delta").cloned().unwrap_or(Value::Null);
    let message = choice.get("message").cloned().unwrap_or(Value::Null);
    let content_val = [&delta, &message, payload]
        .iter()
        .find_map(|v| v.get("content").filter(|c| !c.is_null()))
        .cloned()
        .or_else(|| payload.get("response").cloned())
        .unwrap_or(Value::Null);
    let content = join_text_parts(&content_val);
    let reasoning = pick_str(&delta, &["/reasoning_content", "/reasoning"])
        .or_else(|| pick_str(&message, &["/reasoning_content", "/reasoning"]))
        .or_else(|| pick_str(payload, &["/reasoning_content", "/reasoning"]))
        .unwrap_or("")
        .to_string();
    let usage = payload.get("usage").filter(|u| !u.is_null()).cloned();
    if content.is_empty() && reasoning.is_empty() && usage.is_none() {
        return None;
    }
    Some(ModelChunk { content, reasoning_content: reasoning, usage })
}

/// responses 协议流式事件归一化
pub fn normalize_responses_event(event: &Value) -> Option<ModelChunk> {
    let ty = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let delta = event.get("delta").and_then(|d| d.as_str());
    if ty == "response.output_text.delta" {
        return delta.map(|d| ModelChunk { content: d.to_string(), ..Default::default() });
    }
    if ty.contains("reasoning") {
        return delta.map(|d| ModelChunk { reasoning_content: d.to_string(), ..Default::default() });
    }
    if let Some(usage) = event.get("usage").filter(|u| !u.is_null()) {
        return Some(ModelChunk { usage: Some(usage.clone()), ..Default::default() });
    }
    if ty == "response.completed" {
        if let Some(usage) = event.pointer("/response/usage").filter(|u| !u.is_null()) {
            return Some(ModelChunk { usage: Some(usage.clone()), ..Default::default() });
        }
    }
    None
}

/// responses 协议非流式归一化
pub fn normalize_responses_payload(payload: &Value) -> Option<ModelChunk> {
    let mut content = payload.get("output_text").and_then(|t| t.as_str()).unwrap_or("").to_string();
    let outputs = payload
        .get("output")
        .or_else(|| payload.pointer("/response/output"))
        .and_then(|o| o.as_array())
        .cloned()
        .unwrap_or_default();
    if content.is_empty() {
        content = outputs
            .iter()
            .map(|item| match item.get("content").and_then(|c| c.as_array()) {
                Some(parts) => parts
                    .iter()
                    .map(|p| {
                        p.get("text")
                            .and_then(|t| t.as_str())
                            .or_else(|| p.get("content").and_then(|t| t.as_str()))
                            .unwrap_or("")
                    })
                    .collect::<String>(),
                None => item.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string(),
            })
            .collect();
    }
    let reasoning = pick_str(
        payload,
        &["/reasoning_content", "/reasoning", "/response/reasoning_content", "/response/reasoning"],
    )
    .unwrap_or("")
    .to_string();
    let usage = payload.get("usage").or_else(|| payload.pointer("/response/usage")).filter(|u| !u.is_null()).cloned();
    if content.is_empty() && reasoning.is_empty() && usage.is_none() {
        return None;
    }
    Some(ModelChunk { content, reasoning_content: reasoning, usage })
}

/// anthropic 流式事件归一化
pub fn normalize_anthropic_event(event: &Value) -> Option<ModelChunk> {
    let ty = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if ty == "content_block_delta" {
        let delta = event.get("delta")?;
        match delta.get("type").and_then(|t| t.as_str()).unwrap_or("") {
            "text_delta" => {
                return delta
                    .get("text")
                    .and_then(|t| t.as_str())
                    .map(|t| ModelChunk { content: t.to_string(), ..Default::default() })
            }
            "thinking_delta" => {
                return delta
                    .get("thinking")
                    .and_then(|t| t.as_str())
                    .map(|t| ModelChunk { reasoning_content: t.to_string(), ..Default::default() })
            }
            _ => return None,
        }
    }
    if ty == "message_delta" {
        if let Some(usage) = event.get("usage").filter(|u| !u.is_null()) {
            return Some(ModelChunk { usage: Some(usage.clone()), ..Default::default() });
        }
    }
    if ty == "error" {
        // 流内错误以 Parse 错误形式上抛由调用方处理；这里返回 None
        return None;
    }
    event
        .get("usage")
        .filter(|u| !u.is_null())
        .map(|u| ModelChunk { usage: Some(u.clone()), ..Default::default() })
}

/// anthropic 非流式归一化
pub fn normalize_anthropic_payload(payload: &Value) -> Option<ModelChunk> {
    let blocks = payload.get("content").and_then(|c| c.as_array()).cloned().unwrap_or_default();
    let content: String = blocks
        .iter()
        .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
        .map(|b| b.get("text").and_then(|t| t.as_str()).unwrap_or(""))
        .collect();
    let reasoning: String = blocks
        .iter()
        .filter(|b| matches!(b.get("type").and_then(|t| t.as_str()), Some("thinking") | Some("redacted_thinking")))
        .map(|b| {
            b.get("thinking")
                .and_then(|t| t.as_str())
                .or_else(|| b.get("text").and_then(|t| t.as_str()))
                .unwrap_or("")
        })
        .collect();
    let usage = payload.get("usage").filter(|u| !u.is_null()).cloned();
    if content.is_empty() && reasoning.is_empty() && usage.is_none() {
        return None;
    }
    Some(ModelChunk { content, reasoning_content: reasoning, usage })
}

/// SSE 分帧器：按空行切块，取 data: 行，忽略 [DONE]
#[derive(Default)]
pub struct SseFramer {
    buffer: String,
}

impl SseFramer {
    /// 喂入文本，返回完整帧的 data JSON 列表
    pub fn push(&mut self, text: &str) -> Vec<Value> {
        self.buffer.push_str(text);
        let normalized = self.buffer.replace("\r\n", "\n");
        let mut blocks: Vec<&str> = normalized.split("\n\n").collect();
        let rest = blocks.pop().unwrap_or("").to_string();
        let out = blocks.iter().filter_map(|b| Self::parse_block(b)).collect();
        self.buffer = rest;
        out
    }

    /// 流结束时冲刷残留
    pub fn finish(&mut self) -> Vec<Value> {
        let rest = std::mem::take(&mut self.buffer);
        if rest.trim().is_empty() {
            return Vec::new();
        }
        Self::parse_block(&rest).into_iter().collect()
    }

    fn parse_block(block: &str) -> Option<Value> {
        let data: Vec<&str> = block
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with("data:"))
            .map(|l| l[5..].trim())
            .collect();
        if data.is_empty() {
            return None;
        }
        let joined = data.join("\n");
        if joined.is_empty() || joined == "[DONE]" {
            return None;
        }
        serde_json::from_str(&joined).ok()
    }
}

fn normalize_stream_event(protocol: ApiProtocol, event: &Value) -> Option<ModelChunk> {
    match protocol.effective() {
        ApiProtocol::OpenAiChat | ApiProtocol::Custom => normalize_openai_chunk(event),
        ApiProtocol::OpenAiResponse => normalize_responses_event(event),
        ApiProtocol::Anthropic => normalize_anthropic_event(event),
    }
}

fn normalize_payload(protocol: ApiProtocol, payload: &Value) -> Option<ModelChunk> {
    match protocol.effective() {
        ApiProtocol::OpenAiChat | ApiProtocol::Custom => normalize_openai_chunk(payload),
        ApiProtocol::OpenAiResponse => normalize_responses_payload(payload),
        ApiProtocol::Anthropic => normalize_anthropic_payload(payload),
    }
}

/// 流内错误事件（如 OpenAI 兼容端点在 SSE 中返回 {"error":...}）
fn stream_error(event: &Value) -> Option<ModelCallError> {
    let err = event.get("error")?;
    if err.is_null() {
        return None;
    }
    let msg = err
        .get("message")
        .and_then(|m| m.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| err.to_string());
    Some(ModelCallError::Http { status: 0, message: msg })
}

/// 发起调用并返回归一化增量流。`timeout` 同时约束首字节与相邻两块之间的静默时间。
pub async fn call_model_stream(
    http: &reqwest::Client,
    cfg: &ModelCallConfig,
    input: &ModelCallInput,
    timeout: Duration,
) -> Result<ChunkStream, ModelCallError> {
    let (url, headers, body) = build_request(cfg, input)?;
    let protocol = cfg.protocol.effective();
    let stream_mode = input.stream;

    let send = http.post(&url).headers(headers).json(&body).send();
    let response = tokio::time::timeout(timeout, send)
        .await
        .map_err(|_| ModelCallError::Timeout)?
        .map_err(|e| ModelCallError::Transport(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(ModelCallError::Http { status: status.as_u16(), message: http_error_message(status.as_u16(), &text) });
    }

    let is_event_stream = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("text/event-stream"))
        .unwrap_or(false);

    if !(stream_mode && is_event_stream) {
        // 非流式：整体读取后归一化
        let text = tokio::time::timeout(timeout, response.text())
            .await
            .map_err(|_| ModelCallError::Timeout)?
            .map_err(|e| ModelCallError::Transport(e.to_string()))?;
        let payload: Value = serde_json::from_str(&text).map_err(|_| {
            ModelCallError::Parse(if text.is_empty() { "响应为空".into() } else { text.chars().take(500).collect() })
        })?;
        if let Some(err) = stream_error(&payload) {
            return Err(err);
        }
        let chunk = normalize_payload(protocol, &payload).unwrap_or_default();
        return Ok(Box::pin(futures::stream::once(async move { Ok(chunk) })));
    }

    let mut bytes = response.bytes_stream();
    let s = async_stream::stream! {
        let mut framer = SseFramer::default();
        loop {
            let next = tokio::time::timeout(timeout, bytes.next()).await;
            match next {
                Err(_) => {
                    yield Err(ModelCallError::Timeout);
                    break;
                }
                Ok(None) => {
                    for ev in framer.finish() {
                        if let Some(err) = stream_error(&ev) { yield Err(err); break; }
                        if let Some(c) = normalize_stream_event(protocol, &ev) { yield Ok(c); }
                    }
                    break;
                }
                Ok(Some(Err(e))) => {
                    yield Err(ModelCallError::Transport(e.to_string()));
                    break;
                }
                Ok(Some(Ok(chunk))) => {
                    let text = String::from_utf8_lossy(&chunk);
                    let mut failed = false;
                    for ev in framer.push(&text) {
                        if let Some(err) = stream_error(&ev) { yield Err(err); failed = true; break; }
                        if let Some(c) = normalize_stream_event(protocol, &ev) { yield Ok(c); }
                    }
                    if failed { break; }
                }
            }
        }
    };
    Ok(Box::pin(s))
}

/// 聚合调用：拼接全部增量，可选逐块回调（参数为累计内容/累计推理）
pub async fn call_model(
    http: &reqwest::Client,
    cfg: &ModelCallConfig,
    input: &ModelCallInput,
    timeout: Duration,
    mut on_progress: Option<Box<dyn FnMut(&str, &str) + Send>>,
) -> Result<ModelCallOutput, ModelCallError> {
    let started = Instant::now();
    let mut stream = call_model_stream(http, cfg, input, timeout).await?;
    let mut out = ModelCallOutput::default();
    while let Some(item) = stream.next().await {
        let chunk = item?;
        if !chunk.content.is_empty() {
            out.content.push_str(&chunk.content);
        }
        if !chunk.reasoning_content.is_empty() {
            out.reasoning_content.push_str(&chunk.reasoning_content);
        }
        if chunk.usage.is_some() {
            out.usage = chunk.usage;
        }
        if let Some(cb) = on_progress.as_mut() {
            if !chunk.content.is_empty() || !chunk.reasoning_content.is_empty() {
                cb(&out.content, &out.reasoning_content);
            }
        }
    }
    out.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(out)
}

/// 探测平台可用模型：依次 GET {base}/models、{base}/v1/models
pub async fn probe_models(
    http: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    custom_headers: &HashMap<String, String>,
) -> Result<Vec<String>, ModelCallError> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err(ModelCallError::Config("baseUrl 为空".into()));
    }
    let candidates = if base.ends_with("/v1") {
        vec![format!("{base}/models")]
    } else {
        vec![format!("{base}/models"), format!("{base}/v1/models")]
    };
    let mut last_err = ModelCallError::Transport("未知错误".into());
    for url in candidates {
        let mut req = http.get(&url).timeout(Duration::from_secs(20));
        if !api_key.trim().is_empty() {
            req = req.bearer_auth(api_key.trim());
        }
        for (k, v) in custom_headers {
            req = req.header(k.as_str(), v.as_str());
        }
        match req.send().await {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if !status.is_success() {
                    last_err = ModelCallError::Http { status: status.as_u16(), message: http_error_message(status.as_u16(), &text) };
                    continue;
                }
                let Ok(v) = serde_json::from_str::<Value>(&text) else {
                    last_err = ModelCallError::Parse("模型列表不是合法 JSON".into());
                    continue;
                };
                let list = v
                    .get("data")
                    .and_then(|d| d.as_array())
                    .or_else(|| v.get("models").and_then(|d| d.as_array()))
                    .or_else(|| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let mut ids: Vec<String> = list
                    .iter()
                    .filter_map(|m| {
                        m.get("id")
                            .and_then(|x| x.as_str())
                            .or_else(|| m.get("name").and_then(|x| x.as_str()))
                            .or_else(|| m.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect();
                ids.sort();
                ids.dedup();
                return Ok(ids);
            }
            Err(e) => {
                last_err = ModelCallError::Transport(e.to_string());
            }
        }
    }
    Err(last_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ThinkingConfig;

    fn cfg(protocol: ApiProtocol, enabled: bool) -> ModelCallConfig {
        ModelCallConfig {
            protocol,
            base_url: "https://api.example.com".into(),
            api_key: "k".into(),
            custom_headers: HashMap::from([("X-Custom".to_string(), "1".to_string())]),
            model_id: "m".into(),
            temperature: 0.7,
            top_p: 0.9,
            max_tokens: 4096,
            thinking: ThinkingConfig {
                enabled,
                effort: "high".into(),
                off_enable_thinking_false: true,
                off_thinking_type_disabled: true,
                off_responses_effort: "none".into(),
            },
        }
    }

    fn input() -> ModelCallInput {
        ModelCallInput {
            messages: vec![
                ChatMessage::system("sys"),
                ChatMessage::user_parts(vec![
                    ContentPart::Text { text: "看图".into() },
                    ContentPart::ImageUrl { url: "data:image/png;base64,AAAA".into(), detail: Some("high".into()) },
                ]),
            ],
            stream: true,
        }
    }

    #[test]
    fn endpoint_handles_v1_suffix() {
        assert_eq!(resolve_endpoint("https://x/v1/", ApiProtocol::OpenAiChat), "https://x/v1/chat/completions");
        assert_eq!(resolve_endpoint("https://x", ApiProtocol::Anthropic), "https://x/v1/messages");
        assert_eq!(resolve_endpoint("https://x", ApiProtocol::Custom), "https://x/v1/chat/completions");
    }

    #[test]
    fn chat_body_thinking_flags() {
        let (url, headers, body) = build_request(&cfg(ApiProtocol::OpenAiChat, false), &input()).unwrap();
        assert_eq!(url, "https://api.example.com/v1/chat/completions");
        assert_eq!(headers.get("authorization").unwrap(), "Bearer k");
        assert_eq!(headers.get("x-custom").unwrap(), "1");
        assert_eq!(body["enable_thinking"], false);
        assert_eq!(body["thinking"]["type"], "disabled");
        assert_eq!(body["messages"][1]["content"][1]["image_url"]["detail"], "high");
        let (_, _, on) = build_request(&cfg(ApiProtocol::OpenAiChat, true), &input()).unwrap();
        assert_eq!(on["reasoning_effort"], "high");
        assert!(on.get("enable_thinking").is_none());
    }

    #[test]
    fn responses_and_anthropic_bodies() {
        let (_, _, r) = build_request(&cfg(ApiProtocol::OpenAiResponse, false), &input()).unwrap();
        assert_eq!(r["reasoning"]["effort"], "none");
        assert_eq!(r["input"][1]["content"][1]["type"], "input_image");
        let (_, h, a) = build_request(&cfg(ApiProtocol::Anthropic, true), &input()).unwrap();
        assert_eq!(h.get("anthropic-version").unwrap(), "2023-06-01");
        assert_eq!(h.get("x-api-key").unwrap(), "k");
        assert_eq!(a["system"], "sys");
        assert_eq!(a["messages"][0]["content"][1]["source"]["media_type"], "image/png");
        assert_eq!(a["thinking"]["type"], "enabled");
    }

    #[test]
    fn sse_framer_and_normalizers() {
        let mut f = SseFramer::default();
        let mut evs = f.push("data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"reasoning_content\":\"想\"}}]}\n\nda");
        evs.extend(f.push("ta: [DONE]\n\n"));
        assert_eq!(evs.len(), 2);
        let c = normalize_openai_chunk(&evs[0]).unwrap();
        assert_eq!(c.content, "你");
        assert_eq!(normalize_openai_chunk(&evs[1]).unwrap().reasoning_content, "想");
        assert!(normalize_openai_chunk(&json!({"choices":[{"delta":{}}]})).is_none());

        let r = normalize_responses_event(&json!({"type":"response.output_text.delta","delta":"hi"})).unwrap();
        assert_eq!(r.content, "hi");
        let a = normalize_anthropic_event(&json!({"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"t"}})).unwrap();
        assert_eq!(a.reasoning_content, "t");
        let np = normalize_anthropic_payload(&json!({"content":[{"type":"text","text":"ok"}]})).unwrap();
        assert_eq!(np.content, "ok");
        let rp = normalize_responses_payload(&json!({"output":[{"content":[{"type":"output_text","text":"x"}]}]})).unwrap();
        assert_eq!(rp.content, "x");
    }

    #[test]
    fn http_error_message_formats() {
        assert_eq!(http_error_message(401, r#"{"error":{"message":"bad key"}}"#), "HTTP 401: bad key");
        assert_eq!(http_error_message(500, ""), "HTTP 500");
    }
}
