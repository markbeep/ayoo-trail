//! Loop generation (D39): triangle and 3-via seeds at fixed headings, each via placed where the
//! user's costs are lowest nearby, preferring peaks and viewpoints (D82), radius rescaled until
//! the length fits and climb pressure tuned until the climb fits (D52), then ranked by the
//! user's score and deduplicated.

use std::collections::HashMap;

use rayon::prelude::*;

use crate::cost::{traversable, Profile};
use crate::geodesy::{haversine_m, LocalProjection};
use crate::graph::{Graph, LatLon};
use crate::region::Region;
use crate::route::{
    drop_backtracks, route, route_geometry, route_legs_with, route_stats, Route, Visited,
};
use crate::snap::{Snap, SNAP_LIMIT_M};

const HEADINGS: usize = 12;
/// Network distance over straight-line distance on trails.
const DETOUR: f64 = 1.3;
const VIA_SNAP_M: f32 = 500.0;
/// Edges this close to the start may be reused without penalty (the access stem).
const STEM_M: f64 = 500.0;
const TOLERANCE: f64 = 0.10;
/// Via points may move this share of the seed radius to reach cheaper terrain (D85).
const VIA_SEARCH: f64 = 0.5;
/// Weight of a loop's preference (0–1) in its rank, against distance and climb error.
const PREFERENCE_RANK: f64 = 1.0;
const MAX_OVERLAP: f64 = 0.5;
const ATTEMPTS: usize = 8;
/// Search region radius over the target distance. A loop of length D stays within D/2 of
/// its start; legs that leave the region fall back to the whole graph.
const REGION_RADIUS: f64 = 0.5;
/// Below this bracket width the climb-pressure bisection stops: the loops barely change.
const MIN_PRESSURE_STEP: f32 = 0.05;
/// Accepted climb error at strictness 0 and 1: "roughly" is ±15 %, "exactly" ±3 % (D63).
const CLIMB_TOLERANCE: [f64; 2] = [0.15, 0.03];
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
    /// Snapped via points; moved onto the loop only for the loops that are kept.
    vias: Vec<Snap>,
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
    let start = match g.snap_start(req.start, SNAP_LIMIT_M, p.max_sac, ok) {
        Some(s) => s,
        None if g.snap(req.start).is_some() => return Err(GenerateError::StartBlocked),
        None => return Err(GenerateError::StartOffNetwork),
    };
    let target = req.distance_m;
    let region = Region::new(g, p, start, REGION_RADIUS * target);

    let mut cands: Vec<Candidate> = (0..HEADINGS * SHAPES.len())
        .into_par_iter()
        .filter_map(|i| {
            let heading = (i % HEADINGS) as f64 * 360.0 / HEADINGS as f64;
            seed(g, &region, req, start, heading, SHAPES[i / HEADINGS])
        })
        .collect();
    cands.sort_by(|a, b| {
        (a.err > TOLERANCE)
            .cmp(&(b.err > TOLERANCE))
            .then(a.rank.total_cmp(&b.rank))
    });
    // D87: only loops within the distance tolerance; when none fits, the single closest.
    if cands.iter().any(|c| c.err <= TOLERANCE) {
        cands.retain(|c| c.err <= TOLERANCE);
    } else if let Some(i) = (0..cands.len()).min_by(|&a, &b| cands[a].err.total_cmp(&cands[b].err))
    {
        cands = vec![cands.swap_remove(i)];
    }

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
            waypoints: std::iter::once(req.start)
                .chain(c.vias.iter().map(|v| on_route(g, &c.route, v.pos)))
                .chain(std::iter::once(req.start))
                .collect(),
            route: c.route,
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
    region: &Region,
    req: &GenerateRequest,
    start: Snap,
    heading_deg: f64,
    bearings: &[f64],
) -> Option<Candidate> {
    let target = req.distance_m;
    let sac = req.profile.max_sac;
    let home = g.edge_component(start.edge, sac);
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
                let ideal = proj.unproject([o[0] + r * b.sin(), o[1] + r * b.cos()]);
                // D82: the via goes where the user's costs are low, a peak or viewpoint if close.
                let anchor = anchor_near(g, ideal, r / 3.0);
                let pt = region
                    .via_near(pressure, ideal, r * VIA_SEARCH, anchor)
                    .or(anchor)
                    .unwrap_or(ideal);
                g.snap_filtered(pt, VIA_SNAP_M, |e| {
                    g.edge_component(e, sac) == home && traversable(g, e, p)
                })
            })
            .collect();
        let Some(vias) = vias else {
            // Off the network (lake, graph edge, tiny village): pull the triangle in.
            r *= 0.7;
            continue;
        };
        let Some(route) = route_loop(g, region, p, start, &vias) else {
            r *= 0.7;
            continue;
        };
        if route.distance_m < 1.0 {
            r *= 1.5;
            continue;
        }
        let (err, climb_err, rank) = judge(g, req, p, &route);
        let scale = (target / route.distance_m).clamp(0.5, 2.0);
        let c = Candidate {
            vias,
            route,
            err,
            rank,
            pressure,
        };
        if best.as_ref().is_none_or(|b| c.better_than(b)) {
            best = Some(c);
        }
        let tol = req.climb.map_or(CLIMB_TOLERANCE[0], |c| {
            let s = c.strictness.clamp(0.0, 1.0) as f64;
            CLIMB_TOLERANCE[0] + (CLIMB_TOLERANCE[1] - CLIMB_TOLERANCE[0]) * s
        });
        let climb_ok = climb_err.is_none_or(|e| e.abs() <= tol);
        if err <= TOLERANCE && climb_ok {
            break;
        }
        // Climb is settled when it fits, or no pressure in the bracket can change it.
        let mut climb_settled = climb_ok;
        if let Some(e) = climb_err.filter(|_| goal.is_some()) {
            if e < -tol {
                lo = pressure;
                pressure = (lo + hi) / 2.0;
            } else if e > tol {
                hi = pressure;
                pressure = (lo + hi) / 2.0;
            }
            climb_settled |= hi - lo < MIN_PRESSURE_STEP;
        }
        if err <= TOLERANCE && climb_settled {
            break;
        }
        r *= scale;
    }
    best
}

