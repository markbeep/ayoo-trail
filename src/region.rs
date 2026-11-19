//! Per-request search region for loop generation (Q25 steps 1–2).
//!
//! A request runs hundreds of searches around one start. The region holds the graph within a
//! radius of the start, with local ids and edge costs worked out once (a flat part plus a
//! climb-pressure part, so one region serves every pressure). Per pressure it keeps a
//! shortest-path tree from the start and one into it: the first leg of a loop is read off the
//! first tree, and both trees give later legs a much tighter A* bound (ALT with the start as
//! landmark). Legs come out as `route::route` would route them on the whole graph.

use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};

use crate::cost::{climb_discount, edge_cost_split, FactorTable, Profile};
use crate::geodesy::LocalProjection;
use crate::graph::{Direction, Graph};
use crate::route::{assemble, Route, RouteStep, Visited, REPEAT_PENALTY};
use crate::snap::Snap;

/// One directed hop out of a region node: the far node, the edge, and both directions'
/// `(flat, graded)` cost.
#[derive(Clone, Copy)]
struct Hop {
    to: u32,
    edge: u32,
    forward: bool,
    out: (f64, f64),
    back: (f64, f64),
}

/// Distances under one climb pressure.
struct Trees {
    /// Cost from the start snap to each node.
    from: Vec<f64>,
    /// Step that reached the node in `from`, and whether it is the start partial.
    pred: Vec<(RouteStep, bool)>,
    /// Cost from each node to the start snap.
    to: Vec<f64>,
}

pub(crate) struct Region<'g> {
    g: &'g Graph,
    profile: Profile,
    start: Snap,
    /// Global node id → local id + 1, 0 = outside. Zero-allocated, so only touched pages cost.
    local: Vec<u32>,
    xy: Vec<[f64; 2]>,
    first: Vec<u32>,
    hops: Vec<Hop>,
    proj: LocalProjection,
    trees: Mutex<HashMap<u32, Arc<OnceLock<Trees>>>>,
}

