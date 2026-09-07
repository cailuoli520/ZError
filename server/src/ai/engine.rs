//! AI 编排：多模型并发答题、重试/超时、总结与多数投票、同题判重、URL 图片题。
//! 逻辑移植自上游 Home.vue（callModelAPI / callModelWithStreaming / withModelRetry /
//! handleSameQuestionCheckRequest / analyzeUrlQuestion）。

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::Serialize;
use tokio::task::JoinSet;

use super::image::{extract_vision_image_size_error, fetch_image_as_data_url, find_image_url_matches, preprocess_for_vision};
use super::protocol::call_model;
use super::{ChatMessage, ContentPart, MessageContent, ModelCallConfig, ModelCallError, ModelCallInput, ModelCallOutput};
use crate::answer::{get_most_frequent_answer, strip_markdown_code_block};
use crate::config::ResolvedModel;
use crate::prompt;
use crate::state::AppState;

const PROGRESS_THROTTLE: Duration = Duration::from_millis(800);
const DEFAULT_VISION_IMAGE_MIN_SIZE: u32 = 32;

#[derive(Debug, Clone, Serialize)]
pub struct ModelResult {
    pub model_id: String,
    pub model_name: String,
    pub platform_name: String,
    pub content: String,
    pub reasoning_content: String,
    pub error: Option<String>,
    pub elapsed_ms: u64,
    pub is_summary: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnswerOutcome {
    pub content: String,
    pub reasoning_content: String,
    pub per_model: Vec<ModelResult>,
    pub used_summary: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("错误: 未选择模型")]
    NoModelSelected,
    #[error("错误: 请先在模型选择中配置视觉模型")]
    NoVisionModel,
    #[error("{0}")]
    AllModelsFailed(String),
    #[error("错误: {0}")]
    Other(String),
}

impl EngineError {
    pub fn http_status(&self) -> u16 {
        match self {
            EngineError::NoModelSelected | EngineError::NoVisionModel => 400,
            EngineError::AllModelsFailed(msg) => {
                if crate::answer::is_timeout_like_model_failure(msg) {
                    408
                } else {
                    500
                }
            }
            EngineError::Other(_) => 500,
        }
    }
}

pub struct EngineCtx<'a> {
    pub state: &'a AppState,
    pub request_id: &'a str,
}

fn format_model_error(err: &ModelCallError, timeout_secs: u64) -> String {
    match err {
        ModelCallError::Timeout => format!("模型响应超时（{timeout_secs} 秒）或连接被中断"),
        other => other.to_string(),
    }
}

fn model_label(r: &ResolvedModel) -> String {
    r.model.label().to_string()
}

fn platform_label(r: &ResolvedModel) -> String {
    if !r.platform.display_name.trim().is_empty() {
        r.platform.display_name.clone()
    } else if !r.platform.name.trim().is_empty() {
        r.platform.name.clone()
    } else {
        "未知平台".into()
    }
}

/// 单模型一次调用（流式聚合 + 节流进度广播）
async fn call_once(
    state: &AppState,
    request_id: &str,
    model: &ResolvedModel,
    progress_key: &str,
    input: ModelCallInput,
) -> Result<ModelCallOutput, ModelCallError> {
    let settings = state.settings();
    let timeout = Duration::from_secs(settings.model_timeout_secs());
    let cfg = ModelCallConfig::from(model);
    let logger = state.logger.clone();
    let request_id = request_id.to_string();
    let key = progress_key.to_string();
    let name = model_label(model);
    let last_sent = Arc::new(Mutex::new(Instant::now() - PROGRESS_THROTTLE));
    let on_progress: Box<dyn FnMut(&str, &str) + Send> = Box::new(move |content: &str, reasoning: &str| {
        let mut last = last_sent.lock();
        if last.elapsed() >= PROGRESS_THROTTLE {
            *last = Instant::now();
            logger.send_model_call_progress(
                &request_id,
                &strip_markdown_code_block(content),
                Some(&key),
                Some(&name),
                (!reasoning.is_empty()).then_some(reasoning),
            );
        }
    });
    call_model(&state.http, &cfg, &input, timeout, Some(on_progress)).await
}

