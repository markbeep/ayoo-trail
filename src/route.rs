//! Forward A* between snaps, route assembly and per-route metrics (design §8–§10).

use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

use serde::Serialize;

use crate::cost::{edge_cost, edge_factors, score, Breakdown, Profile, Weights};
use crate::geodesy::{gain_loss, haversine_m, polyline_length_m};
use crate::graph::{flags, Direction, Graph, LatLon, FLAT_BAND, GRADE_BANDS};
use crate::snap::Snap;

pub const REPEAT_PENALTY: f64 = 5.0;

/// One traversed piece of an edge. from_t < to_t = forward along the edge, from_t > to_t = backward.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RouteStep {
    pub edge: u32,
    pub from_t: f32,
    pub to_t: f32,
}

impl RouteStep {
    pub fn direction(&self) -> Direction {
        if self.from_t > self.to_t {
            Direction::Backward
        } else {
            Direction::Forward
        }
    }

    pub fn fraction(&self) -> f64 {
        (self.to_t - self.from_t).abs() as f64
    }
}

/// Dense bitset over edge ids (Vec<u64>); consulted in the hot loop.
pub struct Visited {
    bits: Vec<u64>,
}

impl Visited {
    pub fn new(edge_count: usize) -> Self {
        Visited {
            bits: vec![0; edge_count.div_ceil(64)],
        }
    }

    pub fn insert(&mut self, e: u32) {
        let i = e as usize / 64;
        if i >= self.bits.len() {
            self.bits.resize(i + 1, 0);
        }
        self.bits[i] |= 1 << (e % 64);
    }

