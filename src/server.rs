//! axum application: API routes and the static client.
//!
//! `/api/*` is the JSON API. Every other path serves the built client from
//! `static_dir`, falling back to `index.html` so client-side deep links work.

use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use std::path::PathBuf;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

/// Runtime configuration for [`app`].
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Directory holding the built client (`web/dist`).
    pub static_dir: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct Health {
    pub status: &'static str,
    /// OSM date of the loaded graph; `None` until the graph milestone lands.
    pub graph_date: Option<String>,
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        graph_date: None,
    })
}

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

/// Build the full application router.
pub fn app(config: ServerConfig) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .fallback(api_not_found);

    let index = config.static_dir.join("index.html");
    let client = ServeDir::new(&config.static_dir).fallback(ServeFile::new(index));

    Router::new()
        .nest("/api", api)
        .fallback_service(client)
        .layer(TraceLayer::new_for_http())
}
