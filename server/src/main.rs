//! zerror-server 入口

mod ai;
mod answer;
mod auth;
mod config;
mod db;
mod logger;
mod matching;
mod prompt;
mod routes;
mod state;
mod web;

use std::sync::atomic::AtomicI64;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;
use tracing_subscriber::EnvFilter;

use crate::config::{AppSettings, ModelSettings, RuntimeConfig};
use crate::state::AppStateInner;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("ZERROR_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let runtime = RuntimeConfig::from_env()?;
    std::fs::create_dir_all(&runtime.data_dir)?;
    std::fs::create_dir_all(runtime.image_cache_dir())?;

    // 数据库
    let db = db::Db::open(&runtime.db_path())?;

    // 设置：首次启动生成管理员令牌
    let mut settings: AppSettings = config::read_json_file(&runtime.settings_path());
    if settings.admin_token.trim().is_empty() {
        let token = runtime
            .admin_token_env
            .clone()
            .unwrap_or_else(config::generate_token);
        settings.admin_token = token.clone();
        config::write_json_file(&runtime.settings_path(), &settings)?;
        tracing::warn!("已生成管理员令牌（请妥善保存，可在 settings.json 修改）: {}", token);
    }
    let model_settings: ModelSettings = config::read_json_file(&runtime.model_config_path());

    let http = reqwest::Client::builder()
        .user_agent(format!("zerror-server/{VERSION}"))
        .connect_timeout(std::time::Duration::from_secs(20))
        .build()?;

    let logger = logger::RequestLogger::new(db.clone(), 100);

    let state = Arc::new(AppStateInner {
        runtime: runtime.clone(),
        db,
        settings: RwLock::new(settings),
        model_settings: RwLock::new(model_settings),
        logger,
        http,
        started_at: Instant::now(),
        last_ocs_contact_at: AtomicI64::new(0),
    });

    let app = routes::build_router(state.clone());
    let listener = tokio::net::TcpListener::bind(runtime.bind).await?;
    tracing::info!(
        "ZError Server v{} 已启动: http://{}  数据目录: {}",
        VERSION,
        runtime.bind,
        runtime.data_dir.display()
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut sig) = signal(SignalKind::terminate()) {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("收到退出信号，正在关闭…");
}
