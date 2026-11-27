//! Loop generation (D39): triangle and 3-via seeds at fixed headings, each via placed where the
//! user's costs are lowest nearby (D82; peaks and viewpoints don't pull it, D92), radius rescaled until
//! the length fits and climb pressure tuned until the climb fits (D52), then ranked by the
//! user's score and deduplicated.

use std::collections::HashMap;

use rayon::prelude::*;

use crate::cost::{traversable, ExclusionMode, Profile};
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
const TOLERANCE: f64 = 0.10;
/// Via points may move this share of the seed radius to reach cheaper terrain (D85).
const VIA_SEARCH: f64 = 0.5;
/// Weight of a loop's preference (0–1) in its rank, against distance and climb error.
const PREFERENCE_RANK: f64 = 1.0;
/// With hard avoids, a closest loop further off the distance than this is no loop (D97).
const WALLED_ERR: f64 = 0.5;
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

#[derive(Debug, PartialEq)]
pub enum GenerateError {
    StartOffNetwork,
    StartBlocked,
    /// A must-pass area has no trail the start can reach (D97).
    MustPassUnreachable,
    /// The shortest loop through the must-pass areas is this long, over the distance (D97).
    MustPassTooFar {
        min_m: f64,
    },
    /// No seed gave a loop, e.g. hard avoids cut the start off.
    NoLoop,
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
    let start = match g.snap_start(req.start, SNAP_LIMIT_M, p.access(), ok) {
        Some(s) => s,
        None if g.snap(req.start).is_some() => return Err(GenerateError::StartBlocked),
        None => return Err(GenerateError::StartOffNetwork),
    };
    let target = req.distance_m;
    let region = Region::new(g, p, start, REGION_RADIUS * target);
    let musts = must_passes(g, p, start)?;
    let musts = if musts.is_empty() {
        musts
    } else {
        order_musts(g, &region, p, start, &musts).ok_or(GenerateError::MustPassUnreachable)?
    };
    if !musts.is_empty() {
        let shortest = route_loop(g, &region, p, start, &musts, &musts)
            .ok_or(GenerateError::MustPassUnreachable)?;
        if shortest.distance_m > target * (1.0 + TOLERANCE) {
            return Err(GenerateError::MustPassTooFar {
                min_m: shortest.distance_m,
            });
        }
        // D110: when the must-pass points alone fill the distance, they are the vias.
        if shortest.distance_m >= target * (1.0 - TOLERANCE) {
            return Ok(vec![GeneratedRoute {
                distance_met: true,
                waypoints: std::iter::once(req.start)
                    .chain(musts.iter().map(|m| on_route(g, &shortest, m.pos)))
                    .chain(std::iter::once(req.start))
                    .collect(),
                route: shortest,
                profile: p.clone(),
            }]);
        }
    }

