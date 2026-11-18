use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::path::PathBuf;
use tower::ServiceExt;
use trail_route::server::{app, ServerConfig};

fn config(static_dir: PathBuf) -> ServerConfig {
    ServerConfig {
        static_dir,
        graph: None,
    }
}

fn fixture_dist() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("trail-route-dist-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(
        dir.join("index.html"),
        "<!doctype html><title>trail-route</title>",
    )
    .unwrap();
    std::fs::write(dir.join("assets/app.js"), "console.log(1)").unwrap();
    dir
}

async fn get(app: axum::Router, uri: &str) -> (StatusCode, String) {
    let res = app
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn health_reports_ok_without_a_graph() {
    let (status, body) = get(app(config(fixture_dist())), "/api/health").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert!(json["graph_date"].is_null());
}

#[tokio::test]
async fn unknown_api_path_is_404_not_the_client() {
    let (status, _) = get(app(config(fixture_dist())), "/api/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn serves_static_assets() {
    let (status, body) = get(app(config(fixture_dist())), "/assets/app.js").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "console.log(1)");
}

#[tokio::test]
async fn unknown_client_path_falls_back_to_index() {
    let (status, body) = get(app(config(fixture_dist())), "/some/deep/link").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("<title>trail-route</title>"));
}
