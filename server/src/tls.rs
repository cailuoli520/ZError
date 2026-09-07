//! HTTPS：加载外部证书管理器提供的 PEM 证书/私钥，并在文件更新后自动热加载。
//! 程序不负责申请与续签证书。

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use axum_server::tls_rustls::RustlsConfig;

/// 证书文件变化检测间隔
const RELOAD_INTERVAL: Duration = Duration::from_secs(60);

fn mtimes(cert: &PathBuf, key: &PathBuf) -> Option<(SystemTime, SystemTime)> {
    let c = std::fs::metadata(cert).and_then(|m| m.modified()).ok()?;
    let k = std::fs::metadata(key).and_then(|m| m.modified()).ok()?;
    Some((c, k))
}

/// 加载证书并启动后台任务：证书或私钥文件 mtime 变化时重新加载（加载失败保持旧证书）
pub async fn load_and_watch(cert: PathBuf, key: PathBuf) -> anyhow::Result<RustlsConfig> {
    // 安装 ring 作为 rustls 加密后端（与 reqwest 的 rustls 共用）
    let _ = rustls::crypto::ring::default_provider().install_default();

    let config = RustlsConfig::from_pem_file(&cert, &key)
        .await
        .map_err(|e| anyhow::anyhow!("加载 TLS 证书失败（{} / {}）: {}", cert.display(), key.display(), e))?;
    tracing::info!("已加载 TLS 证书: {}", cert.display());

    let watcher = config.clone();
    tokio::spawn(async move {
        let mut last = mtimes(&cert, &key);
        loop {
            tokio::time::sleep(RELOAD_INTERVAL).await;
            let now = mtimes(&cert, &key);
            if now.is_some() && now != last {
                match watcher.reload_from_pem_file(&cert, &key).await {
                    Ok(()) => {
                        tracing::info!("检测到证书文件更新，已热加载: {}", cert.display());
                        last = now;
                    }
                    Err(e) => tracing::warn!("证书热加载失败，继续使用旧证书: {}", e),
                }
            }
        }
    });
    Ok(config)
}
