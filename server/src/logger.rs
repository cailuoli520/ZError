//! 请求日志：内存环形缓冲 + SSE 广播 + 持久化。移植自上游 logger.rs，
//! 去掉了浏览器回调等待机制（pending_responses / wait_for_model_response）。

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::db::{Db, PersistedRequestLog};

/// 持久化上限
const PERSIST_MAX_LOGS: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestLog {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub method: String,
    pub path: String,
    pub status: Option<u16>,
    pub response_time: Option<u64>,
    pub request_body: Option<String>,
    pub response_body: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    /// "started" 或 "completed"
    pub stage: String,
}

impl From<PersistedRequestLog> for RequestLog {
    fn from(p: PersistedRequestLog) -> Self {
        RequestLog {
            id: p.id,
            timestamp: DateTime::parse_from_rfc3339(&p.timestamp)
                .map(|v| v.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            method: p.method,
            path: p.path,
            status: p.status,
            response_time: p.response_time,
            request_body: p.request_body,
            response_body: p.response_body,
            headers: p.headers,
            ip: p.ip,
            user_agent: p.user_agent,
            stage: p.stage,
        }
    }
}

impl From<&RequestLog> for PersistedRequestLog {
    fn from(l: &RequestLog) -> Self {
        PersistedRequestLog {
            id: l.id.clone(),
            timestamp: l.timestamp.to_rfc3339(),
            method: l.method.clone(),
            path: l.path.clone(),
            status: l.status,
            response_time: l.response_time,
            request_body: l.request_body.clone(),
            response_body: l.response_body.clone(),
            headers: l.headers.clone(),
            ip: l.ip.clone(),
            user_agent: l.user_agent.clone(),
            stage: l.stage.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCallRequest {
    pub request_id: String,
    pub query: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCallProgress {
    pub request_id: String,
    pub content: String,
    /// 模型标识（多模型并发时区分来源；总结模型为 summary:<id>）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCallResponse {
    pub request_id: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    pub is_success: bool,
    /// 各模型的结果明细（多模型时）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_model: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcsHead {
    pub timestamp: DateTime<Utc>,
}

/// 统一的 SSE 事件（tag 为 type，字段扁平）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SSEEvent {
    #[serde(rename = "request_log")]
    RequestLog(RequestLog),
    #[serde(rename = "model_call_request")]
    ModelCallRequest(ModelCallRequest),
    #[serde(rename = "model_call_progress")]
    ModelCallProgress(ModelCallProgress),
    #[serde(rename = "model_call_response")]
    ModelCallResponse(ModelCallResponse),
    #[serde(rename = "ocs_head")]
    OcsHead(OcsHead),
}

impl SSEEvent {
    /// SSE 事件名（request_log 对外沿用上游的 "log"）
    pub fn event_name(&self) -> &'static str {
        match self {
            SSEEvent::RequestLog(_) => "log",
            SSEEvent::ModelCallRequest(_) => "model_call_request",
            SSEEvent::ModelCallProgress(_) => "model_call_progress",
            SSEEvent::ModelCallResponse(_) => "model_call_response",
            SSEEvent::OcsHead(_) => "ocs_head",
        }
    }
}

#[derive(Clone)]
pub struct RequestLogger {
    db: Db,
    logs: Arc<Mutex<VecDeque<RequestLog>>>,
    max_logs: usize,
    broadcaster: broadcast::Sender<SSEEvent>,
}

impl RequestLogger {
    pub fn new(db: Db, max_memory_logs: usize) -> Self {
        let (broadcaster, _) = broadcast::channel(1000);
        Self {
            db,
            logs: Arc::new(Mutex::new(VecDeque::new())),
            max_logs: max_memory_logs.max(1),
            broadcaster,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SSEEvent> {
        self.broadcaster.subscribe()
    }

    fn broadcast(&self, event: SSEEvent) {
        // 无订阅者时 send 返回 Err，属正常情况
        let _ = self.broadcaster.send(event);
    }

    fn push_memory(&self, log: RequestLog) {
        let mut logs = self.logs.lock();
        logs.push_back(log);
        while logs.len() > self.max_logs {
            logs.pop_front();
        }
    }

    /// 只对 /query 的 started 阶段计数
    fn persist_query_count(&self, log: &RequestLog) {
        if log.path == "/query" && (log.method == "GET" || log.method == "POST") && log.stage == "started" {
            let db = self.db.clone();
            tokio::spawn(async move {
                if let Err(err) = db.increment_daily_request_count().await {
                    tracing::warn!("更新每日请求计数失败: {}", err);
                }
            });
        }
    }

    /// 记录请求开始，返回请求 id
    #[allow(clippy::too_many_arguments)]
    pub fn log_request_start(
        &self,
        method: &str,
        path: &str,
        ip: Option<String>,
        user_agent: Option<String>,
        headers: Option<HashMap<String, String>>,
        request_body: Option<String>,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let log = RequestLog {
            id: id.clone(),
            timestamp: Utc::now(),
            method: method.to_string(),
            path: path.to_string(),
            status: None,
            response_time: None,
            request_body,
            response_body: None,
            headers,
            ip,
            user_agent,
            stage: "started".to_string(),
        };
        self.push_memory(log.clone());
        self.persist_query_count(&log);
        self.broadcast(SSEEvent::RequestLog(log));
        id
    }

    /// 记录请求完成：就地更新 started 记录并持久化
    pub fn log_request_complete(&self, id: &str, status: u16, response_time_ms: u64, response_body: Option<String>) {
        let log = {
            let mut logs = self.logs.lock();
            if let Some(existing) = logs.iter_mut().rev().find(|item| item.id == id) {
                existing.status = Some(status);
                existing.response_time = Some(response_time_ms);
                existing.response_body = response_body;
                existing.stage = "completed".to_string();
                existing.clone()
            } else {
                let created = RequestLog {
                    id: id.to_string(),
                    timestamp: Utc::now(),
                    method: String::new(),
                    path: String::new(),
                    status: Some(status),
                    response_time: Some(response_time_ms),
                    request_body: None,
                    response_body,
                    headers: None,
                    ip: None,
                    user_agent: None,
                    stage: "completed".to_string(),
                };
                logs.push_back(created.clone());
                while logs.len() > self.max_logs {
                    logs.pop_front();
                }
                created
            }
        };
        let persisted: PersistedRequestLog = (&log).into();
        let db = self.db.clone();
        tokio::spawn(async move {
            if let Err(err) = db.insert_request_log(persisted, PERSIST_MAX_LOGS).await {
                tracing::warn!("写入请求日志失败: {}", err);
            }
        });
        self.broadcast(SSEEvent::RequestLog(log));
    }

    pub fn send_model_call_request(&self, request_id: &str, query: &str) {
        self.broadcast(SSEEvent::ModelCallRequest(ModelCallRequest {
            request_id: request_id.to_string(),
            query: query.to_string(),
            timestamp: Utc::now(),
        }));
    }

    pub fn send_model_call_progress(
        &self,
        request_id: &str,
        content: &str,
        model_id: Option<&str>,
        model_name: Option<&str>,
        reasoning: Option<&str>,
    ) {
        self.broadcast(SSEEvent::ModelCallProgress(ModelCallProgress {
            request_id: request_id.to_string(),
            content: content.to_string(),
            model_id: model_id.map(|s| s.to_string()),
            model_name: model_name.map(|s| s.to_string()),
            reasoning_content: reasoning.map(|s| s.to_string()),
            timestamp: Utc::now(),
        }));
    }

    pub fn send_model_call_response(
        &self,
        request_id: &str,
        content: &str,
        reasoning_content: Option<String>,
        is_success: bool,
        per_model: Option<serde_json::Value>,
    ) {
        self.broadcast(SSEEvent::ModelCallResponse(ModelCallResponse {
            request_id: request_id.to_string(),
            content: content.to_string(),
            reasoning_content,
            is_success,
            per_model,
            timestamp: Utc::now(),
        }));
    }

    pub fn send_ocs_head(&self) {
        self.broadcast(SSEEvent::OcsHead(OcsHead { timestamp: Utc::now() }));
    }

    /// 最近的内存日志（按时间正序）
    pub fn recent(&self) -> Vec<RequestLog> {
        self.logs.lock().iter().cloned().collect()
    }

    pub fn clear_memory(&self) {
        self.logs.lock().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_event_serializes_flat_with_type() {
        let ev = SSEEvent::ModelCallRequest(ModelCallRequest {
            request_id: "r1".into(),
            query: "q".into(),
            timestamp: Utc::now(),
        });
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["type"], "model_call_request");
        assert_eq!(v["request_id"], "r1");
        assert_eq!(ev.event_name(), "model_call_request");
    }
}
