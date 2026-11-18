//! axum application: API routes and the static client.
//!
//! `/api/*` is the JSON API. Every other path serves the built client from
//! `static_dir`, falling back to `index.html` so client-side deep links work.

use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::cost::{term_minima, traversable, Profile, ProfileError};
use crate::graph::{Graph, LatLon};
use crate::route::{route_geometry, route_stats, route_via, RouteStats};
use crate::snap::SNAP_LIMIT_M;

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
}

async fn route(
    State(g): State<Shared>,
    body: Result<Json<RouteRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let g = g.ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "no graph loaded"))?;
    let Json(req) = body.map_err(|_| unprocessable("invalid request body"))?;
    tokio::task::spawn_blocking(move || compute_route(&g, req))
        .await
        .map_err(|_| api_err(StatusCode::INTERNAL_SERVER_ERROR, "routing failed"))?
        .map(Json)
}

fn compute_route(g: &Graph, req: RouteRequest) -> Result<Value, ApiError> {
    let RouteRequest { waypoints, profile } = req;
    if !(2..=20).contains(&waypoints.len()) {
        return Err(unprocessable("need 2-20 waypoints"));
    }
    // Also keeps NaN out of the snap R-tree.
    if waypoints.iter().any(|p| {
        !(p.lat.is_finite() && p.lon.is_finite() && p.lat.abs() <= 90.0 && p.lon.abs() <= 180.0)
    }) {
        return Err(unprocessable("invalid coordinates"));
    }
    profile.validate().map_err(|e| {
        unprocessable(match e {
            ProfileError::WeightOutOfRange => "weight out of range",
            ProfileError::MaxSacOutOfRange => "max_sac out of range",
            ProfileError::BadExclusion => "bad exclusion",
        })
    })?;

    let mut snaps = Vec::with_capacity(waypoints.len());
    for (i, &p) in waypoints.iter().enumerate() {
        let snap = g.snap_filtered(p, SNAP_LIMIT_M, |e| traversable(g, e, &profile));
        snaps.push(snap.ok_or_else(|| {
            let n = i + 1;
            unprocessable(&if g.snap(p).is_some() {
                format!("waypoint {n} is blocked")
            } else {
                format!("no trail near waypoint {n}")
            })
        })?);
    }

    let r =
        route_via(g, &profile, &snaps).ok_or_else(|| unprocessable("no path between waypoints"))?;
    if r.distance_m < 1.0 {
        return Err(unprocessable("waypoints are too close together"));
    }
    let stats = route_stats(g, &r, &profile);
    let r6 = |x: f64| (x * 1e6).round() / 1e6;
    let coordinates: Vec<_> = route_geometry(g, &r)
        .into_iter()
        .map(|(la, lo, z)| json!([r6(lo), r6(la), (f64::from(z) * 10.0).round() / 10.0]))
        .collect();
    Ok(json!({
        "id": "manual",
        "geometry": { "type": "LineString", "coordinates": coordinates },
        "waypoints": waypoints,
        "distance_m": r.distance_m,
        "climb_m": r.climb_m,
        "descent_m": r.descent_m,
        "score": stats.score,
        "official_share": stats.official_share,
        "breakdown": breakdown(&stats, &profile),
        "distance_met": true,
        "graph_date": utc_date(g.osm_date()),
        "stats": stats,
    }))
}

/// Per-term score items: 1 means the route hit the best factor the weights allow.
fn breakdown(s: &RouteStats, p: &Profile) -> Value {
    let (best, mean) = (term_minima(&p.weights), s.breakdown);
    let value = |b: f32, m: f32| if m <= b { 1.0 } else { (b / m).clamp(0.0, 1.0) };
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
    json!([
        {"key": "official", "value": value(best.official, mean.official),
         "detail": format!("{} on marked trails", km(s.official_m))},
        {"key": "surface", "value": value(best.surface, mean.surface),
         "detail": format!("{} on asphalt or paved", km(surface[0] + surface[1]))},
        {"key": "slope", "value": value(best.slope, mean.slope),
         "detail": format!("{} of steep downhill", km(s.grade_m[..3].iter().sum()))},
        {"key": "tech", "value": value(best.tech, mean.tech),
         "detail": format!("{} T3 or harder{tech}", km(s.sac_m[3..].iter().sum()))},
        {"key": "forest", "value": value(best.forest, mean.forest),
         "detail": format!("{forest:.0} % in forest")},
        {"key": "urban", "value": value(best.urban, mean.urban),
         "detail": format!("{} through built-up areas", km(s.urban_m[1] + s.urban_m[2]))},
    ])
}

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

/// Build the full application router.
pub fn app(config: ServerConfig) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/route", post(route))
        .fallback(api_not_found)
        .with_state(config.graph);

    let index = config.static_dir.join("index.html");
    let client = ServeDir::new(&config.static_dir).fallback(ServeFile::new(index));

    Router::new()
        .nest("/api", api)
        .fallback_service(client)
        .layer(TraceLayer::new_for_http())
}
