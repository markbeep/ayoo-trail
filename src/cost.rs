//! Edge cost model, routing profile and route score (design §8, D31, D32, D35).

use serde::{Deserialize, Serialize};

use crate::geodesy::{closest_on_segment, haversine_m, LocalProjection};
use crate::graph::{flags, Direction, Graph, LatLon, GRADE_BANDS};

/// Raw slope factor per grade band, steepest descent first.
pub const SLOPE_BAND_FACTORS: [f32; 9] = [2.5, 1.8, 1.8, 1.2, 1.0, 0.95, 0.95, 1.0, 1.0];
/// Cost per metre of a flat official forest trail; scores 100 (D32).
pub const SCORE_REFERENCE: f64 = 0.63;
pub const EXCLUSION_PENALTY: f64 = 5.0;
pub const WEIGHT_FLOOR: f32 = 0.05;

const SURFACE_FACTORS: [f32; 8] = [2.0, 1.5, 1.1, 1.0, 0.95, 1.1, 1.2, 1.4];
const TECH_FACTORS: [f32; 5] = [1.0, 1.05, 1.15, 1.4, 1.3];
const OFFICIAL_FACTORS: [f32; 2] = [0.7, 1.0];
const FOREST_FACTORS: [f32; 2] = [0.9, 1.0];
const URBAN_FACTORS: [f32; 3] = [1.0, 1.3, 2.0];

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Weights {
    pub slope: f32,
    pub surface: f32,
    pub tech: f32,
    pub official: f32,
    pub forest: f32,
    pub urban: f32,
}

