use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::path::PathBuf;
use tower::ServiceExt;
use trail_calc::server::{app, ServerConfig};

fn config(static_dir: PathBuf) -> ServerConfig {
    ServerConfig {
        static_dir,
        graph: None,
        view: None,
    }
}

fn fixture_dist() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("trail-calc-dist-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(dir.join("index.html"), "<!doctype html><title>ayoo</title>").unwrap();
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
    assert!(body.contains("<title>ayoo</title>"));
}

#[tokio::test]
async fn view_tiles_are_png_inside_the_grid_204_outside_and_404_without_view() {
    use std::sync::Arc;
    use trail_calc::import::ch1903::wgs84_to_lv95;
    use trail_calc::view::ViewGrid;
    let (e, n) = wgs84_to_lv95(47.3, 9.09);
    let grid = ViewGrid {
        e0: e - 5000.0,
        n0: n + 5000.0,
        cell: 100.0,
        w: 100,
        h: 100,
        v: vec![200; 10_000],
    };
    let with = || {
        app(ServerConfig {
            view: Some(Arc::new(grid.clone())),
            ..config(fixture_dist())
        })
    };
    let res = with()
        .oneshot(
            Request::get("/api/tiles/view/12/2151/1435")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "image/png");
    assert_eq!(
        get(with(), "/api/tiles/view/12/0/0").await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        get(with(), "/api/tiles/view/12/9999/0").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(app(config(fixture_dist())), "/api/tiles/view/12/2151/1435")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