/// 带重试的调用（withModelRetry）：enabled=false 时只尝试一次
async fn call_with_retry(
    state: &AppState,
    request_id: &str,
    model: &ResolvedModel,
    progress_key: &str,
    input: ModelCallInput,
    retry_enabled: bool,
) -> Result<ModelCallOutput, ModelCallError> {
    let settings = state.settings();
    let max_attempts = if retry_enabled { settings.model_retry_count() + 1 } else { 1 };
    let mut last_err: Option<ModelCallError> = None;
    for attempt in 1..=max_attempts {
        match call_once(state, request_id, model, progress_key, input.clone()).await {
            Ok(out) => return Ok(out),
            Err(err) => {
                let is_config = matches!(err, ModelCallError::Config(_));
                tracing::warn!("[模型重试 {}/{}] {}: {}", attempt, max_attempts, model_label(model), err);
                last_err = Some(err);
                if is_config || attempt >= max_attempts {
                    break;
                }
                if attempt > 1 {
                    state.logger.send_model_call_progress(
                        request_id,
                        &format!("第 {} 次失败，准备重试（{}/{}）…", attempt, attempt + 1, max_attempts),
                        Some(progress_key),
                        Some(&model_label(model)),
                        None,
                    );
                }
                tokio::time::sleep(Duration::from_millis((400 * attempt as u64).min(2000))).await;
            }
        }
    }
    Err(last_err.unwrap_or(ModelCallError::Transport("未知错误".into())))
}

fn result_from(model: &ResolvedModel, key: &str, is_summary: bool, res: Result<ModelCallOutput, ModelCallError>, timeout_secs: u64) -> ModelResult {
    match res {
        Ok(out) => ModelResult {
            model_id: key.to_string(),
            model_name: model_label(model),
            platform_name: platform_label(model),
            content: strip_markdown_code_block(&out.content),
            reasoning_content: out.reasoning_content,
            error: None,
            elapsed_ms: out.elapsed_ms,
            is_summary,
        },
        Err(err) => ModelResult {
            model_id: key.to_string(),
            model_name: model_label(model),
            platform_name: platform_label(model),
            content: format!("错误: {}", format_model_error(&err, timeout_secs)),
            reasoning_content: String::new(),
            error: Some(format_model_error(&err, timeout_secs)),
            elapsed_ms: 0,
            is_summary,
        },
    }
}

/// 拼装多模型输出：成功内容或真实报错原文
fn format_model_outputs(entries: &[&ModelResult]) -> String {
    let usable: Vec<&&ModelResult> = entries.iter().filter(|e| !e.content.trim().is_empty()).collect();
    match usable.len() {
        0 => String::new(),
        1 => usable[0].content.clone(),
        _ => usable
            .iter()
            .map(|e| format!("[{}]\n{}", e.model_name, e.content))
            .collect::<Vec<_>>()
            .join("\n\n"),
    }
}

fn has_image_hint(query: &str) -> bool {
    static RE: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| regex::Regex::new(r"(?i)https?://\S+\.(png|jpg|jpeg|gif|webp)").expect("re"));
    RE.is_match(query) || query.contains("base64")
}

