//! Snapping a map point to the nearest edge via an R-tree of geometry segments.

use rstar::{PointDistance, RTree, RTreeObject, AABB};

use crate::geodesy::{closest_on_segment, haversine_m, LocalProjection};
use crate::graph::{Graph, LatLon};

pub const SNAP_LIMIT_M: f32 = 150.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snap {
    pub edge: u32,
    /// Fraction along the edge geometry, 0 at `from`, 1 at `to`.
    pub t: f32,
    pub pos: LatLon,
    pub dist_m: f32,
}

/// One geometry segment in projected metres.
#[derive(Debug, Clone, Copy)]
struct Seg {
    a: [f64; 2],
    b: [f64; 2],
    edge: u32,
    idx: u32,
}

impl RTreeObject for Seg {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(self.a, self.b)
    }
}

impl PointDistance for Seg {
    fn distance_2(&self, p: &[f64; 2]) -> f64 {
        let (c, _) = closest_on_segment(*p, self.a, self.b);
        (c[0] - p[0]).powi(2) + (c[1] - p[1]).powi(2)
    }
}

pub struct SnapIndex {
    proj: LocalProjection,
    tree: RTree<Seg>,
}

impl SnapIndex {
    pub fn build(g: &Graph) -> SnapIndex {
        let bb = g.bbox();
        let proj = LocalProjection::new((bb.min_lat + bb.max_lat) / 2.0);
        let mut segs = Vec::new();
        for e in 0..g.edge_count() as u32 {
            let pts: Vec<[f64; 2]> = g
                .edge_geometry(e)
                .map(|(lat, lon, _)| proj.project(LatLon { lat, lon }))
                .collect();
            for (i, w) in pts.windows(2).enumerate() {
                segs.push(Seg {
                    a: w[0],
                    b: w[1],
                    edge: e,
                    idx: i as u32,
                });
            }
        }
        SnapIndex {
            proj,
            tree: RTree::bulk_load(segs),
        }
    }
}

impl Graph {
    /// Build the index now (the server calls this at startup); otherwise built on first snap.
    pub fn warm_snap_index(&self) {
        self.snap_index.get_or_init(|| SnapIndex::build(self));
    }

    pub fn snap(&self, p: LatLon) -> Option<Snap> {
        self.snap_within(p, SNAP_LIMIT_M)
    }

    pub fn snap_within(&self, p: LatLon, max_m: f32) -> Option<Snap> {
        self.snap_filtered(p, max_m, |_| true)
    }

    /// Nearest edge within max_m for which `accept(edge)` is true (e.g. traversable for a profile).
    pub fn snap_filtered(
        &self,
        p: LatLon,
        max_m: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Option<Snap> {
        let index = self.snap_index.get_or_init(|| SnapIndex::build(self));
        let q = index.proj.project(p);
        let cutoff = (max_m as f64 * 1.05).powi(2);
        let (seg, _) = index
            .tree
            .nearest_neighbor_iter_with_distance_2(&q)
            .take_while(|(_, d2)| *d2 <= cutoff)
            .find(|(s, _)| accept(s.edge))?;

        let (c, st) = closest_on_segment(q, seg.a, seg.b);
        let pos = index.proj.unproject(c);
        let dist = haversine_m(p, pos);
        if dist > max_m as f64 {
            return None;
        }

        // Haversine length along the geometry up to the snapped point, over the total.
        let mut prev: Option<LatLon> = None;
        let (mut before, mut total) = (0.0, 0.0);
        for (i, (lat, lon, _)) in self.edge_geometry(seg.edge).enumerate() {
            let cur = LatLon { lat, lon };
            if let Some(pv) = prev {
                let l = haversine_m(pv, cur);
                // Segment `idx` joins vertices idx and idx + 1.
                if i - 1 < seg.idx as usize {
                    before += l;
                } else if i - 1 == seg.idx as usize {
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
            edge: seg.edge,
            t: t as f32,
            pos,
            dist_m: dist as f32,
        })
    }
}