impl Default for Weights {
    fn default() -> Self {
        Weights {
            slope: 1.0,
            surface: 1.0,
            tech: 1.0,
            official: 1.0,
            forest: 1.0,
            urban: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExclusionMode {
    Penalize,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Exclusion {
    pub lat: f64,
    pub lon: f64,
    pub radius_m: f32,
    pub mode: ExclusionMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub weights: Weights,
    pub max_sac: u8,
    pub exclude: Vec<Exclusion>,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            weights: Weights::default(),
            max_sac: 3,
            exclude: Vec::new(),
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ProfileError {
    #[error("weights must be finite and between 0 and 3")]
    WeightOutOfRange,
    #[error("max_sac must be between 1 and 6")]
    MaxSacOutOfRange,
    #[error("exclusion needs finite lat/lon and a radius in (0, 50000] m")]
    BadExclusion,
}

/// `max(1 + (f - 1) * w, WEIGHT_FLOOR)`.
fn weighted(f: f32, w: f32) -> f32 {
    (1.0 + (f - 1.0) * w).max(WEIGHT_FLOOR)
}

fn min_weighted(table: &[f32], w: f32) -> f64 {
    table
        .iter()
        .map(|&f| weighted(f, w))
        .fold(f32::INFINITY, f32::min) as f64
}

impl Profile {
    pub fn validate(&self) -> Result<(), ProfileError> {
        let w = &self.weights;
        let ws = [w.slope, w.surface, w.tech, w.official, w.forest, w.urban];
        if ws
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=3.0).contains(v))
        {
            return Err(ProfileError::WeightOutOfRange);
        }
        if !(1..=6).contains(&self.max_sac) {
            return Err(ProfileError::MaxSacOutOfRange);
        }
        for x in &self.exclude {
            if !x.lat.is_finite()
                || !x.lon.is_finite()
                || !x.radius_m.is_finite()
                || x.radius_m <= 0.0
                || x.radius_m > 50_000.0
            {
                return Err(ProfileError::BadExclusion);
            }
        }
        Ok(())
    }

    /// Admissible A* lower bound on cost per metre under these weights.
    pub fn min_cost_per_m(&self) -> f64 {
        // The slack absorbs f32 rounding in the slope average, keeping the bound admissible.
        let w = &self.weights;
        (1.0 - 1e-6)
            * min_weighted(&SLOPE_BAND_FACTORS, w.slope)
            * min_weighted(&SURFACE_FACTORS, w.surface)
            * min_weighted(&TECH_FACTORS, w.tech)
            * min_weighted(&OFFICIAL_FACTORS, w.official)
            * min_weighted(&FOREST_FACTORS, w.forest)
            * min_weighted(&URBAN_FACTORS, w.urban)
    }
}

/// Weighted factor per term for one edge in one direction. 1.0 = neutral.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct Breakdown {
    pub slope: f32,
    pub surface: f32,
    pub tech: f32,
    pub official: f32,
    pub forest: f32,
    pub urban: f32,
}

impl Breakdown {
    pub fn product(&self) -> f64 {
        [
            self.slope,
            self.surface,
            self.tech,
            self.official,
            self.forest,
            self.urban,
        ]
        .iter()
        .map(|&f| f as f64)
        .product()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeCost {
    pub cost: f64,
    pub per_m: f64,
    pub breakdown: Breakdown,
    pub penalized: bool,
}

/// Pure factor computation: no hard filters, no exclusions.
pub fn edge_factors(g: &Graph, e: u32, dir: Direction, w: &Weights) -> Breakdown {
    let a = g.edge_attrs(e);
    let sum: u32 = a.grade.iter().map(|&v| v as u32).sum();
    let slope = if sum == 0 {
        weighted(1.0, w.slope)
    } else {
        let mut s = 0.0f32;
        for (i, &raw) in SLOPE_BAND_FACTORS.iter().enumerate() {
            let share = match dir {
                Direction::Forward => a.grade[i],
                Direction::Backward => a.grade[GRADE_BANDS - 1 - i],
            };
            s += share as f32 / sum as f32 * weighted(raw, w.slope);
        }
        s
    };
    let tech = match a.sac {
        0 if a.flags & flags::MISSING_TAGS != 0 => 1.3,
        0 | 1 => 1.0,
        2 => 1.05,
        3 => 1.15,
        _ => 1.4,
    };
    Breakdown {
        slope,
        surface: weighted(SURFACE_FACTORS[(a.surface as usize).min(7)], w.surface),
        tech: weighted(tech, w.tech),
        official: weighted(if a.official_category > 0 { 0.7 } else { 1.0 }, w.official),
        forest: weighted(1.0 - 0.1 * a.forest_pct.min(100) as f32 / 100.0, w.forest),
        urban: weighted(
            match a.urban {
                0 => 1.0,
                1 => 1.3,
                _ => 2.0,
            },
            w.urban,
        ),
    }
}

/// True when the marker is within `radius_m` of any segment of the edge geometry.
fn touches(g: &Graph, e: u32, x: &Exclusion) -> bool {
    let pts: Vec<(f64, f64)> = g.edge_geometry(e).map(|(la, lo, _)| (la, lo)).collect();
    if pts.is_empty() {
        return false;
    }
    // Cheap bbox prefilter, padded by the radius (degrees, with a safety margin).
    let r = x.radius_m as f64;
    let dlat = r / 111_000.0 * 1.01;
    let dlon = dlat / x.lat.to_radians().cos().abs().max(1e-6);
    let (mut min_la, mut max_la, mut min_lo, mut max_lo) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(la, lo) in &pts {
        min_la = min_la.min(la);
        max_la = max_la.max(la);
        min_lo = min_lo.min(lo);
        max_lo = max_lo.max(lo);
    }
    if x.lat < min_la - dlat
        || x.lat > max_la + dlat
        || x.lon < min_lo - dlon
        || x.lon > max_lo + dlon
    {
        return false;
    }
    let proj = LocalProjection::new(x.lat);
    let m = LatLon {
        lat: x.lat,
        lon: x.lon,
    };
    let pm = proj.project(m);
    let near = |a: (f64, f64), b: (f64, f64)| {
        let (pt, _) = closest_on_segment(
            pm,
            proj.project(LatLon { lat: a.0, lon: a.1 }),
            proj.project(LatLon { lat: b.0, lon: b.1 }),
        );
        haversine_m(m, proj.unproject(pt)) <= r
    };
    if pts.len() == 1 {
        return near(pts[0], pts[0]);
    }
    pts.windows(2).any(|s| near(s[0], s[1]))
}

/// Hard filters only: sac above `max_sac`, or a touching Block marker.
pub fn traversable(g: &Graph, e: u32, p: &Profile) -> bool {
    if g.edge_attrs(e).sac > p.max_sac {
        return false;
    }
    !p.exclude
        .iter()
        .any(|x| x.mode == ExclusionMode::Block && touches(g, e, x))
}

pub fn edge_cost(g: &Graph, e: u32, dir: Direction, p: &Profile) -> Option<EdgeCost> {
    if !traversable(g, e, p) {
        return None;
    }
    let breakdown = edge_factors(g, e, dir, &p.weights);
    let penalized = p
        .exclude
        .iter()
        .any(|x| x.mode == ExclusionMode::Penalize && touches(g, e, x));
    let per_m = breakdown.product() * if penalized { EXCLUSION_PENALTY } else { 1.0 };
    Some(EdgeCost {
        cost: g.edge_len_m(e) * per_m,
        per_m,
        breakdown,
        penalized,
    })
}

/// D32: `100 * min(SCORE_REFERENCE / mean_factor_default, 1)`.
pub fn score(mean_factor_default: f64) -> f32 {
    if !mean_factor_default.is_finite() || mean_factor_default <= 0.0 {
        return 0.0;
    }
    (100.0 * (SCORE_REFERENCE / mean_factor_default).min(1.0)) as f32
}