/// 普通答题：多模型并发 → 总结 / 多数投票
pub async fn answer_question(ctx: &EngineCtx<'_>, query: &str) -> Result<AnswerOutcome, EngineError> {
    let state = ctx.state;
    let request_id = ctx.request_id;
    let ms = state.model_settings();
    let timeout_secs = state.settings().model_timeout_secs();

    let mut selected = ms.selected_text_models();
    let vision = ms.selected_vision_model();
    if has_image_hint(query) {
        if let Some(v) = &vision {
            if !selected.iter().any(|m| m.model.id == v.model.id) {
                selected.push(v.clone());
            }
        }
    }
    if selected.is_empty() {
        return Err(EngineError::NoModelSelected);
    }
    state.logger.send_model_call_request(request_id, query);

    let vision_id = vision.as_ref().map(|v| v.model.id.clone());
    let single = selected.len() == 1;
    let messages = prompt::build_answer_messages(query);

    // 阶段 1：基础模型并发
    let mut set: JoinSet<(usize, ModelResult)> = JoinSet::new();
    for (idx, model) in selected.iter().cloned().enumerate() {
        let state = state.clone();
        let request_id = request_id.to_string();
        let input = ModelCallInput { messages: messages.clone(), stream: true };
        let allow_retry = single || vision_id.as_deref() == Some(model.model.id.as_str());
        set.spawn(async move {
            let res = call_with_retry(&state, &request_id, &model, &model.model.id, input, allow_retry).await;
            (idx, result_from(&model, &model.model.id, false, res, timeout_secs))
        });
    }
    let mut base: Vec<(usize, ModelResult)> = Vec::new();
    while let Some(joined) = set.join_next().await {
        if let Ok(r) = joined {
            base.push(r);
        }
    }
    base.sort_by_key(|(i, _)| *i);
    let mut per_model: Vec<ModelResult> = base.into_iter().map(|(_, r)| r).collect();

    let successful: Vec<&ModelResult> = per_model.iter().filter(|r| r.error.is_none()).collect();
    let successful_contents: Vec<String> = successful.iter().map(|r| r.content.clone()).collect();
    let base_combined = if successful.len() == 1 {
        successful[0].content.clone()
    } else {
        successful
            .iter()
            .map(|r| format!("[{}]\n{}", r.model_name, r.content))
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    let majority = get_most_frequent_answer(&successful_contents);
    let single_success_reasoning = if successful.len() == 1 { successful[0].reasoning_content.clone() } else { String::new() };

    // 阶段 2：总结
    let summary_models = ms.selected_summary_models();
    if !summary_models.is_empty() && !successful.is_empty() {
        let summary_query = prompt::build_summary_prompt(query, &base_combined);
        let mut set: JoinSet<(usize, ModelResult)> = JoinSet::new();
        for (idx, model) in summary_models.iter().cloned().enumerate() {
            let state = state.clone();
            let request_id = request_id.to_string();
            let key = format!("summary:{}", model.model.id);
            let input = ModelCallInput { messages: vec![ChatMessage::user(summary_query.clone())], stream: true };
            set.spawn(async move {
                let res = call_with_retry(&state, &request_id, &model, &key, input, true).await;
                let mut r = result_from(&model, &key, true, res, timeout_secs);
                r.model_name = format!("总结: {}", r.model_name);
                (idx, r)
            });
        }
        let mut summaries: Vec<(usize, ModelResult)> = Vec::new();
        while let Some(joined) = set.join_next().await {
            if let Ok(r) = joined {
                summaries.push(r);
            }
        }
        summaries.sort_by_key(|(i, _)| *i);
        let summaries: Vec<ModelResult> = summaries.into_iter().map(|(_, r)| r).collect();
        let ok_summaries: Vec<&ModelResult> = summaries.iter().filter(|r| r.error.is_none()).collect();

        let (content, used_summary) = if !ok_summaries.is_empty() {
            let labeled: Vec<ModelResult> = ok_summaries
                .iter()
                .map(|r| ModelResult { model_name: format!("{} 总结", r.model_name.trim_start_matches("总结: ")), ..(*r).clone() })
                .collect();
            (format_model_outputs(&labeled.iter().collect::<Vec<_>>()), true)
        } else {
            let fallback = majority
                .clone()
                .filter(|m| !m.is_empty())
                .or_else(|| Some(format_model_outputs(&summaries.iter().collect::<Vec<_>>())).filter(|s| !s.is_empty()))
                .or_else(|| Some(format_model_outputs(&per_model.iter().collect::<Vec<_>>())).filter(|s| !s.is_empty()))
                .unwrap_or_else(|| "错误: 未获得任何模型响应".into());
            (fallback, false)
        };
        let reasoning = if ok_summaries.len() == 1 {
            ok_summaries[0].reasoning_content.clone()
        } else if ok_summaries.is_empty() {
            single_success_reasoning
        } else {
            String::new()
        };
        per_model.extend(summaries);
        let success = used_summary || majority.as_deref().map(|m| !m.is_empty()).unwrap_or(false);
        state.logger.send_model_call_response(
            request_id,
            &content,
            (!reasoning.is_empty()).then(|| reasoning.clone()),
            success,
            serde_json::to_value(&per_model).ok(),
        );
        if !success {
            return Err(EngineError::AllModelsFailed(content));
        }
        return Ok(AnswerOutcome { content, reasoning_content: reasoning, per_model, used_summary });
    }

    // 无总结模型：多模型时取多数答案；否则返回真实报错
    let (content, success) = if !successful.is_empty() {
        let c = if successful.len() > 1 {
            majority.clone().filter(|m| !m.is_empty()).unwrap_or_else(|| format_model_outputs(&successful))
        } else {
            successful[0].content.clone()
        };
        (c, true)
    } else {
        let all = per_model.iter().collect::<Vec<_>>();
        let c = format_model_outputs(&all);
        (if c.is_empty() { "错误: 未获得任何模型响应".to_string() } else { c }, false)
    };
    state.logger.send_model_call_response(
        request_id,
        &content,
        (!single_success_reasoning.is_empty()).then(|| single_success_reasoning.clone()),
        success,
        serde_json::to_value(&per_model).ok(),
    );
    if !success {
        return Err(EngineError::AllModelsFailed(content));
    }
    Ok(AnswerOutcome { content, reasoning_content: single_success_reasoning, per_model, used_summary: false })
}

/// 同题判重：使用第一个可用文本模型，返回模型原始输出
pub async fn check_same_question(ctx: &EngineCtx<'_>, check_prompt: &str) -> Result<String, EngineError> {
    let state = ctx.state;
    let ms = state.model_settings();
    let model = ms.selected_text_models().into_iter().next().ok_or(EngineError::NoModelSelected)?;
    state.logger.send_model_call_request(ctx.request_id, check_prompt);
    let input = ModelCallInput { messages: vec![ChatMessage::user(check_prompt.to_string())], stream: true };
    let timeout_secs = state.settings().model_timeout_secs();
    let res = call_with_retry(state, ctx.request_id, &model, &model.model.id, input, false).await;
    let r = result_from(&model, &model.model.id, false, res, timeout_secs);
    let ok = r.error.is_none();
    state.logger.send_model_call_response(
        ctx.request_id,
        &r.content,
        (!r.reasoning_content.is_empty()).then(|| r.reasoning_content.clone()),
        ok,
        serde_json::to_value(vec![&r]).ok(),
    );
    if ok {
        Ok(r.content)
    } else {
        Err(EngineError::AllModelsFailed(r.content))
    }
}

/// 把含图片 URL 的文本拆成多模态片段（图片先抓取为 data URL 并预处理）
async fn build_multimodal_content(state: &AppState, text: &str, min_side: u32) -> Result<Vec<ContentPart>, String> {
    let matches = find_image_url_matches(text);
    let cache_dir = state.runtime.image_cache_dir();
    let mut parts: Vec<ContentPart> = Vec::new();
    let mut failed: Vec<String> = Vec::new();
    let mut last = 0usize;
    for m in matches {
        if m.start > last {
            parts.push(ContentPart::Text { text: text[last..m.start].to_string() });
        }
        match fetch_image_as_data_url(&state.http, &cache_dir, &m.url).await {
            Ok(data_url) => {
                let processed = preprocess_for_vision(&data_url, min_side);
                parts.push(ContentPart::ImageUrl { url: processed, detail: Some("high".into()) });
            }
            Err(err) => {
                tracing::warn!("图片下载失败 {}: {}", m.url, err);
                failed.push(m.url.clone());
            }
        }
        last = m.end.max(last);
    }
    if last < text.len() {
        parts.push(ContentPart::Text { text: text[last..].to_string() });
    }
    if !failed.is_empty() {
        return Err(format!("以下图片本地下载失败，无法转为 base64：{}", failed.join("，")));
    }
    Ok(parts)
}

fn upscale_parts(parts: &[ContentPart], min_side: u32) -> (Vec<ContentPart>, bool) {
    let mut changed = false;
    let out = parts
        .iter()
        .map(|p| match p {
            ContentPart::ImageUrl { url, detail } if url.starts_with("data:image/") => {
                let resized = preprocess_for_vision(url, min_side);
                if resized != *url {
                    changed = true;
                }
                ContentPart::ImageUrl { url: resized, detail: detail.clone() }
            }
            other => other.clone(),
        })
        .collect();
    (out, changed)
}

/// URL 图片题：视觉模型分析，返回 {"answer": ...} JSON 或原文
pub async fn answer_url_question(
    ctx: &EngineCtx<'_>,
    title: &str,
    options: Option<&str>,
    question_type: Option<&str>,
) -> Result<AnswerOutcome, EngineError> {
    let state = ctx.state;
    let request_id = ctx.request_id;
    let ms = state.model_settings();
    let vision = ms.selected_vision_model().ok_or(EngineError::NoVisionModel)?;
    let timeout_secs = state.settings().model_timeout_secs();

    let mode = prompt::classify_url_question_mode(question_type, options);
    let option_map = prompt::build_url_option_map_for_mode(mode, options);
    let full_text = prompt::build_url_question_text(title, options, question_type, mode, &option_map);
    state.logger.send_model_call_request(request_id, &full_text);

    let parts = build_multimodal_content(state, &full_text, DEFAULT_VISION_IMAGE_MIN_SIZE)
        .await
        .map_err(EngineError::Other)?;
    let input = ModelCallInput { messages: vec![ChatMessage { role: "user".into(), content: MessageContent::Parts(parts.clone()) }], stream: true };

    // 视觉模型按设置重试；遇到尺寸错误时放大后再试一次
    let mut res = call_with_retry(state, request_id, &vision, &vision.model.id, input, true).await;
    if let Err(err) = &res {
        if let Some(min_side) = extract_vision_image_size_error(&err.to_string()) {
            let (upscaled, changed) = upscale_parts(&parts, min_side);
            if changed {
                tracing::warn!("视觉模型图片尺寸不足，已自动放大到至少 {}px 后重试一次", min_side);
                let retry_input = ModelCallInput { messages: vec![ChatMessage { role: "user".into(), content: MessageContent::Parts(upscaled) }], stream: true };
                res = call_with_retry(state, request_id, &vision, &vision.model.id, retry_input, false).await;
            }
        }
    }
    let r = result_from(&vision, &vision.model.id, false, res, timeout_secs);
    if let Some(err) = &r.error {
        state.logger.send_model_call_response(request_id, &r.content, None, false, serde_json::to_value(vec![&r]).ok());
        return Err(EngineError::AllModelsFailed(format!("错误: {err}")));
    }
    if r.content.trim().is_empty() {
        let msg = "错误: 视觉模型返回空内容".to_string();
        state.logger.send_model_call_response(request_id, &msg, None, false, None);
        return Err(EngineError::AllModelsFailed(msg));
    }
    let answer = prompt::resolve_url_answer(&r.content, &option_map, mode);
    let content = if answer.is_empty() {
        r.content.clone()
    } else {
        serde_json::json!({ "answer": answer }).to_string()
    };
    state.logger.send_model_call_response(
        request_id,
        &content,
        (!r.reasoning_content.is_empty()).then(|| r.reasoning_content.clone()),
        true,
        serde_json::to_value(vec![&r]).ok(),
    );
    Ok(AnswerOutcome { content, reasoning_content: r.reasoning_content.clone(), per_model: vec![r], used_summary: false })
}

/// 管理后台模型测试：流式回调 (累计内容, 累计推理)
pub async fn test_model(
    state: &AppState,
    model: &ResolvedModel,
    messages: Vec<ChatMessage>,
    on_progress: Box<dyn FnMut(&str, &str) + Send>,
) -> Result<ModelCallOutput, ModelCallError> {
    let cfg = ModelCallConfig::from(model);
    let timeout = Duration::from_secs(state.settings().model_timeout_secs().max(60));
    call_model(&state.http, &cfg, &ModelCallInput { messages, stream: true }, timeout, Some(on_progress)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mr(name: &str, content: &str, err: Option<&str>) -> ModelResult {
        ModelResult {
            model_id: name.into(),
            model_name: name.into(),
            platform_name: "p".into(),
            content: content.into(),
            reasoning_content: String::new(),
            error: err.map(|s| s.to_string()),
            elapsed_ms: 0,
            is_summary: false,
        }
    }

    #[test]
    fn formats_outputs() {
        let a = mr("A", "x", None);
        let b = mr("B", "y", None);
        assert_eq!(format_model_outputs(&[&a]), "x");
        assert_eq!(format_model_outputs(&[&a, &b]), "[A]\nx\n\n[B]\ny");
        assert_eq!(format_model_outputs(&[]), "");
    }

    #[test]
    fn error_status_mapping() {
        assert_eq!(EngineError::NoModelSelected.http_status(), 400);
        assert_eq!(EngineError::AllModelsFailed("错误: 模型响应超时".into()).http_status(), 408);
        assert_eq!(EngineError::AllModelsFailed("错误: HTTP 500".into()).http_status(), 500);
        assert!(has_image_hint("看图 https://a.com/x.PNG 回答"));
        assert!(!has_image_hint("纯文本"));
    }
}