/// The loop start → vias → start, with out-and-back spurs at dead-end vias cut.
fn route_loop(
    g: &Graph,
    region: &Region,
    p: &Profile,
    start: Snap,
    vias: &[Snap],
) -> Option<Route> {
    let snaps: Vec<Snap> = std::iter::once(start)
        .chain(vias.iter().copied())
        .chain(std::iter::once(start))
        .collect();
    let away_from_start = |e: u32| {
        haversine_m(g.node(g.edge_from(e)), start.pos) > STEM_M
            || haversine_m(g.node(g.edge_to(e)), start.pos) > STEM_M
    };
    let mut visited = Visited::new(g.edge_count());
    let leg = |a, b, v: &Visited| region.leg(p, a, b, v).or_else(|| route(g, p, a, b, v));
    let route = route_legs_with(g, &snaps, &mut visited, away_from_start, leg)?;
    // A via point on a dead end would leave an out-and-back spur; cut it. Where the
    // network allows only out-and-backs (trimming removes most of it), keep the original.
    let trimmed = drop_backtracks(g, route.clone());
    Some(if trimmed.distance_m >= 0.5 * route.distance_m {
        trimmed
    } else {
        route
    })
}

/// Distance error, signed climb error (None without a target) and rank of a loop: distance and
/// climb error minus how close every meter is to the best the user's weights allow (D82).
fn judge(g: &Graph, req: &GenerateRequest, p: &Profile, route: &Route) -> (f64, Option<f64>, f64) {
    let target = req.distance_m;
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
    // 0.1 of preference counts like a 10 % distance error. The displayed bars are too flat for
    // this: at unit weight a loop's forest share moves its bar by only a few points.
    let preference = route_stats(g, route, p).preference;
    (
        err,
        climb_err,
        err + climb_term - PREFERENCE_RANK * preference,
    )
}

/// The route vertex nearest `p`: a via point cut off with its spur moves back onto the loop.
fn on_route(g: &Graph, r: &Route, p: LatLon) -> LatLon {
    route_geometry(g, r)
        .into_iter()
        .map(|(lat, lon, _)| LatLon { lat, lon })
        .min_by(|a, b| haversine_m(*a, p).total_cmp(&haversine_m(*b, p)))
        .unwrap_or(p)
}

/// The nearest anchor (peak or viewpoint) within `max_m` of `pt`.
fn anchor_near(g: &Graph, pt: LatLon, max_m: f64) -> Option<LatLon> {
    // A degree of latitude is >= 110.5 km, so this skips the haversine for far anchors.
    let dlat = max_m / 110_000.0;
    g.anchors()
        .iter()
        .filter(|a| (a.pos.lat - pt.lat).abs() <= dlat)
        .map(|a| (haversine_m(a.pos, pt), a.pos))
        .filter(|(d, _)| *d <= max_m)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, pos)| pos)
}

/// Edge id → meters of it the route covers.
fn coverage(g: &Graph, r: &Route) -> HashMap<u32, f64> {
    let mut m = HashMap::new();
    for s in &r.steps {
        *m.entry(s.edge).or_insert(0.0) += s.fraction() * g.edge_len_m(s.edge);
    }
    m
}
