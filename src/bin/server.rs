//! `trail-server`: serves the API and the static client.

use std::net::SocketAddr;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use trail_route::server::{app, ServerConfig};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let addr: SocketAddr = std::env::var("BIND")
        .unwrap_or_else(|_| "0.0.0.0:8080".into())
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let static_dir = std::env::var_os("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("web/dist"));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, static_dir = %static_dir.display(), "trail-server listening");
    axum::serve(listener, app(ServerConfig { static_dir }))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}
