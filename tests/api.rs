use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use trail_calc::graph::{EdgeAttrs, Graph, GraphWriter, LatLon, Surface};
use trail_calc::server::{app, ServerConfig};

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
        view: None,
        transit: None,
        windy_key: None,
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
    post_to(g, "/api/route", body).await
}

async fn generate(g: Option<Arc<Graph>>, body: Value) -> (StatusCode, Value) {
    post_to(g, "/api/generate", body.to_string()).await
}

async fn post_to(g: Option<Arc<Graph>>, path: &str, body: String) -> (StatusCode, Value) {
    let req = Request::post(path)
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
    assert_eq!(v["view"], false, "the test graph has no view scores");
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
        ["official", "surface", "slope", "tech", "forest", "urban", "traffic"]
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
    body["profile"] = json!({"weights":{"slope":101.0}});
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

#[tokio::test]
async fn generate_returns_loops_with_waypoints() {
    let body = json!({"start":{"lat":LAT,"lon":7.4005},"distance_m":500,"profile":{},"count":3,
        "climb":{"target_m":null,"strictness":0.6}});
    let (status, v) = generate(Some(graph()), body).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let routes = v["routes"].as_array().unwrap();
    assert!(!routes.is_empty() && routes.len() <= 3);
    for r in routes {
        assert!(r["waypoints"].as_array().unwrap().len() >= 3);
        assert!(r["distance_met"].as_bool().is_some());
        let cp = r["climb_pressure"].as_f64().unwrap();
        assert!((0.0..=0.6 + 1e-6).contains(&cp), "{cp}");
        assert!(r["geometry"]["coordinates"].as_array().unwrap().len() >= 2);
    }
    // Best score first, ids in that order (D79).
    for (i, w) in routes.windows(2).enumerate() {
        assert!(
            w[0]["score"].as_f64() >= w[1]["score"].as_f64(),
            "{} before {}",
            w[0]["score"],
            w[1]["score"]
        );
        assert_eq!(w[0]["id"], format!("loop-{i}"));
    }
}

#[tokio::test]
async fn generate_off_network_start_is_422() {
    let body = json!({"start":{"lat":47.5,"lon":8.5},"distance_m":5000});
    let (status, v) = generate(Some(graph()), body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(err(&v).contains("no trail"));
}

#[tokio::test]
async fn generate_rejects_bad_distance_and_count() {
    for body in [
        json!({"start":{"lat":LAT,"lon":7.4005},"distance_m":100}),
        json!({"start":{"lat":LAT,"lon":7.4005},"distance_m":90000}),
        json!({"start":{"lat":LAT,"lon":7.4005},"distance_m":5000,"count":0}),
        json!({"start":{"lat":LAT,"lon":7.4005},"distance_m":5000,"climb":{"target_m":-5,"strictness":0.5}}),
        json!({"start":{"lat":LAT,"lon":7.4005},"distance_m":5000,"climb":{"strictness":1.5}}),
    ] {
        let (status, _) = generate(Some(graph()), body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
}

#[tokio::test]
async fn generate_without_graph_is_503() {
    let body = json!({"start":{"lat":LAT,"lon":7.4005},"distance_m":5000});
    let (status, _) = generate(None, body).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn route_carries_cost_per_meter_per_vertex() {
    let plain = two((LAT, 7.4), (LAT, 7.402));
    let (status, v) = post(Some(graph()), plain.clone()).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let n = v["geometry"]["coordinates"].as_array().unwrap().len();
    let cost: Vec<f64> = v["cost_per_m"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_f64().unwrap())
        .collect();
    assert_eq!(cost.len(), n);
    assert!(cost.iter().all(|&c| c > 0.0), "{cost:?}");

    // A penalize marker over the whole route multiplies every vertex by the exclusion penalty.
    let mut marked = plain;
    marked["profile"] =
        json!({"exclude":[{"lat":LAT,"lon":7.401,"radius_m":5000,"mode":"penalize"}]});
    let (_, m) = post(Some(graph()), marked).await;
    let mc: Vec<f64> = m["cost_per_m"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_f64().unwrap())
        .collect();
    assert!(
        (mc[0] / cost[0] - 5.0).abs() < 1e-3,
        "{} vs {}",
        mc[0],
        cost[0]
    );
}

#[tokio::test]
async fn route_carries_per_stretch_term_ratios() {
    let (status, v) = post(Some(graph()), two((LAT, 7.4), (LAT, 7.402))).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let n = v["geometry"]["coordinates"].as_array().unwrap().len() as u64;
    let terms = v["terms"].as_array().unwrap();
    assert!(!terms.is_empty());
    assert_eq!(terms[0]["i"], 0);
    let mut prev = None;
    for t in terms {
        let i = t["i"].as_u64().unwrap();
        assert!(
            i < n && prev.is_none_or(|p| i > p),
            "starts must increase: {terms:?}"
        );
        prev = Some(i);
        for k in [
            "official", "surface", "slope", "tech", "forest", "urban", "traffic",
        ] {
            // Each term relative to the best factor the weights allow: 1 is ideal, never below.
            assert!(t[k].as_f64().unwrap() >= 0.999, "{k} in {t}");
        }
    }
}

#[tokio::test]
async fn climb_target_adds_an_elevation_bar_that_counts_in_the_score() {
    let plain = two((LAT, 7.4), (LAT, 7.402));
    let (_, a) = post(Some(graph()), plain.clone()).await;
    assert!(
        a["breakdown"]
            .as_array()
            .unwrap()
            .iter()
            .all(|b| b["key"] != "climb"),
        "no target, no bar"
    );

    let mut with = plain;
    with["climb_target_m"] = json!(500.0);
    let (status, b) = post(Some(graph()), with).await;
    assert_eq!(status, StatusCode::OK, "{b}");
    let bars = b["breakdown"].as_array().unwrap();
    assert_eq!(bars[0]["key"], "climb", "elevation leads the bars");
    let c = bars[0]["value"].as_f64().unwrap();
    let climb = b["climb_m"].as_f64().unwrap();
    assert!(
        (c - climb.clamp(1.0, 500.0) / climb.max(500.0)).abs() < 1e-6,
        "{c} for {climb} m of 500"
    );
    // D84, D93: score = 100 · (preference · climb match)^(1/8): a missed target lowers it.
    let pref = b["stats"]["preference"].as_f64().unwrap();
    assert!(
        (b["score"].as_f64().unwrap() - 100.0 * (pref * c).powf(1.0 / 8.0)).abs() < 0.05,
        "{b}"
    );
    assert!(b["score"].as_f64().unwrap() < a["score"].as_f64().unwrap());
}

#[tokio::test]
async fn score_is_how_close_the_loop_gets_to_the_best_the_weights_allow() {
    // D84, D93: 100 · preference^(1/7); preference is 1 when every meter is as good as it gets, and
    // all weights off makes every meter ideal.
    for w in [
        json!({}),
        json!({"urban": 4.0, "official": 0.0}),
        json!({"slope": 0.0, "surface": 0.0, "tech": 0.0, "official": 0.0, "forest": 0.0, "urban": 0.0, "traffic": 0.0}),
    ] {
        let mut body = two((LAT, 7.4), (LAT, 7.402));
        body["profile"] = json!({ "weights": w });
        let (status, v) = post(Some(graph()), body).await;
        assert_eq!(status, StatusCode::OK, "{v}");
        let (s, pref) = (
            v["score"].as_f64().unwrap(),
            v["stats"]["preference"].as_f64().unwrap(),
        );
        assert!((0.0..=1.0).contains(&pref), "{pref}");
        assert!(
            (s - 100.0 * pref.powf(1.0 / 7.0)).abs() < 0.05,
            "{s} for {w}: {v}"
        );
    }
    let mut off = two((LAT, 7.4), (LAT, 7.402));
    off["profile"] = json!({"weights": {"slope": 0.0, "surface": 0.0, "tech": 0.0, "official": 0.0, "forest": 0.0, "urban": 0.0, "traffic": 0.0}});
    assert!((post(Some(graph()), off).await.1["score"].as_f64().unwrap() - 100.0).abs() < 0.05);
}
