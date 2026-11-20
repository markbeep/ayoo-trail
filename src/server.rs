//! axum application: API routes and the static client.
//!
//! `/api/*` is the JSON API. Every other path serves the built client from
//! `static_dir`, falling back to `index.html` so client-side deep links work.

use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::cost::{climb_match, traversable, Profile, ProfileError};
use crate::generate::{generate, ClimbGoal, GenerateError, GenerateRequest};
use crate::graph::{Graph, LatLon};
use crate::route::{
    route_cost_per_m, route_geometry, route_stats, route_term_steps, route_via, term_values, Route,
    RouteStats,
};
use crate::snap::{Snap, SNAP_LIMIT_M};

/// Runtime configuration for [`app`].
#[derive(Clone)]
pub struct ServerConfig {
    /// Directory holding the built client (`web/dist`).
    pub static_dir: PathBuf,
    /// Shared read-only graph; `None` runs the server without routing.
    pub graph: Option<Arc<Graph>>,
}

type Shared = Option<Arc<Graph>>;
type ApiError = (StatusCode, Json<Value>);

fn api_err(status: StatusCode, msg: &str) -> ApiError {
    (status, Json(json!({ "error": msg })))
}

fn unprocessable(msg: &str) -> ApiError {
    api_err(StatusCode::UNPROCESSABLE_ENTITY, msg)
}

/// UTC `YYYY-MM-DD` of Unix seconds (Hinnant's civil-from-days).
fn utc_date(secs: i64) -> String {
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{d:02}", yoe + era * 400 + i64::from(m <= 2))
}

async fn health(State(g): State<Shared>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "graph_date": g.as_ref().map(|g| utc_date(g.osm_date())),
        "bbox": g.as_ref().map(|g| g.bbox()),
    }))
}

#[derive(Deserialize)]
struct RouteRequest {
    waypoints: Vec<LatLon>,
    #[serde(default)]
    profile: Profile,
    /// The climb the user asked for; adds the Elevation bar and counts in the score (D63).
    climb_target_m: Option<f64>,
}

async fn route(
    State(g): State<Shared>,
    body: Result<Json<RouteRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let g = g.ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "no graph loaded"))?;
    let Json(req) = body.map_err(|_| unprocessable("invalid request body"))?;
    // Queue (never reject) so concurrent requests can't oversubscribe the CPUs.
    let _permit = permits().acquire().await.expect("semaphore never closed");
    tokio::task::spawn_blocking(move || compute_route(&g, req))
        .await
        .map_err(|_| api_err(StatusCode::INTERNAL_SERVER_ERROR, "routing failed"))?
        .map(Json)
}

fn permits() -> &'static tokio::sync::Semaphore {
    static S: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    S.get_or_init(|| {
        tokio::sync::Semaphore::new(std::thread::available_parallelism().map_or(4, |n| n.get()))
    })
}

fn valid_point(p: &LatLon) -> bool {
    // Also keeps NaN out of the snap R-tree.
    p.lat.is_finite() && p.lon.is_finite() && p.lat.abs() <= 90.0 && p.lon.abs() <= 180.0
}

fn check_profile(profile: &Profile) -> Result<(), ApiError> {
    profile.validate().map_err(|e| {
        unprocessable(match e {
            ProfileError::WeightOutOfRange => "weight out of range",
            ProfileError::MaxSacOutOfRange => "max_sac out of range",
            ProfileError::BadExclusion => "bad exclusion",
        })
    })
}

fn compute_route(g: &Graph, req: RouteRequest) -> Result<Value, ApiError> {
    let RouteRequest {
        waypoints,
        profile,
        climb_target_m,
    } = req;
    if climb_target_m.is_some_and(|m| !(1.0..=20_000.0).contains(&m)) {
        return Err(unprocessable("climb_target_m out of range"));
    }
    if !(2..=20).contains(&waypoints.len()) {
        return Err(unprocessable("need 2-20 waypoints"));
    }
    if !waypoints.iter().all(valid_point) {
        return Err(unprocessable("invalid coordinates"));
    }
    check_profile(&profile)?;

    let mut snaps: Vec<Snap> = Vec::with_capacity(waypoints.len());
    for (i, &p) in waypoints.iter().enumerate() {
        let ok = |e| traversable(g, e, &profile);
        // Later waypoints prefer the first one's network; one on an island can't be reached,
        // and A* would only find that out after exploring the whole graph.
        let snap = match snaps.first() {
            None => g.snap_start(p, SNAP_LIMIT_M, profile.max_sac, ok),
            Some(s0) => {
                let home = g.edge_component(s0.edge, profile.max_sac);
                g.snap_filtered(p, SNAP_LIMIT_M, |e| {
                    ok(e) && g.edge_component(e, profile.max_sac) == home
                })
                .or_else(|| g.snap_filtered(p, SNAP_LIMIT_M, ok))
            }
        };
        snaps.push(snap.ok_or_else(|| {
            let n = i + 1;
            unprocessable(&if g.snap(p).is_some() {
                format!("waypoint {n} is blocked")
            } else {
                format!("no trail near waypoint {n}")
            })
        })?);
    }

    let no_path = || unprocessable("no path between waypoints");
    if snaps.iter().any(|s| {
        g.edge_component(s.edge, profile.max_sac)
            != g.edge_component(snaps[0].edge, profile.max_sac)
    }) {
        return Err(no_path());
    }
    let r = route_via(g, &profile, &snaps).ok_or_else(no_path)?;
    if r.distance_m < 1.0 {
        return Err(unprocessable("waypoints are too close together"));
    }
    Ok(route_json(
        g,
        &r,
        &waypoints,
        &profile,
        climb_target_m,
        "manual",
        true,
    ))
}

