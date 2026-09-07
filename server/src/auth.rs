//! 鉴权：管理员令牌（管理接口）与查询令牌（公网 /query）。

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use subtle::ConstantTimeEq;

use crate::config::AppSettings;
use crate::state::AppState;

/// 常量时间比较
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.as_bytes().ct_eq(b.as_bytes()).into()
}

/// 从 Authorization 头提取 Bearer 令牌（也接受裸值）
pub fn extract_bearer(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    if raw.is_empty() {
        return None;
    }
    let token = if raw.len() >= 7 && raw[..7].eq_ignore_ascii_case("bearer ") {
        raw[7..].trim()
    } else {
        raw
    };
    (!token.is_empty()).then(|| token.to_string())
}

/// 从查询串中提取 token 参数
pub fn extract_query_token(query: Option<&str>) -> Option<String> {
    let query = query?;
    for pair in query.split('&') {
        let mut it = pair.splitn(2, '=');
        if it.next() == Some("token") {
            let v = it.next().unwrap_or("");
            let decoded = urlencoding::decode(v).map(|c| c.into_owned()).unwrap_or_else(|_| v.to_string());
            let t = decoded.trim().to_string();
            return (!t.is_empty()).then_some(t);
        }
    }
    None
}

pub fn is_admin_token(settings: &AppSettings, token: &str) -> bool {
    let admin = settings.admin_token.trim();
    !admin.is_empty() && constant_time_eq(admin, token.trim())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryAuth {
    Admin,
    User(String),
    Anonymous,
}

/// 校验查询令牌：None 表示拒绝
pub fn check_query_token(settings: &AppSettings, token: Option<&str>) -> Option<QueryAuth> {
    if let Some(token) = token.map(str::trim).filter(|t| !t.is_empty()) {
        if is_admin_token(settings, token) {
            return Some(QueryAuth::Admin);
        }
        if settings.multi_user.enabled {
            if let Some(user) = settings
                .multi_user
                .users
                .iter()
                .find(|u| !u.token.trim().is_empty() && constant_time_eq(u.token.trim(), token))
            {
                return Some(QueryAuth::User(user.name.clone()));
            }
        }
        // 提供了令牌但无效：即便允许匿名也拒绝，避免误配
        return None;
    }
    if settings.public_query_require_token {
        None
    } else {
        Some(QueryAuth::Anonymous)
    }
}

/// 从请求中取出候选令牌：?token= → X-Token → Authorization
pub fn token_from_request(headers: &HeaderMap, query: Option<&str>) -> Option<String> {
    extract_query_token(query)
        .or_else(|| {
            headers
                .get("x-token")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| extract_bearer(headers))
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"success": false, "message": "未授权"})),
    )
        .into_response()
}

/// 管理接口中间件：Bearer 或 ?token= 必须等于管理员令牌
pub async fn require_admin(State(state): State<AppState>, req: Request<Body>, next: Next) -> Response {
    let token = extract_bearer(req.headers()).or_else(|| extract_query_token(req.uri().query()));
    let ok = match token {
        Some(t) => is_admin_token(&state.settings.read(), &t),
        None => false,
    };
    if !ok {
        return unauthorized();
    }
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MultiUserConfig, UserConfig};

    fn settings() -> AppSettings {
        let mut s = AppSettings::default();
        s.admin_token = "admin-secret".into();
        s.multi_user = MultiUserConfig {
            enabled: true,
            users: vec![UserConfig {
                id: "u1".into(),
                name: "小明".into(),
                token: "user-token".into(),
                created_at: String::new(),
            }],
        };
        s
    }

    #[test]
    fn query_token_rules() {
        let s = settings();
        assert_eq!(check_query_token(&s, Some("admin-secret")), Some(QueryAuth::Admin));
        assert_eq!(check_query_token(&s, Some("user-token")), Some(QueryAuth::User("小明".into())));
        assert_eq!(check_query_token(&s, Some("bad")), None);
        assert_eq!(check_query_token(&s, None), None);
        let mut open = settings();
        open.public_query_require_token = false;
        assert_eq!(check_query_token(&open, None), Some(QueryAuth::Anonymous));
        assert_eq!(check_query_token(&open, Some("bad")), None);
        let mut disabled = settings();
        disabled.multi_user.enabled = false;
        assert_eq!(check_query_token(&disabled, Some("user-token")), None);
    }

    #[test]
    fn extracts_tokens() {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, "Bearer abc".parse().unwrap());
        assert_eq!(extract_bearer(&h), Some("abc".into()));
        assert_eq!(extract_query_token(Some("title=x&token=t%201")), Some("t 1".into()));
        assert_eq!(token_from_request(&h, Some("a=b")), Some("abc".into()));
    }
}
