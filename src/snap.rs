//! Snapping a map point to the nearest edge via a fixed grid of geometry
//! segments, built by the importer and stored in `graph.bin` (D50).

use std::ops::Range;

use crate::geodesy::{closest_on_segment, haversine_m, LocalProjection};
use crate::graph::{Bbox, Graph, LatLon};

pub const SNAP_LIMIT_M: f32 = 150.0;

/// Grid cell edge in projected meters. CH: ~17 M segments over ~1.3 M cells.
const CELL_M: f64 = 250.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snap {
    pub edge: u32,
    /// Fraction along the edge geometry, 0 at `from`, 1 at `to`.
    pub t: f32,
    pub pos: LatLon,
    pub dist_m: f32,
}

/// Cells of `CELL_M` over the graph bbox in the local projection at its mid
/// latitude. Derived from the stored f32 bbox, so writer and reader agree.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SnapGrid {
    proj: LocalProjection,
    origin: [f64; 2],
    nx: usize,
    ny: usize,
}

impl SnapGrid {
    pub(crate) fn new(bb: Bbox) -> SnapGrid {
        let proj = LocalProjection::new((bb.min_lat + bb.max_lat) / 2.0);
        let origin = proj.project(LatLon {
            lat: bb.min_lat,
            lon: bb.min_lon,
        });
        let far = proj.project(LatLon {
            lat: bb.max_lat,
            lon: bb.max_lon,
        });
        let n = |d: f64| (d / CELL_M) as usize + 1;
        SnapGrid {
            proj,
            origin,
            nx: n(far[0] - origin[0]),
            ny: n(far[1] - origin[1]),
        }
    }

    pub(crate) fn cells(&self) -> usize {
        self.nx * self.ny
    }

    /// Projects a stored (f32) vertex.
    pub(crate) fn xy(&self, lat: f32, lon: f32) -> [f64; 2] {
        self.proj.project(LatLon {
            lat: lat as f64,
            lon: lon as f64,
        })
    }

    /// Cell ids overlapping the projected box `lo..=hi`, clipped to the grid.
    pub(crate) fn cells_in(&self, lo: [f64; 2], hi: [f64; 2]) -> impl Iterator<Item = usize> {
        let span = |axis: usize, n: usize| -> Range<usize> {
            let a = ((lo[axis] - self.origin[axis]) / CELL_M).floor().max(0.0);
            let b = ((hi[axis] - self.origin[axis]) / CELL_M)
                .floor()
                .min(n as f64 - 1.0);
            if a > b {
                0..0
            } else {
                a as usize..b as usize + 1
            }
        };
        let (xs, nx) = (span(0, self.nx), self.nx);
        span(1, self.ny).flat_map(move |j| xs.clone().map(move |i| j * nx + i))
    }
}

/// CSR over cells: `segs[first[c]..first[c + 1]]` are the global geometry
/// indices of the first vertex of each segment whose bbox touches cell `c`.
pub(crate) fn build_snap_index<'a>(
    grid: &SnapGrid,
    edges: impl Iterator<Item = &'a [(f64, f64, f32)]>,
) -> (Vec<u32>, Vec<u32>) {
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    let mut gi = 0u32;
    for geom in edges {
        let pts: Vec<[f64; 2]> = geom
            .iter()
            .map(|v| grid.xy(v.0 as f32, v.1 as f32))
            .collect();
        for (k, w) in pts.windows(2).enumerate() {
            let lo = [w[0][0].min(w[1][0]), w[0][1].min(w[1][1])];
            let hi = [w[0][0].max(w[1][0]), w[0][1].max(w[1][1])];
            pairs.extend(grid.cells_in(lo, hi).map(|c| (c as u32, gi + k as u32)));
        }
        gi += geom.len() as u32;
    }
    pairs.sort_unstable();
    let mut first = vec![0u32; grid.cells() + 1];
    for &(c, _) in &pairs {
        first[c as usize + 1] += 1;
    }
    for c in 0..grid.cells() {
        first[c + 1] += first[c];
    }
    (first, pairs.into_iter().map(|(_, s)| s).collect())
}

impl Graph {
    pub fn snap(&self, p: LatLon) -> Option<Snap> {
        self.snap_within(p, SNAP_LIMIT_M)
    }

    pub fn snap_within(&self, p: LatLon, max_m: f32) -> Option<Snap> {
        self.snap_filtered(p, max_m, |_| true)
    }

    /// Like `snap_filtered`, but on the main network (under `max_sac`) when it is within
    /// `max_m`, so a start beside station stairs or a courtyard path doesn't land on an island.
    /// Else any component.
    pub fn snap_start(
        &self,
        p: LatLon,
        max_m: f32,
        max_sac: u8,
        accept: impl Fn(u32) -> bool,
    ) -> Option<Snap> {
        let main = self.main_component(max_sac);
        self.snap_filtered(p, max_m, |e| {
            accept(e) && self.edge_component(e, max_sac) == main
        })
        .or_else(|| self.snap_filtered(p, max_m, accept))
    }

    /// Nearest edge within max_m for which `accept(edge)` is true (e.g. traversable for a profile).
    pub fn snap_filtered(
        &self,
        p: LatLon,
        max_m: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Option<Snap> {
        let grid = self.snap_grid();
        let (first, segs) = self.snap_cells();
        let q = grid.proj.project(p);
        // The projection drifts from true meters away from its reference latitude.
        let r = max_m as f64 * 1.05;
        let mut best: Option<(f64, u32, u32)> = None; // (dist², geometry index, edge)
        for c in grid.cells_in([q[0] - r, q[1] - r], [q[0] + r, q[1] + r]) {
            for &gi in &segs[first[c] as usize..first[c + 1] as usize] {
                let (c, _) = closest_on_segment(q, self.geom_xy(gi), self.geom_xy(gi + 1));
                let d2 = (c[0] - q[0]).powi(2) + (c[1] - q[1]).powi(2);
                if d2 > r * r || best.is_some_and(|(b, _, _)| d2 >= b) {
                    continue;
                }
                let (e, _) = self.geom_edge(gi);
                if accept(e) {
                    best = Some((d2, gi, e));
                }
            }
        }
        let (_, gi, edge) = best?;
        let (_, idx) = self.geom_edge(gi);

        let (c, st) = closest_on_segment(q, self.geom_xy(gi), self.geom_xy(gi + 1));
        let pos = grid.proj.unproject(c);
        let dist = haversine_m(p, pos);
        if dist > max_m as f64 {
            return None;
        }

        // Haversine length along the geometry up to the snapped point, over the total.
        let mut prev: Option<LatLon> = None;
        let (mut before, mut total) = (0.0, 0.0);
        for (i, (lat, lon, _)) in self.edge_geometry(edge).enumerate() {
            let cur = LatLon { lat, lon };
            if let Some(pv) = prev {
                let l = haversine_m(pv, cur);
                // Segment `idx` joins vertices idx and idx + 1.
                if i - 1 < idx as usize {
                    before += l;
                } else if i - 1 == idx as usize {
                    before += l * st;
                }
                total += l;
            }
            prev = Some(cur);
        }
        let t = if total > 0.0 {
            (before / total).clamp(0.0, 1.0)
        } else {
            0.0
        };
        Some(Snap {
            edge,
            t: t as f32,
            pos,
            dist_m: dist as f32,
        })
    }
}