    let mut cands: Vec<Candidate> = (0..HEADINGS * SHAPES.len())
        .into_par_iter()
        .filter_map(|i| {
            let heading = (i % HEADINGS) as f64 * 360.0 / HEADINGS as f64;
            seed(
                g,
                &region,
                req,
                start,
                heading,
                SHAPES[i / HEADINGS],
                &musts,
            )
        })
        .collect();
    if cands.is_empty() {
        return Err(GenerateError::NoLoop);
    }
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
        // D97: hard avoids that leave only a stub of the distance count as no loop.
        let walled = p.exclude.iter().any(|x| x.mode == ExclusionMode::Block);
        if walled && cands[i].err > WALLED_ERR {
            return Err(GenerateError::NoLoop);
        }
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
            shared / c.route.distance_m.min(k.route.distance_m).max(1.0) <= p.max_overlap
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
    musts: &[Snap],
) -> Option<Candidate> {
    let target = req.distance_m;
    let sac = req.profile.access();
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
    let mut pressure = goal.map_or(0.0, |c| c.strictness * 0.5);
    let mut best: Option<Candidate> = None;
    for _ in 0..ATTEMPTS {
        let pp = with_pressure(&req.profile, pressure);
        let p = &pp;
        let vias: Option<Vec<Snap>> = bearings
            .iter()
            .map(|d: &f64| {
                let b = (heading_deg + d).to_radians();
                let ideal = proj.unproject([o[0] + r * b.sin(), o[1] + r * b.cos()]);
                // D82, D92: the via goes where the user's costs are low; peaks don't pull it.
                let pt = region
                    .via_near(pressure, ideal, r * VIA_SEARCH, r * DETOUR)
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
        let vias = insert_vias(start, musts, &vias);
        let Some(route) = route_loop(g, region, p, start, &vias, musts) else {
            r *= 0.7;
            continue;
        };
        if route.distance_m < 1.0 {
            r *= 1.5;
            continue;
        }
        let (err, climb_err, rank) = judge(g, req, p, &route);
        // D96: "as much as possible" walks the bracket like an unreachable target. Climb isn't
        // monotonic in pressure, so the best-ranked attempt (most climb per km) is kept.
        let climb_err = climb_err.or(goal.map(|_| f64::NEG_INFINITY));
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

/// Must-pass areas snapped onto a reachable trail inside them (D97). Reachability past hard
/// avoids is checked by the caller.
fn must_passes(g: &Graph, p: &Profile, start: Snap) -> Result<Vec<Snap>, GenerateError> {
    let home = g.edge_component(start.edge, p.access());
    p.exclude
        .iter()
        .filter(|x| x.mode == ExclusionMode::Must)
        .map(|x| {
            let c = LatLon {
                lat: x.lat,
                lon: x.lon,
            };
            g.snap_filtered(c, x.radius_m, |e| {
                g.edge_component(e, p.access()) == home && traversable(g, e, p)
            })
            .ok_or(GenerateError::MustPassUnreachable)
        })
        .collect()
}

/// The must-pass points in the order of the cheapest loop start → all of them → start (D110):
/// exact Held-Karp over the leg costs, at most `MAX_MUST` points. None if a leg is unreachable.
fn order_musts(
    g: &Graph,
    region: &Region,
    p: &Profile,
    start: Snap,
    musts: &[Snap],
) -> Option<Vec<Snap>> {
    let pts: Vec<Snap> = std::iter::once(start)
        .chain(musts.iter().copied())
        .collect();
    let empty = Visited::new(g.edge_count());
    let leg = |a: Snap, b: Snap| {
        region
            .leg(p, a, b, &empty)
            .or_else(|| route(g, p, a, b, &empty))
            .map(|r| r.cost)
    };
    let n = musts.len();
    let mut c = vec![vec![0.0; n + 1]; n + 1];
    for i in 0..=n {
        for j in 0..=n {
            if i != j {
                c[i][j] = leg(pts[i], pts[j])?;
            }
        }
    }
    // best[mask][j]: cheapest path from the start through the musts in `mask`, ending at must j.
    let full = (1usize << n) - 1;
    let mut best = vec![vec![(f64::INFINITY, usize::MAX); n]; full + 1];
    for j in 0..n {
        best[1 << j][j] = (c[0][j + 1], usize::MAX);
    }
    for mask in 1..=full {
        for j in 0..n {
            let (cost, _) = best[mask][j];
            if mask & (1 << j) == 0 || !cost.is_finite() {
                continue;
            }
            for k in 0..n {
                if mask & (1 << k) == 0 {
                    let next = cost + c[j + 1][k + 1];
                    let m = mask | (1 << k);
                    if next < best[m][k].0 {
                        best[m][k] = (next, j);
                    }
                }
            }
        }
    }
    let mut j = (0..n).min_by(|&a, &b| {
        (best[full][a].0 + c[a + 1][0]).total_cmp(&(best[full][b].0 + c[b + 1][0]))
    })?;
    let mut mask = full;
    let mut order = Vec::with_capacity(n);
    while j != usize::MAX {
        order.push(musts[j]);
        let prev = best[mask][j].1;
        mask &= !(1 << j);
        j = prev;
    }
    order.reverse();
    Some(order)
}

/// The free vias inserted into start → `order` → start where each adds the least straight-line
/// detour.
fn insert_vias(start: Snap, order: &[Snap], vias: &[Snap]) -> Vec<Snap> {
    // Without must-pass points the seed's own (clockwise) order stands: loop direction matters.
    if order.is_empty() {
        return vias.to_vec();
    }
    let mut out = order.to_vec();
    for &v in vias {
        let at = |i: usize| {
            if i == 0 || i > out.len() {
                start.pos
            } else {
                out[i - 1].pos
            }
        };
        let i = (0..=out.len())
            .min_by(|&a, &b| {
                let d = |i: usize| {
                    haversine_m(at(i), v.pos) + haversine_m(v.pos, at(i + 1))
                        - haversine_m(at(i), at(i + 1))
                };
                d(a).total_cmp(&d(b))
            })
            .unwrap_or(0);
        out.insert(i, v);
    }
    out
}

/// The loop start → vias → start, with out-and-back spurs at dead-end vias cut.
fn route_loop(
    g: &Graph,
    region: &Region,
    p: &Profile,
    start: Snap,
    vias: &[Snap],
    musts: &[Snap],
) -> Option<Route> {
    let snaps: Vec<Snap> = std::iter::once(start)
        .chain(vias.iter().copied())
        .chain(std::iter::once(start))
        .collect();
    let away_from_start = |e: u32| {
        haversine_m(g.node(g.edge_from(e)), start.pos) > p.stem_m
            || haversine_m(g.node(g.edge_to(e)), start.pos) > p.stem_m
    };
    let mut visited = Visited::new(g.edge_count());
    let leg = |a, b, v: &Visited| region.leg(p, a, b, v).or_else(|| route(g, p, a, b, v));
    // D110: the way straight back from a must-pass point (a summit) is free of the repeat
    // penalty, and its out-and-back is not cut.
    let hold = |i: usize| i < snaps.len() - 1 && musts.contains(&snaps[i]);
    let route = route_legs_with(g, &snaps, &mut visited, away_from_start, leg, hold)?;
    // A via point on a dead end would leave an out-and-back spur; cut it. Where the
    // network allows only out-and-backs (trimming removes most of it), keep the original.
    let trimmed = drop_backtracks(g, route.clone(), musts);
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
    // Signed climb error for a target; for "as much as possible", climb per 100 m/km of the target.
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
            // Total climb over the asked distance, not per km of this loop: a shorter loop
            // shouldn't win on rate while climbing less (D96).
            -(0.25 + 4.0 * strictness as f64) * route.climb_m / target / 0.1,
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

/// Edge id → meters of it the route covers.
fn coverage(g: &Graph, r: &Route) -> HashMap<u32, f64> {
    let mut m = HashMap::new();
    for s in &r.steps {
        *m.entry(s.edge).or_insert(0.0) += s.fraction() * g.edge_len_m(s.edge);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(lat: f64, lon: f64) -> Snap {
        Snap {
            edge: 0,
            t: 0.0,
            pos: LatLon { lat, lon },
            dist_m: 0.0,
        }
    }

    #[test]
    fn without_must_passes_the_seed_via_order_stays() {
        let start = snap(46.0, 7.0);
        let vias = [snap(46.01, 7.0), snap(46.0, 7.01)];
        assert_eq!(insert_vias(start, &[], &vias), vias.to_vec());
    }
}
