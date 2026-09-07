//! Router 组装。

pub mod admin;
pub mod public;
pub mod sse;

use axum::http::{header, HeaderValue, Method};
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::auth::require_admin;
use crate::state::AppState;
use crate::web;

pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
            Method::HEAD,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::HeaderName::from_static("x-token")])
        .max_age(std::time::Duration::from_secs(3600));

    let admin = Router::new()
        // 设置
        .route("/settings", get(admin::get_settings).put(admin::put_settings))
        .route("/model-settings", get(admin::get_model_settings).put(admin::put_model_settings))
        .route("/remote-catalog", get(admin::remote_catalog))
        .route("/models/test", post(admin::test_model))
        .route("/platforms/probe-models", post(admin::probe_models))
        // 文件夹
        .route("/folders", get(admin::folders).post(admin::add_folder))
        .route("/folders/stats", get(admin::folder_stats))
        .route("/folders/:id", axum::routing::patch(admin::patch_folder).delete(admin::delete_folder))
        .route("/folders/:id/path", get(admin::folder_path))
        .route("/folders/:id/count", get(admin::folder_count))
        .route("/folders/:id/clear", post(admin::clear_folder))
        // 题目
        .route("/questions", get(admin::list_questions).post(admin::add_question))
        .route("/questions/search", get(admin::search_questions))
        .route("/questions/pending-count", get(admin::pending_count))
        .route("/questions/bulk", post(admin::add_questions_bulk))
        .route("/questions/batch-delete", post(admin::batch_delete))
        .route("/questions/export", get(admin::export_questions))
        .route("/questions/:id", axum::routing::patch(admin::update_question).delete(admin::delete_question))
        .route("/questions/:id/move", post(admin::move_question))
        .route("/questions/:id/copy", post(admin::copy_question))
        .route("/questions/:id/pending", post(admin::set_pending))
        // 日志 / 统计
        .route("/logs", get(admin::list_logs).delete(admin::clear_logs))
        .route("/logs/recent", get(admin::recent_logs))
        .route("/logs/stream", get(sse::logs_stream))
        .route("/stats/daily", get(admin::daily_stats))
        // 其它
        .route("/image", get(admin::image_proxy))
        .route("/segment", get(admin::segment))
        .route("/version", get(admin::version))
        .layer(middleware::from_fn_with_state(state.clone(), require_admin));

    Router::new()
        .route("/", axum::routing::head(public::head_root).get(web::serve_index))
        .route("/query", get(public::query_get).post(public::query_post))
        .route("/api/status", get(public::status))
        .route("/api/time", get(public::time))
        .route("/api/echo", post(public::echo))
        .route("/api/questions/:id/pending-correction", post(public::mark_pending_correction))
        .route("/api/login", post(admin::login))
        .nest("/api/admin", admin)
        .fallback(web::serve)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .with_state(state)
}
