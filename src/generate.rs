//! Loop generation (D39): triangle seeds at fixed headings, pulled toward nearby anchors,
//! radius rescaled until the length fits, then ranked and deduplicated.

use std::collections::HashMap;

use rayon::prelude::*;

use crate::cost::{traversable, Profile};
use crate::geodesy::{haversine_m, LocalProjection};
use crate::graph::{Graph, LatLon};
use crate::route::{drop_backtracks, route_geometry, route_legs, route_stats, Route, Visited};
use crate::snap::{Snap, SNAP_LIMIT_M};

const HEADINGS: usize = 12;
/// Network distance over straight-line distance on trails.
const DETOUR: f64 = 1.3;
const VIA_SNAP_M: f32 = 500.0;
/// Edges this close to the start may be reused without penalty (the access stem).
const STEM_M: f64 = 500.0;
const TOLERANCE: f64 = 0.10;
const MAX_OVERLAP: f64 = 0.5;
const ATTEMPTS: usize = 8;

pub struct GenerateRequest {
    pub start: LatLon,
    pub distance_m: f64,
    pub climb_m: Option<f64>,
    pub profile: Profile,
    pub count: usize,
}

pub struct GeneratedRoute {
    pub route: Route,
    /// Start, the via points, start: the drag handles.
    pub waypoints: Vec<LatLon>,
    pub distance_met: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum GenerateError {
    StartOffNetwork,
    StartBlocked,
}

struct Candidate {
    route: Route,
    waypoints: Vec<LatLon>,
    err: f64,
    rank: f64,
}

pub fn generate(g: &Graph, req: &GenerateRequest) -> Result<Vec<GeneratedRoute>, GenerateError> {
    let p = &req.profile;
    let ok = |e: u32| traversable(g, e, p);
    let start = match g.snap_filtered(req.start, SNAP_LIMIT_M, ok) {
        Some(s) => s,
        None if g.snap(req.start).is_some() => return Err(GenerateError::StartBlocked),
        None => return Err(GenerateError::StartOffNetwork),
    };
    let target = req.distance_m;

    let mut cands: Vec<Candidate> = (0..HEADINGS)
        .into_par_iter()
        .filter_map(|i| seed(g, req, start, i as f64 * 360.0 / HEADINGS as f64))
        .collect();
    cands.sort_by(|a, b| {
        (a.err > TOLERANCE)
            .cmp(&(b.err > TOLERANCE))
            .then(a.rank.total_cmp(&b.rank))
    });

    let mut kept: Vec<(Candidate, HashMap<u32, f64>)> = Vec::new();
    for c in cands {
        if kept.len() >= req.count {
            break;
        }
        let cov = coverage(g, &c.route);
        let distinct = kept.iter().all(|(k, kc)| {
            let shared: f64 = cov
                .iter()
                .filter_map(|(e, a)| kc.get(e).map(|b| a.min(*b)))
                .sum();
            shared / c.route.distance_m.min(k.route.distance_m).max(1.0) <= MAX_OVERLAP
        });
        if distinct {
            kept.push((c, cov));
        }
    }
    Ok(kept
        .into_iter()
        .map(|(c, _)| GeneratedRoute {
            distance_met: (c.route.distance_m - target).abs() / target <= TOLERANCE,
            route: c.route,
            waypoints: c.waypoints,
        })
        .collect())
}

/// One heading: the best triangle loop over a few radius corrections.
fn seed(g: &Graph, req: &GenerateRequest, start: Snap, heading_deg: f64) -> Option<Candidate> {
    let p = &req.profile;
    let target = req.distance_m;
    let proj = LocalProjection::new(start.pos.lat);
    let o = proj.project(start.pos);
    let mut r = target / (3.0 * DETOUR);
    let mut best: Option<Candidate> = None;
    for _ in 0..ATTEMPTS {
        let vias: Option<Vec<Snap>> = [-30.0, 30.0]
            .iter()
            .map(|d: &f64| {
                let b = (heading_deg + d).to_radians();
                let pt = pull(
                    g,
                    proj.unproject([o[0] + r * b.sin(), o[1] + r * b.cos()]),
                    r / 3.0,
                );
                g.snap_filtered(pt, VIA_SNAP_M, |e| traversable(g, e, p))
            })
            .collect();
        let Some(vias) = vias else {
            // Off the network (lake, graph edge, tiny village): pull the triangle in.
            r *= 0.7;
            continue;
        };
        let snaps = [start, vias[0], vias[1], start];
        let away_from_start = |e: u32| {
            haversine_m(g.node(g.edge_from(e)), start.pos) > STEM_M
                || haversine_m(g.node(g.edge_to(e)), start.pos) > STEM_M
        };
        let mut visited = Visited::new(g.edge_count());
        let Some(route) = route_legs(g, p, &snaps, &mut visited, away_from_start) else {
            r *= 0.7;
            continue;
        };
        // A via point on a dead end would leave an out-and-back spur; cut it. Where the
        // network allows only out-and-backs (trimming removes most of it), keep the original.
        let trimmed = drop_backtracks(g, route.clone());
        let route = if trimmed.distance_m >= 0.5 * route.distance_m {
            trimmed
        } else {
            route
        };
        if route.distance_m < 1.0 {
            r *= 1.5;
            continue;
        }
        let err = (route.distance_m - target).abs() / target;
        let climb_term = req
            .climb_m
            .filter(|c| *c > 0.0)
            .map_or(0.0, |c| (route.climb_m - c).abs() / c);
        let score = route_stats(g, &route, p).score as f64;
        let rank = err + 0.25 * climb_term - 0.2 * score / 100.0;
        let scale = (target / route.distance_m).clamp(0.5, 2.0);
        if best.as_ref().is_none_or(|b| err < b.err) {
            best = Some(Candidate {
                waypoints: vec![
                    req.start,
                    on_route(g, &route, vias[0].pos),
                    on_route(g, &route, vias[1].pos),
                    req.start,
                ],
                route,
                err,
                rank,
            });
        }
        if err <= TOLERANCE {
            break;
        }
        r *= scale;
    }
    best
}

/// The route vertex nearest `p`: a via point cut off with its spur moves back onto the loop.
fn on_route(g: &Graph, r: &Route, p: LatLon) -> LatLon {
    route_geometry(g, r)
        .into_iter()
        .map(|(lat, lon, _)| LatLon { lat, lon })
        .min_by(|a, b| haversine_m(*a, p).total_cmp(&haversine_m(*b, p)))
        .unwrap_or(p)
}

/// The nearest anchor within `max_m` of `pt`, else `pt` itself.
fn pull(g: &Graph, pt: LatLon, max_m: f64) -> LatLon {
    g.anchors()
        .iter()
        .map(|a| (haversine_m(a.pos, pt), a.pos))
        .filter(|(d, _)| *d <= max_m)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(pt, |(_, pos)| pos)
}

/// Edge id → metres of it the route covers.
fn coverage(g: &Graph, r: &Route) -> HashMap<u32, f64> {
    let mut m = HashMap::new();
    for s in &r.steps {
        *m.entry(s.edge).or_insert(0.0) += s.fraction() * g.edge_len_m(s.edge);
    }
    m
}