    pub fn contains(&self, e: u32) -> bool {
        self.bits
            .get(e as usize / 64)
            .is_some_and(|w| w & (1 << (e % 64)) != 0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub steps: Vec<RouteStep>,
    pub distance_m: f64,
    pub climb_m: f64,
    pub descent_m: f64,
    pub cost: f64,
}

/// Per-thread search state; `stamp != gen` means "unset", so queries never clear O(n) memory.
#[derive(Default)]
struct Scratch {
    gen: u32,
    stamp: Vec<u32>,
    dist: Vec<f64>,
    /// Step that reached the node, and whether it is the start partial.
    pred: Vec<(RouteStep, bool)>,
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

fn key(x: f64) -> u64 {
    // Bits of a non-negative f64 order like the value.
    x.max(0.0).to_bits()
}

/// Forward A* between two snaps. Edges in `visited` cost ×REPEAT_PENALTY. None if unreachable
/// or if a snapped edge is not traversable under the profile.
pub fn route(g: &Graph, p: &Profile, start: Snap, end: Snap, visited: &Visited) -> Option<Route> {
    let m = g.edge_count() as u32;
    if start.edge >= m || end.edge >= m {
        return None;
    }
    // Full-edge cost in a direction, with the repetition penalty.
    let full = |e: u32, dir: Direction| {
        let pen = if visited.contains(e) {
            REPEAT_PENALTY
        } else {
            1.0
        };
        edge_cost(g, e, dir, p).map(|c| c.cost * pen)
    };
    let step = |edge, from_t, to_t| RouteStep { edge, from_t, to_t };
    let (s_fwd, s_bwd) = (
        full(start.edge, Direction::Forward)?,
        full(start.edge, Direction::Backward)?,
    );
    let (e_fwd, e_bwd) = (
        full(end.edge, Direction::Forward)?,
        full(end.edge, Direction::Backward)?,
    );

    // Best GOAL: (cost, node the final step leaves from or None for the direct partial, final step).
    let mut best: Option<(f64, Option<u32>, RouteStep)> = None;
    if start.edge == end.edge {
        let c = if end.t >= start.t { s_fwd } else { s_bwd };
        let c = c * (end.t - start.t).abs() as f64;
        best = Some((c, None, step(start.edge, start.t, end.t)));
    }

    let h_per_m = p.min_cost_per_m();
    let h = |n: u32| haversine_m(g.node(n), end.pos) * h_per_m;
    let (end_from, end_to) = (g.edge_from(end.edge), g.edge_to(end.edge));

    SCRATCH.with_borrow_mut(|s| {
        let n = g.node_count();
        if s.stamp.len() < n {
            s.stamp.resize(n, 0);
            s.dist.resize(n, 0.0);
            s.pred.resize(n, (step(0, 0.0, 0.0), false));
        }
        s.gen = s.gen.wrapping_add(1);
        if s.gen == 0 {
            s.stamp.fill(0);
            s.gen = 1;
        }
        let gen = s.gen;

        let mut heap = BinaryHeap::new();
        let relax = |s: &mut Scratch, heap: &mut BinaryHeap<_>, v: u32, d: f64, pr| {
            let i = v as usize;
            if s.stamp[i] != gen || d < s.dist[i] {
                s.stamp[i] = gen;
                s.dist[i] = d;
                s.pred[i] = pr;
                heap.push(Reverse((key(d + h(v)), key(d), v)));
            }
        };
        let (a, b) = (g.edge_from(start.edge), g.edge_to(start.edge));
        let t = start.t as f64;
        relax(
            s,
            &mut heap,
            b,
            s_fwd * (1.0 - t),
            (step(start.edge, start.t, 1.0), true),
        );
        relax(
            s,
            &mut heap,
            a,
            s_bwd * t,
            (step(start.edge, start.t, 0.0), true),
        );

        while let Some(Reverse((f, d, u))) = heap.pop() {
            if best.is_some_and(|(c, _, _)| f >= key(c)) {
                break;
            }
            let d = f64::from_bits(d);
            if d > s.dist[u as usize] {
                continue;
            }
            // Virtual GOAL from either endpoint of the end edge.
            for (node, c, from_t) in [
                (end_from, e_fwd * end.t as f64, 0.0),
                (end_to, e_bwd * (1.0 - end.t as f64), 1.0),
            ] {
                if u == node && best.is_none_or(|(b, _, _)| d + c < b) {
                    best = Some((d + c, Some(u), step(end.edge, from_t, end.t)));
                }
            }
            for (e, dir) in g.neighbors(u) {
                let Some(c) = full(e, dir) else { continue };
                let (v, st) = match dir {
                    Direction::Forward => (g.edge_to(e), step(e, 0.0, 1.0)),
                    Direction::Backward => (g.edge_from(e), step(e, 1.0, 0.0)),
                };
                relax(s, &mut heap, v, d + c, (st, false));
            }
        }

        let (cost, mut node, last) = best?;
        let mut steps = vec![last];
        while let Some(v) = node {
            let (st, seed) = s.pred[v as usize];
            steps.push(st);
            node = (!seed).then(|| match st.direction() {
                Direction::Forward => g.edge_from(st.edge),
                Direction::Backward => g.edge_to(st.edge),
            });
        }
        steps.reverse();
        Some(assemble(g, steps, cost))
    })
}

/// Consecutive legs through 2+ snaps with an empty `Visited` (manual routing, out-and-back allowed).
/// Concatenates steps; merges a step into the previous one when it is the same edge, same direction,
/// and contiguous (prev.to_t == next.from_t). None if any leg fails or fewer than 2 snaps.
pub fn route_via(g: &Graph, p: &Profile, snaps: &[Snap]) -> Option<Route> {
    route_legs(g, p, snaps, &mut Visited::new(g.edge_count()), |_| false)
}

/// Like `route_via`, but after each leg the edges for which `mark` holds go into `visited`,
/// so later legs pay the repetition penalty on them (loop generation).
pub(crate) fn route_legs(
    g: &Graph,
    p: &Profile,
    snaps: &[Snap],
    visited: &mut Visited,
    mark: impl Fn(u32) -> bool,
) -> Option<Route> {
    if snaps.len() < 2 {
        return None;
    }
    let mut steps: Vec<RouteStep> = Vec::new();
    let mut cost = 0.0;
    for w in snaps.windows(2) {
        let leg = route(g, p, w[0], w[1], visited)?;
        cost += leg.cost;
        for st in &leg.steps {
            if mark(st.edge) {
                visited.insert(st.edge);
            }
        }
        for st in leg.steps {
            match steps.last_mut() {
                Some(prev)
                    if prev.edge == st.edge
                        && prev.direction() == st.direction()
                        && prev.to_t == st.from_t =>
                {
                    prev.to_t = st.to_t
                }
                _ => steps.push(st),
            }
        }
    }
    Some(assemble(g, steps, cost))
}

/// Cancels immediate out-and-backs (along an edge, then straight back along it), so a via point
/// on a dead end does not leave a spur in a generated loop. Stack-based, so multi-edge spurs
/// unwind fully. `cost` is kept from the input; distance, climb and descent are recomputed.
pub fn drop_backtracks(g: &Graph, r: Route) -> Route {
    let mut out: Vec<RouteStep> = Vec::with_capacity(r.steps.len());
    for st in r.steps {
        match out.last_mut() {
            Some(prev)
                if prev.edge == st.edge
                    && prev.direction() != st.direction()
                    && prev.to_t == st.from_t =>
            {
                prev.to_t = st.to_t;
                if prev.from_t == prev.to_t {
                    out.pop();
                }
            }
            _ => out.push(st),
        }
    }
    assemble(g, out, r.cost)
}

/// Drops zero-length steps and totals distance, climb and descent.
fn assemble(g: &Graph, mut steps: Vec<RouteStep>, cost: f64) -> Route {
    steps.retain(|s| s.from_t != s.to_t);
    let (mut distance_m, mut climb_m, mut descent_m) = (0.0, 0.0, 0.0);
    for s in &steps {
        distance_m += step_len(g, s);
        let (up, down) = if s.fraction() >= 1.0 {
            let a = g.edge_attrs(s.edge);
            (a.gain as f64, a.loss as f64)
        } else {
            gain_loss(&slice(g, s).iter().map(|v| v.2).collect::<Vec<_>>())
        };
        let (up, down) = match s.direction() {
            // Sliced geometry is already in travel order; stored gain/loss are forward.
            Direction::Backward if s.fraction() >= 1.0 => (down, up),
            _ => (up, down),
        };
        climb_m += up;
        descent_m += down;
    }
    Route {
        steps,
        distance_m,
        climb_m,
        descent_m,
        cost,
    }
}

fn step_len(g: &Graph, s: &RouteStep) -> f64 {
    s.fraction() * polyline_length_m(g.edge_geometry(s.edge))
}

/// The step's geometry between from_t and to_t (haversine-length fractions, as in snapping),
/// in travel order.
fn slice(g: &Graph, s: &RouteStep) -> Vec<(f64, f64, f32)> {
    let pts: Vec<_> = g.edge_geometry(s.edge).collect();
    let total = polyline_length_m(pts.iter().copied());
    let (lo, hi) = if s.from_t <= s.to_t {
        (s.from_t, s.to_t)
    } else {
        (s.to_t, s.from_t)
    };
    let (lo, hi) = (lo as f64 * total, hi as f64 * total);
    let mut out = Vec::new();
    let mut acc = 0.0;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let l = haversine_m(LatLon { lat: a.0, lon: a.1 }, LatLon { lat: b.0, lon: b.1 });
        let lerp = |x: f64| {
            let t = if l > 0.0 {
                ((x - acc) / l).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (
                a.0 + (b.0 - a.0) * t,
                a.1 + (b.1 - a.1) * t,
                a.2 + (b.2 - a.2) * t as f32,
            )
        };
        if out.is_empty() && lo <= acc + l {
            out.push(lerp(lo));
        }
        if !out.is_empty() {
            if hi <= acc + l {
                out.push(lerp(hi));
                break;
            }
            out.push(b);
        }
        acc += l;
    }
    if out.is_empty() {
        // Degenerate (single-vertex or zero-length) geometry.
        out.extend(pts.first());
    }
    if s.direction() == Direction::Backward {
        out.reverse();
    }
    out
}

/// Sliced (lat, lon, z) polyline in travel order: each step's geometry between from_t and to_t
/// (interpolating the cut points, including z), reversed for backward steps; the duplicated
/// vertex where consecutive steps meet appears once.
pub fn route_geometry(g: &Graph, r: &Route) -> Vec<(f64, f64, f32)> {
    let mut out: Vec<(f64, f64, f32)> = Vec::new();
    for s in &r.steps {
        let pts = slice(g, s);
        let skip = usize::from(!out.is_empty());
        out.extend(pts.into_iter().skip(skip));
    }
    out
}

/// Cost per metre (incl. exclusion penalty) per `route_geometry` vertex: the cost of the step
/// leaving that vertex, so the last vertex repeats the last step's. For colouring the line.
pub fn route_cost_per_m(g: &Graph, r: &Route, p: &Profile) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for s in &r.steps {
        let c = edge_cost(g, s.edge, s.direction(), p).map_or(0.0, |c| c.per_m);
        let n = slice(g, s).len();
        if let Some(last) = out.last_mut() {
            *last = c;
            out.extend(std::iter::repeat_n(c, n.saturating_sub(1)));
        } else {
            out.extend(std::iter::repeat_n(c, n));
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RouteStats {
    pub distance_m: f64,
    pub climb_m: f64,
    pub descent_m: f64,
    /// Indexed by `Surface as u8`.
    pub surface_m: [f64; 8],
    /// Travel direction (backward steps use reversed bands).
    pub grade_m: [f64; 9],
    /// Index = sac 0..=6.
    pub sac_m: [f64; 7],
    pub official_m: f64,
    /// official_m / distance (0 if distance 0).
    pub official_share: f64,
    /// Σ length * forest_pct/100.
    pub forest_m: f64,
    /// Index = urban class 0..=2.
    pub urban_m: [f64; 3],
    pub missing_tags_m: f64,
    pub surface_inferred_m: f64,
    /// Length-weighted mean of edge_factors under the given profile weights.
    pub breakdown: Breakdown,
    /// Σ len * edge_factors(.., &Weights::default()).product() / distance.
    pub mean_factor_default: f64,
    /// cost::score(mean_factor_default).
    pub score: f32,
}

pub fn route_stats(g: &Graph, r: &Route, p: &Profile) -> RouteStats {
    let mut st = RouteStats {
        distance_m: r.distance_m,
        climb_m: r.climb_m,
        descent_m: r.descent_m,
        surface_m: [0.0; 8],
        grade_m: [0.0; 9],
        sac_m: [0.0; 7],
        official_m: 0.0,
        official_share: 0.0,
        forest_m: 0.0,
        urban_m: [0.0; 3],
        missing_tags_m: 0.0,
        surface_inferred_m: 0.0,
        breakdown: Breakdown::default(),
        mean_factor_default: 0.0,
        score: 0.0,
    };
    let mut bd = [0.0f64; 6];
    for s in &r.steps {
        let len = step_len(g, s);
        let a = g.edge_attrs(s.edge);
        let dir = s.direction();
        st.surface_m[(a.surface as usize).min(7)] += len;
        let sum: u32 = a.grade.iter().map(|&v| v as u32).sum();
        if sum == 0 {
            st.grade_m[FLAT_BAND] += len;
        } else {
            for (i, &share) in a.grade.iter().enumerate() {
                let band = match dir {
                    Direction::Forward => i,
                    Direction::Backward => GRADE_BANDS - 1 - i,
                };
                st.grade_m[band] += len * share as f64 / sum as f64;
            }
        }
        st.sac_m[(a.sac as usize).min(6)] += len;
        if a.official_category > 0 {
            st.official_m += len;
        }
        st.forest_m += len * a.forest_pct.min(100) as f64 / 100.0;
        st.urban_m[(a.urban as usize).min(2)] += len;
        if a.flags & flags::MISSING_TAGS != 0 {
            st.missing_tags_m += len;
        }
        if a.flags & flags::SURFACE_INFERRED != 0 {
            st.surface_inferred_m += len;
        }
        let f = edge_factors(g, s.edge, dir, &p.weights);
        for (acc, v) in bd
            .iter_mut()
            .zip([f.slope, f.surface, f.tech, f.official, f.forest, f.urban])
        {
            *acc += len * v as f64;
        }
        st.mean_factor_default += len * edge_factors(g, s.edge, dir, &Weights::default()).product();
    }
    let d = r.distance_m;
    if d > 0.0 {
        st.official_share = st.official_m / d;
        st.mean_factor_default /= d;
        let m = |i: usize| (bd[i] / d) as f32;
        st.breakdown = Breakdown {
            slope: m(0),
            surface: m(1),
            tech: m(2),
            official: m(3),
            forest: m(4),
            urban: m(5),
        };
    }
    st.score = score(st.mean_factor_default);
    st
}