fn route_json(
    g: &Graph,
    r: &Route,
    waypoints: &[LatLon],
    profile: &Profile,
    climb_target_m: Option<f64>,
    id: &str,
    distance_met: bool,
) -> Value {
    let stats = route_stats(g, r, profile);
    let bars = breakdown(&stats, profile, climb_target_m);
    let r6 = |x: f64| (x * 1e6).round() / 1e6;
    let coordinates: Vec<_> = route_geometry(g, r)
        .into_iter()
        .map(|(la, lo, z)| json!([r6(lo), r6(la), (f64::from(z) * 10.0).round() / 10.0]))
        .collect();
    json!({
        "id": id,
        "geometry": { "type": "LineString", "coordinates": coordinates },
        "waypoints": waypoints,
        "distance_m": r.distance_m,
        "climb_m": r.climb_m,
        "descent_m": r.descent_m,
        // D84: how close the loop gets to the best the user's weights allow, the measure the
        // generator ranks by; a climb target adds its match as a seventh term.
        "score": match climb_target_m {
            Some(t) => 100.0 * (stats.preference * climb_match(r.climb_m, t)).powf(1.0 / 8.0),
            None => 100.0 * stats.preference.powf(1.0 / 7.0),
        },
        "official_share": stats.official_share,
        "breakdown": bars,
        "distance_met": distance_met,
        "climb_pressure": profile.weights.climb_pressure,
        "terms": route_term_steps(g, r, profile).iter().map(|(i, b)| json!({
            "i": i, "official": b.official, "surface": b.surface, "slope": b.slope,
            "tech": b.tech, "forest": b.forest, "urban": b.urban, "traffic": b.traffic,
        })).collect::<Vec<_>>(),
        "cost_per_m": route_cost_per_m(g, r, profile).iter().map(|c| (c * 1e3).round() / 1e3).collect::<Vec<_>>(),
        "graph_date": utc_date(g.osm_date()),
        "stats": stats,
    })
}

#[derive(Deserialize)]
struct GenerateBody {
    start: LatLon,
    distance_m: f64,
    climb: Option<ClimbBody>,
    #[serde(default)]
    profile: Profile,
    #[serde(default = "default_count")]
    count: usize,
}

#[derive(Deserialize)]
struct ClimbBody {
    /// Absent or null: as much climb as possible.
    target_m: Option<f64>,
    strictness: f32,
}

fn default_count() -> usize {
    6
}

async fn generate_loops(
    State(g): State<Shared>,
    body: Result<Json<GenerateBody>, JsonRejection>,
) -> Result<([(header::HeaderName, &'static str); 1], Json<Value>), ApiError> {
    let g = g.ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "no graph loaded"))?;
    let Json(req) = body.map_err(|_| unprocessable("invalid request body"))?;
    let _permit = permits().acquire().await.expect("semaphore never closed");
    let v = tokio::task::spawn_blocking(move || compute_loops(&g, req))
        .await
        .map_err(|_| api_err(StatusCode::INTERNAL_SERVER_ERROR, "generation failed"))??;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(v)))
}

