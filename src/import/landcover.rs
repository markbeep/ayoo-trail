//! Land cover from the OSM polygons (forest share, urban class) and anchors
//! (peaks, viewpoints).

use geo::{GeodesicArea, Polygon};
use rayon::prelude::*;
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{RTree, AABB};

use super::ch1903::wgs84_to_lv95;
use super::osm::OsmExtras;
use super::{sample_lv95, Seg};
use crate::graph::{AnchorKind, GraphWriter};

const SAMPLE_SPACING_M: f64 = 25.0;
const URBAN_NEAR_M: f64 = 50.0;
/// Settlement areas smaller than this (single farmsteads) don't count as built-up (D77).
const URBAN_MIN_AREA_M2: f64 = 10_000.0;

/// Polygons in LV95: their bboxes and all their ring segments, each tagged
/// with the polygon index. Containment is a ray cast over the segments the
/// R-tree returns, so the cost per point does not grow with polygon size.
struct PolyIndex {
    boxes: RTree<GeomWithData<Rectangle<[f64; 2]>, u32>>,
    segs: RTree<Seg>,
}

impl PolyIndex {
    fn new(polys: &[Polygon<f64>]) -> PolyIndex {
        let (mut boxes, mut segs) = (Vec::new(), Vec::new());
        for (i, poly) in polys.iter().enumerate() {
            let (id, first) = (i as u32, segs.len());
            let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
            for ring in std::iter::once(poly.exterior()).chain(poly.interiors()) {
                let lv: Vec<[f64; 2]> = ring
                    .coords()
                    .map(|c| {
                        let (e, n) = wgs84_to_lv95(c.y, c.x);
                        [e, n]
                    })
                    .collect();
                for p in &lv {
                    lo = [lo[0].min(p[0]), lo[1].min(p[1])];
                    hi = [hi[0].max(p[0]), hi[1].max(p[1])];
                }
                segs.extend(lv.windows(2).map(|w| Seg {
                    a: w[0],
                    b: w[1],
                    id,
                }));
            }
            if segs.len() > first {
                boxes.push(GeomWithData::new(Rectangle::from_corners(lo, hi), id));
            }
        }
        PolyIndex {
            boxes: RTree::bulk_load(boxes),
            segs: RTree::bulk_load(segs),
        }
    }

    fn contains(&self, p: &[f64; 2]) -> bool {
        self.boxes.locate_all_at_point(p).any(|b| {
            let ray = AABB::from_corners(*p, [b.geom().upper()[0], p[1]]);
            let crossings = self
                .segs
                .locate_in_envelope_intersecting(&ray)
                .filter(|s| s.id == b.data)
                .filter(|s| {
                    let (a, c) = (s.a, s.b);
                    (a[1] > p[1]) != (c[1] > p[1])
                        && p[0] < a[0] + (p[1] - a[1]) * (c[0] - a[0]) / (c[1] - a[1])
                })
                .count();
            crossings % 2 == 1
        })
    }

    /// Any polygon boundary within `d` meters.
    fn near(&self, p: &[f64; 2], d: f64) -> bool {
        self.segs.locate_within_distance(*p, d * d).next().is_some()
    }
}

/// forest_pct = share (0..=100) of edge sample points (every ~25 m, endpoints included) inside a forest
/// polygon. urban = 2 if a sample is inside a settlement polygon of at least 1 ha, else 1 if any sample
/// is within 50 m of one, else 0 (max over samples). R-tree of polygon bboxes + geo containment; prepared geometry or
/// equivalent so a CH-wide run is minutes, not hours.
pub fn apply_landcover(w: &mut GraphWriter, extras: &OsmExtras) {
    let forest = PolyIndex::new(&extras.forest);
    let towns: Vec<Polygon<f64>> = extras
        .residential
        .iter()
        .filter(|p| p.geodesic_area_unsigned() >= URBAN_MIN_AREA_M2)
        .cloned()
        .collect();
    let urban = PolyIndex::new(&towns);
    let writer = &*w;
    let out: Vec<(u8, u8)> = (0..writer.edge_count() as u32)
        .into_par_iter()
        .map(|e| {
            let samples = sample_lv95(writer.edge_geometry(e), SAMPLE_SPACING_M);
            if samples.is_empty() {
                return (0, 0);
            }
            let inside = samples.iter().filter(|p| forest.contains(p)).count();
            let pct = (inside * 100 + samples.len() / 2) / samples.len();
            let class = if samples.iter().any(|p| urban.contains(p)) {
                2
            } else if samples.iter().any(|p| urban.near(p, URBAN_NEAR_M)) {
                1
            } else {
                0
            };
            (pct as u8, class)
        })
        .collect();
    for (e, (pct, class)) in out.into_iter().enumerate() {
        let a = w.edge_attrs_mut(e as u32);
        a.forest_pct = pct;
        a.urban = class;
    }
}

/// Peaks (weight = ele) and viewpoints (weight 1.0) inside the graph bbox as anchors.
pub fn collect_anchors(w: &mut GraphWriter, extras: &OsmExtras) {
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for n in 0..w.node_count() as u32 {
        let p = w.node_pos(n);
        lo = [lo[0].min(p.lat), lo[1].min(p.lon)];
        hi = [hi[0].max(p.lat), hi[1].max(p.lon)];
    }
    let inside = |p: &crate::graph::LatLon| {
        (lo[0]..=hi[0]).contains(&p.lat) && (lo[1]..=hi[1]).contains(&p.lon)
    };
    for &(pos, ele) in extras.peaks.iter().filter(|(p, _)| inside(p)) {
        w.add_anchor(pos, AnchorKind::Peak, ele);
    }
    for &pos in extras.viewpoints.iter().filter(|p| inside(p)) {
        w.add_anchor(pos, AnchorKind::Viewpoint, 1.0);
    }
}
