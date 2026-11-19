//! Offline importer: builds `graph.bin` from OSM, the DEM, the official trail
//! network and OSM land cover.

pub mod alti3d;
pub mod ch1903;
pub mod dem;
pub mod landcover;
pub mod official;
pub mod osm;

use rstar::{PointDistance, RTreeObject, AABB};

use crate::geodesy::closest_on_segment;
use crate::graph::Bbox;
use ch1903::wgs84_to_lv95;

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("import I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("OSM XML: {0}")]
    Xml(String),
    #[error("OSM PBF: {0}")]
    Pbf(String),
    #[error("bad input: {0}")]
    Format(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    pub name: &'static str,
    pub bbox: Bbox,
}

/// "demo" → Wattwil–Ziegelbrücke (D34); "ch" → all of Switzerland.
pub fn region(name: &str) -> Option<Region> {
    let (name, min_lat, max_lat, min_lon, max_lon) = match name {
        "demo" => ("demo", 47.10, 47.34, 8.95, 9.22),
        "ch" => ("ch", 45.8, 47.9, 5.9, 10.6),
        _ => return None,
    };
    Some(Region {
        name,
        bbox: Bbox {
            min_lat,
            min_lon,
            max_lat,
            max_lon,
        },
    })
}

/// An LV95 line segment tagged with the index of the line or polygon it belongs to.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Seg {
    pub a: [f64; 2],
    pub b: [f64; 2],
    pub id: u32,
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

/// LV95 points every `spacing` meters or less along a (lat, lon, z) geometry,
/// both endpoints included.
pub(crate) fn sample_lv95(geom: &[(f64, f64, f32)], spacing: f64) -> Vec<[f64; 2]> {
    let lv: Vec<[f64; 2]> = geom
        .iter()
        .map(|&(la, lo, _)| {
            let (e, n) = wgs84_to_lv95(la, lo);
            [e, n]
        })
        .collect();
    let mut out: Vec<[f64; 2]> = lv.first().copied().into_iter().collect();
    for w in lv.windows(2) {
        let (a, b) = (w[0], w[1]);
        let k = ((b[0] - a[0]).hypot(b[1] - a[1]) / spacing).ceil().max(1.0) as usize;
        for j in 1..=k {
            let t = j as f64 / k as f64;
            out.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
        }
    }
    out
}
