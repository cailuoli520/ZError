//! 嵌入 web/dist 的静态资源（SPA fallback）。

use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../web/dist/"]
struct Assets;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    serve_path(path)
}

/// GET /：后台首页
pub async fn serve_index() -> Response {
    serve_path("index.html")
}

fn serve_path(path: &str) -> Response {
    match Assets::get(path) {
        Some(file) => file_response(path, file.data.into_owned(), false),
        None => match Assets::get("index.html") {
            Some(index) => file_response("index.html", index.data.into_owned(), true),
            None => (StatusCode::NOT_FOUND, "web/dist 未构建").into_response(),
        },
    }
}

fn file_response(path: &str, data: Vec<u8>, no_cache: bool) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let cache = if no_cache || path == "index.html" {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap())
        .header(header::CACHE_CONTROL, cache)
        .body(Body::from(data))
        .unwrap()
}