fn compute_loops(g: &Graph, b: GenerateBody) -> Result<Value, ApiError> {
    if !valid_point(&b.start) {
        return Err(unprocessable("invalid coordinates"));
    }
    if !(500.0..=80_000.0).contains(&b.distance_m) {
        return Err(unprocessable("distance_m must be 500-80000"));
    }
    if !(1..=12).contains(&b.count) {
        return Err(unprocessable("count must be 1-12"));
    }
    if let Some(c) = &b.climb {
        if c.target_m.is_some_and(|m| !(0.0..=20_000.0).contains(&m)) {
            return Err(unprocessable("climb.target_m out of range"));
        }
        if !(0.0..=1.0).contains(&c.strictness) {
            return Err(unprocessable("climb.strictness must be 0-1"));
        }
    }
    check_profile(&b.profile)?;
    let req = GenerateRequest {
        start: b.start,
        distance_m: b.distance_m,
        climb: b.climb.map(|c| ClimbGoal {
            target_m: c.target_m,
            strictness: c.strictness,
        }),
        profile: b.profile,
        count: b.count,
    };
    let routes = generate(g, &req).map_err(|e| {
        unprocessable(match e {
            GenerateError::StartOffNetwork => "no trail near the start",
            GenerateError::StartBlocked => "start is blocked",
        })
    })?;
    if routes.is_empty() {
        return Err(unprocessable("no loop found from this start"));
    }
    let target = req.climb.and_then(|c| c.target_m).filter(|m| *m > 0.0);
    let routes: Vec<_> = routes
        .iter()
        .map(|r| {
            route_json(
                g,
                &r.route,
                &r.waypoints,
                &r.profile,
                target,
                "",
                r.distance_met,
            )
        })
        .collect();
    Ok(json!({ "routes": best_first(routes) }))
}

/// Loops ordered by score, best first, then named `loop-0`, `loop-1`… in that order (D79).
fn best_first(mut routes: Vec<Value>) -> Vec<Value> {
    let score = |r: &Value| r["score"].as_f64().unwrap_or(0.0);
    routes.sort_by(|a, b| score(b).total_cmp(&score(a)));
    for (i, r) in routes.iter_mut().enumerate() {
        r["id"] = json!(format!("loop-{i}"));
    }
    routes
}

/// Per-term score items: 1 means the route hit the best factor, on the unit-weight scale (D73).
fn breakdown(s: &RouteStats, p: &Profile, climb_target_m: Option<f64>) -> Value {
    let vals = term_values(s, p);
    let value = |k: &str| vals.iter().find(|t| t.0 == k).map_or(0.0, |t| t.1);
    let km = |m: f64| format!("{:.1} km", m / 1000.0);
    let surface = &s.surface_m;
    let tech = match s.missing_tags_m {
        m if m > 0.0 => format!(", {} untagged", km(m)),
        _ => String::new(),
    };
    let forest = if s.distance_m > 0.0 {
        100.0 * s.forest_m / s.distance_m
    } else {
        0.0
    };
    let mut bars = vec![];
    if let Some(t) = climb_target_m {
        bars.push(json!({"key": "climb", "value": climb_match(s.climb_m, t),
            "detail": format!("{:.0} m of climb, aiming for {:.0} m", s.climb_m, t)}));
    }
    bars.extend([
        json!({"key": "official", "value": value("official"),
         "detail": format!("{} on marked trails", km(s.official_m))}),
        json!({"key": "surface", "value": value("surface"),
         "detail": format!("{} on asphalt or paved", km(surface[0] + surface[1]))}),
        json!({"key": "slope", "value": value("slope"),
         "detail": format!("{} steep uphill and {} steep downhill (over 15 %)",
             km(s.grade_m[6..].iter().sum()), km(s.grade_m[..3].iter().sum()))}),
        json!({"key": "tech", "value": value("tech"),
         "detail": format!("{} T3 or harder{tech}", km(s.sac_m[3..].iter().sum()))}),
        json!({"key": "forest", "value": value("forest"),
         "detail": format!("{forest:.0} % in forest")}),
        json!({"key": "urban", "value": value("urban"),
         "detail": format!("{} in towns, {} next to them", km(s.urban_m[2]), km(s.urban_m[1]))}),
        json!({"key": "traffic", "value": value("traffic"),
         "detail": format!("{} on main roads, {} on minor ones",
             km(s.traffic_m[1..].iter().sum()), km(s.traffic_m[0]))}),
    ]);
    Value::Array(bars)
}

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

/// Build the full application router.
pub fn app(config: ServerConfig) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/route", post(route))
        .route("/generate", post(generate_loops))
        .fallback(api_not_found)
        .with_state(config.graph);

    let index = config.static_dir.join("index.html");
    let client = ServeDir::new(&config.static_dir).fallback(ServeFile::new(index));

    Router::new()
        .nest("/api", api)
        .fallback_service(client)
        .layer(TraceLayer::new_for_http())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn best_first_sorts_by_score_and_renames() {
        let r = |s: f64| json!({"id": "", "score": s});
        let out = best_first(vec![r(70.0), r(88.0), r(80.0)]);
        let got: Vec<_> = out
            .iter()
            .map(|v| {
                (
                    v["id"].as_str().unwrap().to_string(),
                    v["score"].as_f64().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                ("loop-0".into(), 88.0),
                ("loop-1".into(), 80.0),
                ("loop-2".into(), 70.0)
            ]
        );
    }
}
