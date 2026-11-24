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
        transit: None,
        windy_key: None,
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

#[tokio::test]
async fn webcams_without_a_key_are_503_and_bad_bboxes_422() {
    assert_eq!(
        get(
            app(config(fixture_dist())),
            "/api/webcams?bbox=47.33,9.12,47.27,9.04"
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let with_key = app(ServerConfig {
        windy_key: Some("k".into()),
        ..config(fixture_dist())
    });
    assert_eq!(
        get(with_key, "/api/webcams?bbox=48,12,46,8").await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

async fn post_travel(
    app: axum::Router,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let res = app
        .oneshot(
            Request::post("/api/travel")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null),
    )
}

fn with_transit() -> ServerConfig {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gtfs");
    let t = trail_calc::transit::Transit::build(&dir, None).unwrap();
    ServerConfig {
        transit: Some(std::sync::Arc::new(t)),
        ..config(fixture_dist())
    }
}

#[tokio::test]
async fn travel_is_503_without_transit_data() {
    let body = serde_json::json!({"origins": [{"lat": 47.0, "lon": 8.0, "minutes": 60}]});
    let (status, _) = post_travel(app(config(fixture_dist())), body).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn travel_rejects_bad_budgets_and_too_many_origins() {
    let one = |m: u32| serde_json::json!({"lat": 47.0, "lon": 8.0, "minutes": m});
    for origins in [vec![one(5)], vec![one(241)], vec![one(60); 9], vec![]] {
        let (status, _) = post_travel(
            app(with_transit()),
            serde_json::json!({ "origins": origins }),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
}

#[tokio::test]
async fn travel_returns_an_image_and_its_bounds() {
    let body = serde_json::json!({"origins": [{"lat": 47.0, "lon": 8.0, "minutes": 30}]});
    let (status, json) = post_travel(app(with_transit()), body).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["image"]
        .as_str()
        .unwrap()
        .starts_with("data:image/png;base64,"));
    let b: Vec<f64> = serde_json::from_value(json["bounds"].clone()).unwrap();
    assert!(b[0] < 8.0 && b[2] > 8.3 && b[1] < 47.0 && b[3] > 47.0);
}
