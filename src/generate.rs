//! Loop generation (D39): triangle and 3-via seeds at fixed headings, pulled toward nearby
//! anchors, radius rescaled until the length fits and climb pressure tuned until the climb fits
//! (D52), then ranked and deduplicated.

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
const CLIMB_TOLERANCE: f64 = 0.15;
/// Via-point bearings around the seed heading: a triangle, and a 3-via loop that can cross
/// more hills.
const SHAPES: [&[f64]; 2] = [&[-30.0, 30.0], &[-40.0, 0.0, 40.0]];

/// How much climb the generator chases (D52).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClimbGoal {
    /// Meters of climb to aim for; None = as much as possible.
    pub target_m: Option<f64>,
    /// 0..1: how hard to chase it, both in ranking and as the ceiling of the climb pressure.
    pub strictness: f32,
}

pub struct GenerateRequest {
    pub start: LatLon,
    pub distance_m: f64,
    pub climb: Option<ClimbGoal>,
    pub profile: Profile,
    pub count: usize,
}

pub struct GeneratedRoute {
    pub route: Route,
    /// Start, the via points, start: the drag handles.
    pub waypoints: Vec<LatLon>,
    pub distance_met: bool,
    /// The profile it was routed with (the request's plus its climb pressure), so a drag
    /// edit re-routes under the same costs.
    pub profile: Profile,
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
    pressure: f32,
}

impl Candidate {
    /// In tolerance first, then by rank.
    fn better_than(&self, o: &Candidate) -> bool {
        ((self.err > TOLERANCE), self.rank) < ((o.err > TOLERANCE), o.rank)
    }
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

    let mut cands: Vec<Candidate> = (0..HEADINGS * SHAPES.len())
        .into_par_iter()
        .filter_map(|i| {
            let heading = (i % HEADINGS) as f64 * 360.0 / HEADINGS as f64;
            seed(g, req, start, heading, SHAPES[i / HEADINGS])
        })
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
            profile: with_pressure(p, c.pressure),
        })
        .collect())
}

fn with_pressure(p: &Profile, pressure: f32) -> Profile {
    let mut p = p.clone();
    p.weights.climb_pressure = pressure;
    p
}

/// One heading and shape: the best loop over a few radius and climb-pressure corrections.
fn seed(
    g: &Graph,
    req: &GenerateRequest,
    start: Snap,
    heading_deg: f64,
    bearings: &[f64],
) -> Option<Candidate> {
    let target = req.distance_m;
    let proj = LocalProjection::new(start.pos.lat);
    let o = proj.project(start.pos);
    // Perimeter of the seed polygon in units of r: two spokes plus the chords between vias.
    let perimeter = 2.0
        + bearings
            .windows(2)
            .map(|w| 2.0 * ((w[1] - w[0]).to_radians() / 2.0).sin())
            .sum::<f64>();
    let mut r = target / (perimeter * DETOUR);
    // Climb pressure is bisected inside [lo, hi] like the radius is rescaled (D52).
    let goal = req.climb.filter(|c| c.strictness > 0.0);
    let (mut lo, mut hi) = (0.0f32, goal.map_or(0.0, |c| c.strictness));
    let mut pressure = match goal {
        Some(ClimbGoal {
            target_m: Some(_),
            strictness,
        }) => strictness * 0.5,
        Some(c) => c.strictness,
        None => 0.0,
    };
    let mut best: Option<Candidate> = None;
    for _ in 0..ATTEMPTS {
        let pp = with_pressure(&req.profile, pressure);
        let p = &pp;
        let vias: Option<Vec<Snap>> = bearings
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
        let snaps: Vec<Snap> = std::iter::once(start)
            .chain(vias.iter().copied())
            .chain(std::iter::once(start))
            .collect();
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
        // Signed climb error for a target; for "as much as possible", climb per 100 m/km.
        let (climb_term, climb_err) = match req.climb {
            Some(ClimbGoal {
                target_m: Some(c),
                strictness,
            }) if c > 0.0 => {
                let e = (route.climb_m - c) / c;
                ((0.25 + 4.0 * strictness as f64) * e.abs(), Some(e))
            }
            Some(ClimbGoal {
                target_m: None,
                strictness,
            }) => (
                -(0.25 + 4.0 * strictness as f64) * route.climb_m / route.distance_m / 0.1,
                None,
            ),
            _ => (0.0, None),
        };
        let score = route_stats(g, &route, p).score as f64;
        let rank = err + climb_term - 0.2 * score / 100.0;
        let scale = (target / route.distance_m).clamp(0.5, 2.0);
        let c = Candidate {
            waypoints: std::iter::once(req.start)
                .chain(vias.iter().map(|v| on_route(g, &route, v.pos)))
                .chain(std::iter::once(req.start))
                .collect(),
            route,
            err,
            rank,
            pressure,
        };
        if best.as_ref().is_none_or(|b| c.better_than(b)) {
            best = Some(c);
        }
        let climb_ok = climb_err.is_none_or(|e| e.abs() <= CLIMB_TOLERANCE);
        if err <= TOLERANCE && climb_ok {
            break;
        }
        if let Some(e) = climb_err.filter(|_| goal.is_some()) {
            if e < -CLIMB_TOLERANCE {
                lo = pressure;
            } else if e > CLIMB_TOLERANCE {
                hi = pressure;
            }
            pressure = (lo + hi) / 2.0;
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

/// Edge id → meters of it the route covers.
fn coverage(g: &Graph, r: &Route) -> HashMap<u32, f64> {
    let mut m = HashMap::new();
    for s in &r.steps {
        *m.entry(s.edge).or_insert(0.0) += s.fraction() * g.edge_len_m(s.edge);
    }
    m
}
