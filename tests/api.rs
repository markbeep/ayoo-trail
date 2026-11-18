use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use trail_route::graph::{EdgeAttrs, Graph, GraphWriter, LatLon, Surface};
use trail_route::server::{app, ServerConfig};

const LAT: f64 = 46.9;

fn ll(lat: f64, lon: f64) -> LatLon {
    LatLon { lat, lon }
}

fn attrs(len: f32, sac: u8) -> EdgeAttrs {
    EdgeAttrs {
        length_m: len,
        surface: Surface::Gravel,
        sac,
        ..Default::default()
    }
}

/// Path (46.9, 7.4)-(46.9, 7.402) in two ~76 m edges, a sac-5 spur, and a disconnected island.
fn graph() -> Arc<Graph> {
    let mut w = GraphWriter::new(1_700_000_000);
    let n: Vec<u32> = [7.4, 7.401, 7.402]
        .iter()
        .map(|&lon| w.add_node(ll(LAT, lon), 500.0))
        .collect();
    for i in 0..2 {
        let (a, b) = (w.node_pos(n[i]), w.node_pos(n[i + 1]));
        w.add_edge(
            n[i],
            n[i + 1],
            attrs(76.0, 1),
            &[(a.lat, a.lon, 500.0), (b.lat, b.lon, 510.0)],
        );
    }
    let spur = w.add_node(ll(46.901, 7.401), 500.0);
    w.add_edge(
        n[1],
        spur,
        attrs(111.0, 5),
        &[(LAT, 7.401, 500.0), (46.901, 7.401, 500.0)],
    );
    let (i0, i1) = (
        w.add_node(ll(46.95, 7.4), 500.0),
        w.add_node(ll(46.95, 7.401), 500.0),
    );
    w.add_edge(
        i0,
        i1,
        attrs(76.0, 1),
        &[(46.95, 7.4, 500.0), (46.95, 7.401, 500.0)],
    );
    Arc::new(Graph::from_bytes(w.finish()).unwrap())
}

fn router(g: Option<Arc<Graph>>) -> axum::Router {
    app(ServerConfig {
        static_dir: std::env::temp_dir(),
        graph: g,
    })
}

async fn send(app: axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&body).unwrap_or_else(|_| panic!("non-JSON body: {body:?}"));
    (status, json)
}

async fn post(g: Option<Arc<Graph>>, body: Value) -> (StatusCode, Value) {
    post_raw(g, body.to_string()).await
}

async fn post_raw(g: Option<Arc<Graph>>, body: String) -> (StatusCode, Value) {
    let req = Request::post("/api/route")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    send(router(g), req).await
}

fn two(a: (f64, f64), b: (f64, f64)) -> Value {
    json!({"waypoints":[{"lat":a.0,"lon":a.1},{"lat":b.0,"lon":b.1}]})
}

fn err(v: &Value) -> &str {
    v["error"].as_str().unwrap()
}

#[tokio::test]
async fn health_reports_graph_date_and_bbox() {
    let req = Request::get("/api/health").body(Body::empty()).unwrap();
    let (status, v) = send(router(Some(graph())), req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["status"], "ok");
    assert_eq!(v["graph_date"], "2023-11-14");
    assert!((v["bbox"]["min_lon"].as_f64().unwrap() - 7.4).abs() < 1e-6);
    assert!((v["bbox"]["max_lat"].as_f64().unwrap() - 46.95).abs() < 1e-6);
}

#[tokio::test]
async fn route_between_two_points() {
    let (status, v) = post(Some(graph()), two((LAT, 7.4), (LAT, 7.402))).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["id"], "manual");
    assert_eq!(v["distance_met"], true);
    assert_eq!(v["graph_date"], "2023-11-14");
    assert_eq!(v["waypoints"].as_array().unwrap().len(), 2);
    assert!(v["stats"]["distance_m"].is_number());
    assert!((v["distance_m"].as_f64().unwrap() - 152.0).abs() < 5.0);
    assert!(v["score"].as_f64().unwrap() > 0.0);
    let c0 = &v["geometry"]["coordinates"][0];
    assert!((c0[0].as_f64().unwrap() - 7.4).abs() < 1e-4, "lon first");
    assert!((c0[1].as_f64().unwrap() - 46.9).abs() < 1e-4, "lat second");
    let keys: Vec<_> = v["breakdown"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| {
            let x = b["value"].as_f64().unwrap();
            assert!((0.0..=1.0).contains(&x));
            assert!(b["detail"].is_string());
            b["key"].as_str().unwrap()
        })
        .collect();
    assert_eq!(
        keys,
        ["official", "surface", "slope", "tech", "forest", "urban"]
    );
}

#[tokio::test]
async fn route_between_two_points_on_the_same_edge_is_direct() {
    let (status, v) = post(Some(graph()), two((LAT, 7.4001), (LAT, 7.4009))).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let d = v["distance_m"].as_f64().unwrap();
    assert!((50.0..70.0).contains(&d), "{d}");
}

#[tokio::test]
async fn off_network_start_is_422() {
    let (status, v) = post(Some(graph()), two((47.5, 8.0), (LAT, 7.402))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err(&v), "no trail near waypoint 1");
}

#[tokio::test]
async fn route_rejects_single_and_too_many_waypoints() {
    let p = json!({"lat":LAT,"lon":7.4});
    for n in [1, 21] {
        let (status, v) = post(Some(graph()), json!({"waypoints": vec![p.clone(); n]})).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err(&v), "need 2-20 waypoints");
    }
}

#[tokio::test]
async fn blocked_start_is_422() {
    let mut body = two((LAT, 7.4), (LAT, 7.402));
    body["profile"] = json!({"exclude":[{"lat":LAT,"lon":7.4,"radius_m":500,"mode":"block"}]});
    let (status, v) = post(Some(graph()), body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err(&v), "waypoint 1 is blocked");
}

#[tokio::test]
async fn bad_weight_is_422() {
    let mut body = two((LAT, 7.4), (LAT, 7.402));
    body["profile"] = json!({"weights":{"slope":9.0}});
    let (status, v) = post(Some(graph()), body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err(&v), "weight out of range");
}

#[tokio::test]
async fn nan_or_out_of_range_coordinates_are_422() {
    let (status, v) = post(Some(graph()), two((91.0, 7.4), (LAT, 7.402))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err(&v), "invalid coordinates");
    let raw = r#"{"waypoints":[{"lat":1e999,"lon":7.4},{"lat":46.9,"lon":7.4}]}"#;
    let (status, v) = post_raw(Some(graph()), raw.into()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(v["error"].is_string());
}

#[tokio::test]
async fn malformed_json_is_422_json() {
    let (status, v) = post_raw(Some(graph()), "{not json".into()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(v["error"].is_string());
}

#[tokio::test]
async fn disconnected_waypoints_are_422() {
    let (status, v) = post(Some(graph()), two((LAT, 7.4), (46.95, 7.4005))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err(&v), "no path between waypoints");
}

#[tokio::test]
async fn identical_waypoints_are_422() {
    let (status, v) = post(Some(graph()), two((LAT, 7.4005), (LAT, 7.4005))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err(&v), "waypoints are too close together");
}

#[tokio::test]
async fn route_without_graph_is_503() {
    let (status, v) = post(None, two((LAT, 7.4), (LAT, 7.402))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(err(&v), "no graph loaded");
}
