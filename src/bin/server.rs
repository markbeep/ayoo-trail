//! `trail-calc-server`: serves the API and the static client.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tracing_subscriber::EnvFilter;
use trail_calc::cost::Profile;
use trail_calc::graph::Graph;
use trail_calc::server::{app, ServerConfig};
use trail_calc::view::ViewGrid;

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

    let data = std::env::var_os("DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data"));
    let graph_path = data.join("graph.bin");
    // Optional (D100): without it the view term stays neutral and the overlay is empty.
    let view = match ViewGrid::open(&data.join("view.bin")) {
        Ok(v) => Some(Arc::new(v)),
        Err(e) => {
            tracing::warn!(error = %e, "no view.bin, running without the 360° view");
            None
        }
    };
    let graph = if graph_path.exists() {
        let t = Instant::now();
        let g = Graph::open(&graph_path).map_err(|e| {
            std::io::Error::other(format!("cannot open {}: {e}", graph_path.display()))
        })?;
        g.main_component(Profile::default().max_sac); // label now, not on the first request
        if let Some(v) = &view {
            g.set_view(v); // before the routing records are packed
        }
        if g.edge_count() > 0 {
            g.hot_edge(0); // pack the routing records now too
        }
        tracing::info!(
            nodes = g.node_count(),
            edges = g.edge_count(),
            elapsed = ?t.elapsed(),
            "graph loaded"
        );
        Some(Arc::new(g))
    } else {
        tracing::warn!(path = %graph_path.display(), "no graph found, running without routing");
        None
    };

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, static_dir = %static_dir.display(), "trail-calc-server listening");
    axum::serve(
        listener,
        app(ServerConfig {
            static_dir,
            graph,
            view,
        }),
    )
    .with_graceful_shutdown(async {
        // Ctrl-C locally; SIGTERM is how Docker and Kubernetes stop the container.
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    })
    .await
}
