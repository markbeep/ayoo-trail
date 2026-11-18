//! Offline importer: builds `graph.bin` from OSM (and, in later stages, the DEM
//! and the official trail network).

pub mod osm;

use crate::graph::Bbox;

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