#[derive(Default)]
struct Scratch {
    gen: u32,
    stamp: Vec<u32>,
    dist: Vec<f64>,
    pred: Vec<(RouteStep, bool)>,
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

fn key(x: f64) -> u64 {
    x.max(0.0).to_bits()
}

fn at(c: (f64, f64), discount: f64) -> f64 {
    c.0 + discount * c.1
}

fn step(edge: u32, from_t: f32, to_t: f32) -> RouteStep {
    RouteStep { edge, from_t, to_t }
}

impl<'g> Region<'g> {
    /// Nodes within `radius_m` (straight line) of the start, plus the start edge's ends.
    /// `p` gives the hard filters and weights; its climb pressure is ignored.
    pub(crate) fn new(g: &'g Graph, p: &Profile, start: Snap, radius_m: f64) -> Region<'g> {
        let mut p0 = p.clone();
        p0.weights.climb_pressure = 0.0;
        let table = FactorTable::new(&p0.weights);
        let proj = LocalProjection::new(start.pos.lat);
        let o = proj.project(start.pos);
        let mut local = vec![0u32; g.node_count()];
        let mut global: Vec<u32> = Vec::new();
        let mut xy = Vec::new();
        let mut add = |v: u32, local: &mut Vec<u32>, global: &mut Vec<u32>| -> u32 {
            if local[v as usize] == 0 {
                global.push(v);
                xy.push(proj.project(g.node(v)));
                local[v as usize] = global.len() as u32;
            }
            local[v as usize] - 1
        };
        add(g.edge_from(start.edge), &mut local, &mut global);
        add(g.edge_to(start.edge), &mut local, &mut global);
        let r2 = radius_m * radius_m;
        let (mut first, mut hops) = (Vec::new(), Vec::new());
        // Breadth-first: nodes get ids in discovery order, so neighbors sit close in memory.
        let mut i = 0;
        while i < global.len() {
            let u = global[i];
            first.push(hops.len() as u32);
            for (e, dir) in g.neighbors(u) {
                let h = g.hot_edge(e);
                let v = if dir == Direction::Forward {
                    h.to
                } else {
                    h.from
                };
                if local[v as usize] == 0 {
                    let q = proj.project(g.node(v));
                    if (q[0] - o[0]).powi(2) + (q[1] - o[1]).powi(2) > r2 {
                        continue;
                    }
                }
                let Some([f, b]) = edge_cost_split(g, e, &p0, &table) else {
                    continue;
                };
                let (out, back) = if dir == Direction::Forward {
                    (f, b)
                } else {
                    (b, f)
                };
                let to = add(v, &mut local, &mut global);
                hops.push(Hop {
                    to,
                    edge: e,
                    forward: dir == Direction::Forward,
                    out,
                    back,
                });
            }
            i += 1;
        }
        first.push(hops.len() as u32);
        Region {
            g,
            profile: p0,
            start,
            local,
            xy,
            first,
            hops,
            proj,
            trees: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn node_count(&self) -> usize {
        self.xy.len()
    }

    fn local_of(&self, v: u32) -> Option<u32> {
        self.local[v as usize].checked_sub(1)
    }

    fn hops(&self, u: u32) -> &[Hop] {
        &self.hops[self.first[u as usize] as usize..self.first[u as usize + 1] as usize]
    }

    /// Both directions' cost of a snapped edge; None if it isn't traversable.
    fn edge_costs(&self, e: u32) -> Option<((f64, f64), (f64, f64))> {
        let t = FactorTable::new(&self.profile.weights);
        let [f, b] = edge_cost_split(self.g, e, &self.profile, &t)?;
        Some((f, b))
    }

    /// Local ids of the snapped edge's ends, if both are in the region.
    fn ends(&self, s: &Snap) -> Option<(u32, u32)> {
        Some((
            self.local_of(self.g.edge_from(s.edge))?,
            self.local_of(self.g.edge_to(s.edge))?,
        ))
    }

    fn trees(&self, pressure: f32) -> Arc<OnceLock<Trees>> {
        let cell = self
            .trees
            .lock()
            .expect("trees lock")
            .entry(pressure.to_bits())
            .or_default()
            .clone();
        cell.get_or_init(|| self.build_trees(climb_discount(pressure)));
        cell
    }

    fn build_trees(&self, d: f64) -> Trees {
        let n = self.node_count();
        let s = self.start;
        let (a, b) = self.ends(&s).expect("start edge is in the region");
        let (fwd, bwd) = self.edge_costs(s.edge).expect("start edge is traversable");
        let (fwd, bwd) = (at(fwd, d), at(bwd, d));
        let t = s.t as f64;

        let mut from = vec![f64::INFINITY; n];
        let mut pred = vec![(step(0, 0.0, 0.0), false); n];
        let mut heap = BinaryHeap::new();
        for (v, c, st) in [
            (b, fwd * (1.0 - t), step(s.edge, s.t, 1.0)),
            (a, bwd * t, step(s.edge, s.t, 0.0)),
        ] {
            if c < from[v as usize] {
                from[v as usize] = c;
                pred[v as usize] = (st, true);
                heap.push(Reverse((key(c), v)));
            }
        }
        while let Some(Reverse((k, u))) = heap.pop() {
            let du = f64::from_bits(k);
            if du > from[u as usize] {
                continue;
            }
            for h in self.hops(u) {
                let c = du + at(h.out, d);
                if c < from[h.to as usize] {
                    from[h.to as usize] = c;
                    pred[h.to as usize] = (
                        if h.forward {
                            step(h.edge, 0.0, 1.0)
                        } else {
                            step(h.edge, 1.0, 0.0)
                        },
                        false,
                    );
                    heap.push(Reverse((key(c), h.to)));
                }
            }
        }

        // Into the start: from `a` forward along the start edge to t, from `b` backward.
        let mut to = vec![f64::INFINITY; n];
        for (v, c) in [(a, fwd * t), (b, bwd * (1.0 - t))] {
            if c < to[v as usize] {
                to[v as usize] = c;
                heap.push(Reverse((key(c), v)));
            }
        }
        while let Some(Reverse((k, u))) = heap.pop() {
            let du = f64::from_bits(k);
            if du > to[u as usize] {
                continue;
            }
            for h in self.hops(u) {
                let c = du + at(h.back, d);
                if c < to[h.to as usize] {
                    to[h.to as usize] = c;
                    heap.push(Reverse((key(c), h.to)));
                }
            }
        }
        Trees { from, pred, to }
    }

    /// One leg as `route::route` would route it, or None to leave it to the full graph (an
    /// end outside the region, or no path inside it).
    pub(crate) fn leg(
        &self,
        p: &Profile,
        from: Snap,
        end: Snap,
        visited: &Visited,
    ) -> Option<Route> {
        let (sa, sb) = self.ends(&from)?;
        let (ea, eb) = self.ends(&end)?;
        let d = climb_discount(p.weights.climb_pressure);
        let cell = self.trees(p.weights.climb_pressure);
        let tr = cell.get().expect("built");
        let (s_c, e_c) = (self.edge_costs(from.edge)?, self.edge_costs(end.edge)?);
        let pen = |e: u32| {
            if visited.contains(e) {
                REPEAT_PENALTY
            } else {
                1.0
            }
        };
        let (s_fwd, s_bwd) = (at(s_c.0, d) * pen(from.edge), at(s_c.1, d) * pen(from.edge));
        let (e_fwd, e_bwd) = (at(e_c.0, d) * pen(end.edge), at(e_c.1, d) * pen(end.edge));
        // Exact distances start → end and end → start (no repeat penalty), for the bound.
        let start = self.start;
        let st_c = self.edge_costs(start.edge)?;
        let direct = |a: &Snap, b: &Snap| {
            (a.edge == b.edge).then(|| {
                let c = if b.t >= a.t { st_c.0 } else { st_c.1 };
                at(c, d) * (b.t - a.t).abs() as f64
            })
        };
        let (ef, eb_) = (at(e_c.0, d), at(e_c.1, d));
        let et = end.t as f64;
        let f_end = (tr.from[ea as usize] + ef * et)
            .min(tr.from[eb as usize] + eb_ * (1.0 - et))
            .min(direct(&start, &end).unwrap_or(f64::INFINITY));
        let t_end = (tr.to[eb as usize] + ef * (1.0 - et))
            .min(tr.to[ea as usize] + eb_ * et)
            .min(direct(&end, &start).unwrap_or(f64::INFINITY));
        if !f_end.is_finite() || !t_end.is_finite() {
            return None;
        }

        // Best GOAL: (cost, local node the final step leaves from or None, final step).
        let mut best: Option<(f64, Option<u32>, RouteStep)> = None;
        if from.edge == end.edge {
            let c = if end.t >= from.t { s_fwd } else { s_bwd };
            best = Some((
                c * (end.t - from.t).abs() as f64,
                None,
                step(from.edge, from.t, end.t),
            ));
        }
        let goals = [
            (ea, e_fwd * et, step(end.edge, 0.0, end.t)),
            (eb, e_bwd * (1.0 - et), step(end.edge, 1.0, end.t)),
        ];

        // The first leg leaves the start with nothing visited: read it off the tree.
        if from == start && visited.is_empty() {
            for (node, c, st) in goals {
                let total = tr.from[node as usize] + c;
                if best.is_none_or(|(b, _, _)| total < b) {
                    best = Some((total, Some(node), st));
                }
            }
            let (cost, node, last) = best?;
            return Some(self.unwind(cost, node, last, |v| tr.pred[v as usize]));
        }

        let h_per_m = p.min_cost_per_m() * 0.99;
        let goal = self.proj.project(end.pos);
        let h = |v: u32| {
            let q = self.xy[v as usize];
            let line = (q[0] - goal[0]).hypot(q[1] - goal[1]) * h_per_m;
            // The start as landmark: d(v, end) ≥ d(start, end) − d(start, v) and
            // ≥ d(v, start) − d(end, start). Penalties only raise costs, so both hold.
            line.max(f_end - tr.from[v as usize])
                .max(tr.to[v as usize] - t_end)
        };

        SCRATCH.with_borrow_mut(|s| {
            let n = self.node_count();
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
            let relax = |s: &mut Scratch, heap: &mut BinaryHeap<_>, v: u32, c: f64, pr| {
                let i = v as usize;
                if s.stamp[i] != gen || c < s.dist[i] {
                    let hv = h(v);
                    if hv.is_infinite() {
                        // Can't reach the start, so can't reach `end` either.
                        return;
                    }
                    s.stamp[i] = gen;
                    s.dist[i] = c;
                    s.pred[i] = pr;
                    heap.push(Reverse((key(c + hv), key(c), v)));
                }
            };
            let t = from.t as f64;
            relax(
                s,
                &mut heap,
                sb,
                s_fwd * (1.0 - t),
                (step(from.edge, from.t, 1.0), true),
            );
            relax(
                s,
                &mut heap,
                sa,
                s_bwd * t,
                (step(from.edge, from.t, 0.0), true),
            );
            while let Some(Reverse((f, du, u))) = heap.pop() {
                if best.is_some_and(|(c, _, _)| f >= key(c)) {
                    break;
                }
                let du = f64::from_bits(du);
                if du > s.dist[u as usize] {
                    continue;
                }
                for (node, c, st) in goals {
                    if u == node && best.is_none_or(|(b, _, _)| du + c < b) {
                        best = Some((du + c, Some(u), st));
                    }
                }
                for hp in self.hops(u) {
                    let c = du + at(hp.out, d) * pen(hp.edge);
                    let st = if hp.forward {
                        step(hp.edge, 0.0, 1.0)
                    } else {
                        step(hp.edge, 1.0, 0.0)
                    };
                    relax(s, &mut heap, hp.to, c, (st, false));
                }
            }
            let (cost, node, last) = best?;
            Some(self.unwind(cost, node, last, |v| s.pred[v as usize]))
        })
    }

    /// Follows predecessor steps back from `node` to the leg's first step.
    fn unwind(
        &self,
        cost: f64,
        mut node: Option<u32>,
        last: RouteStep,
        pred: impl Fn(u32) -> (RouteStep, bool),
    ) -> Route {
        let g = self.g;
        let mut steps = vec![last];
        while let Some(v) = node {
            let (st, seed) = pred(v);
            steps.push(st);
            node = if seed {
                None
            } else {
                let prev = match st.direction() {
                    Direction::Forward => g.edge_from(st.edge),
                    Direction::Backward => g.edge_to(st.edge),
                };
                self.local_of(prev)
            };
        }
        steps.reverse();
        assemble(g, steps, cost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{EdgeAttrs, GraphWriter, LatLon, Surface};
    use crate::route::route;

    /// 12×12 grid, 100 m spacing, with mixed surfaces and grades so costs differ by
    /// direction and climb pressure.
    fn graph() -> Graph {
        let at = |i: usize, j: usize| LatLon {
            lat: 46.0 + j as f64 * 100.0 / 111_195.0,
            lon: 7.0 + i as f64 * 100.0 / (111_195.0 * 46f64.to_radians().cos()),
        };
        let mut w = GraphWriter::new(0);
        let ids: Vec<Vec<u32>> = (0..12)
            .map(|i| (0..12).map(|j| w.add_node(at(i, j), 500.0)).collect())
            .collect();
        let mut k = 0usize;
        let mut link = |w: &mut GraphWriter, a: (usize, usize), b: (usize, usize)| {
            k += 1;
            let mut grade = [0u8; 9];
            grade[k % 9] = 200;
            grade[(k * 7) % 9] += 55;
            let attrs = EdgeAttrs {
                length_m: 100.0,
                surface: [Surface::Gravel, Surface::Asphalt, Surface::Dirt][k % 3],
                sac: 1 + (k % 2) as u8,
                grade,
                forest_pct: (k * 13 % 101) as u8,
                ..Default::default()
            };
            let (p, q) = (at(a.0, a.1), at(b.0, b.1));
            w.add_edge(
                ids[a.0][a.1],
                ids[b.0][b.1],
                attrs,
                &[(p.lat, p.lon, 500.0), (q.lat, q.lon, 500.0)],
            );
        };
        for i in 0..12 {
            for j in 0..12 {
                if i + 1 < 12 {
                    link(&mut w, (i, j), (i + 1, j));
                }
                if j + 1 < 12 {
                    link(&mut w, (i, j), (i, j + 1));
                }
            }
        }
        Graph::from_bytes(w.finish()).unwrap()
    }

    #[test]
    fn legs_match_the_full_graph_search() {
        let g = graph();
        let pt = |x: f64, y: f64| {
            g.snap(LatLon {
                lat: 46.0 + y / 111_195.0,
                lon: 7.0 + x / (111_195.0 * 46f64.to_radians().cos()),
            })
            .unwrap()
        };
        let start = pt(530.0, 470.0);
        let region = Region::new(&g, &Profile::default(), start, 5000.0);
        let ends = [
            pt(120.0, 900.0),
            pt(1000.0, 1040.0),
            pt(860.0, 130.0),
            start,
        ];
        for pressure in [0.0, 0.4, 0.8] {
            let mut p = Profile::default();
            p.weights.climb_pressure = pressure;
            let mut visited = Visited::new(g.edge_count());
            let mut from = start;
            for &end in &ends {
                let full = route(&g, &p, from, end, &visited).unwrap();
                let local = region.leg(&p, from, end, &visited).unwrap();
                // The full search sums the slope factor in f32.
                assert!((full.cost - local.cost).abs() < 1e-6 * full.cost);
                assert_eq!(full.steps, local.steps);
                for s in &full.steps {
                    visited.insert(s.edge);
                }
                from = end;
            }
        }
    }
}
