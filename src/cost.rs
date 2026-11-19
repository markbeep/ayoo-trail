//! Edge cost model, routing profile and route score (design §8, D31, D32, D35).

use serde::{Deserialize, Serialize};

use crate::geodesy::{closest_on_segment, haversine_m, LocalProjection};
use crate::graph::{flags, Direction, Graph, LatLon, GRADE_BANDS};

/// Slope factors for the four descent bands, steepest first (<=-25, -25..-20, -20..-15,
/// -15..-8 %), for gentle / any / steep (D45, D53). Starting points to tune (Q17).
const DESCENT_FACTORS: [[f32; 4]; 3] = [
    [2.5, 1.8, 1.8, 1.2],  // gentle
    [1.0, 1.0, 1.0, 1.0],  // any
    [0.9, 0.9, 0.95, 1.0], // steep
];
/// The four ascent bands, shallowest first (8..15, 15..20, 20..25, >=25 %).
const ASCENT_FACTORS: [[f32; 4]; 3] = [
    [1.0, 1.2, 1.5, 1.8],   // gentle
    [1.0, 1.0, 1.0, 1.0],   // any
    [0.95, 0.9, 0.9, 0.95], // steep
];
/// Cost per meter of a flat official forest trail; scores 100 (D32).
pub const SCORE_REFERENCE: f64 = 0.63;
pub const EXCLUSION_PENALTY: f64 = 5.0;
/// Largest weight exponent (D51).
pub const MAX_WEIGHT: f32 = 4.0;
/// At full climb pressure, edge parts steeper than 8 % cost this share of their slope factor.
const CLIMB_DISCOUNT: f32 = 0.8;

const SURFACE_FACTORS: [f32; 8] = [2.0, 1.5, 1.1, 1.0, 0.95, 1.1, 1.2, 1.4];
const TECH_FACTORS: [f32; 5] = [1.0, 1.05, 1.15, 1.4, 1.3];
const OFFICIAL_FACTORS: [f32; 2] = [0.7, 1.0];
const FOREST_FACTORS: [f32; 2] = [0.9, 1.0];
const URBAN_FACTORS: [f32; 3] = [1.0, 1.3, 2.0];

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Weights {
    /// Strength of the slope term; `uphill`/`downhill` pick its direction preferences.
    pub slope: f32,
    /// -1 gentle, 0 any, +1 steep, interpolated (D53).
    pub uphill: f32,
    pub downhill: f32,
    pub surface: f32,
    pub tech: f32,
    pub official: f32,
    pub forest: f32,
    pub urban: f32,
    /// 0..1, set by the generator to chase climb: steep edge parts get cheaper (D52).
    pub climb_pressure: f32,
}

impl Default for Weights {
    fn default() -> Self {
        Weights {
            slope: 1.0,
            uphill: 0.0,
            downhill: -1.0,
            surface: 1.0,
            tech: 1.0,
            official: 1.0,
            forest: 1.0,
            urban: 1.0,
            climb_pressure: 0.0,
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
    #[error("weights must be finite: terms 0-4, uphill/downhill -1-1, climb_pressure 0-1")]
    WeightOutOfRange,
    #[error("max_sac must be between 1 and 6")]
    MaxSacOutOfRange,
    #[error("exclusion needs finite lat/lon and a radius in (0, 50000] m")]
    BadExclusion,
}

/// `f^w` (D51): 0 ignores the term, 1 is the table value, 4 lets it dominate.
fn weighted(f: f32, w: f32) -> f32 {
    f.powf(w)
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
        let ok = |v: f32, r: std::ops::RangeInclusive<f32>| v.is_finite() && r.contains(&v);
        if !ws.iter().all(|&v| ok(v, 0.0..=MAX_WEIGHT))
            || !ok(w.uphill, -1.0..=1.0)
            || !ok(w.downhill, -1.0..=1.0)
            || !ok(w.climb_pressure, 0.0..=1.0)
        {
            return Err(ProfileError::WeightOutOfRange);
        }
        if !(1..=6).contains(&self.max_sac) {
            return Err(ProfileError::MaxSacOutOfRange);
        }
        if self.exclude.len() > 32 {
            return Err(ProfileError::BadExclusion);
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

    /// Admissible A* lower bound on cost per meter under these weights.
    pub fn min_cost_per_m(&self) -> f64 {
        // The slack absorbs f32 rounding in the slope average, keeping the bound admissible.
        (1.0 - 1e-6) * term_minima(&self.weights).product()
    }
}

/// Row `any` blended toward `gentle` (h < 0) or `steep` (h > 0) by |h|.
fn blend(rows: &[[f32; 4]; 3], h: f32) -> [f32; 4] {
    let (to, t) = if h < 0.0 { (rows[0], -h) } else { (rows[2], h) };
    let t = t.min(1.0);
    std::array::from_fn(|i| rows[1][i] + (to[i] - rows[1][i]) * t)
}

/// Raw slope factor per grade band (steepest descent first) for the direction preferences.
pub fn slope_table(w: &Weights) -> [f32; GRADE_BANDS] {
    let (d, a) = (
        blend(&DESCENT_FACTORS, w.downhill),
        blend(&ASCENT_FACTORS, w.uphill),
    );
    [d[0], d[1], d[2], d[3], 1.0, a[0], a[1], a[2], a[3]]
}

/// Weighted slope factor per band, with climb pressure applied to every non-flat band.
fn weighted_slope(w: &Weights) -> [f32; GRADE_BANDS] {
    let discount = 1.0 - CLIMB_DISCOUNT * w.climb_pressure;
    let raw = slope_table(w);
    std::array::from_fn(|i| {
        weighted(raw[i], w.slope) * if i == GRADE_BANDS / 2 { 1.0 } else { discount }
    })
}

/// Smallest weighted factor each term can take under `w`.
pub fn term_minima(w: &Weights) -> Breakdown {
    Breakdown {
        slope: weighted_slope(w).into_iter().fold(f32::INFINITY, f32::min),
        surface: min_weighted(&SURFACE_FACTORS, w.surface) as f32,
        tech: min_weighted(&TECH_FACTORS, w.tech) as f32,
        official: min_weighted(&OFFICIAL_FACTORS, w.official) as f32,
        forest: min_weighted(&FOREST_FACTORS, w.forest) as f32,
        urban: min_weighted(&URBAN_FACTORS, w.urban) as f32,
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
    FactorTable::new(w).breakdown(g, e, dir)
}

/// Every weighted factor for one set of weights, so the A* inner loop does lookups instead
/// of ~14 `powf` per edge. Same values as computing them per edge.
pub struct FactorTable {
    slope: [f32; GRADE_BANDS],
    surface: [f32; 8],
    tech: [f32; 5],
    official: [f32; 2],
    /// By forest share 0..=100 %.
    forest: [f32; 101],
    urban: [f32; 3],
}

impl FactorTable {
    pub fn new(w: &Weights) -> Self {
        FactorTable {
            slope: weighted_slope(w),
            surface: SURFACE_FACTORS.map(|f| weighted(f, w.surface)),
            tech: TECH_FACTORS.map(|f| weighted(f, w.tech)),
            official: OFFICIAL_FACTORS.map(|f| weighted(f, w.official)),
            forest: std::array::from_fn(|pct| {
                weighted(
                    FOREST_FACTORS[1]
                        - (FOREST_FACTORS[1] - FOREST_FACTORS[0]) * pct as f32 / 100.0,
                    w.forest,
                )
            }),
            urban: URBAN_FACTORS.map(|f| weighted(f, w.urban)),
        }
    }

    pub fn breakdown(&self, g: &Graph, e: u32, dir: Direction) -> Breakdown {
        let a = g.edge_attrs(e);
        let sum: u32 = a.grade.iter().map(|&v| v as u32).sum();
        let slope = if sum == 0 {
            1.0
        } else {
            let mut s = 0.0f32;
            for (i, &f) in self.slope.iter().enumerate() {
                let share = match dir {
                    Direction::Forward => a.grade[i],
                    Direction::Backward => a.grade[GRADE_BANDS - 1 - i],
                };
                s += share as f32 / sum as f32 * f;
            }
            s
        };
        let tech = match a.sac {
            0 if a.flags & flags::MISSING_TAGS != 0 => 4,
            0 | 1 => 0,
            2 => 1,
            3 => 2,
            _ => 3,
        };
        Breakdown {
            slope,
            surface: self.surface[(a.surface as usize).min(7)],
            tech: self.tech[tech],
            official: self.official[(a.official_category == 0) as usize],
            forest: self.forest[a.forest_pct.min(100) as usize],
            urban: self.urban[(a.urban as usize).min(2)],
        }
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
    edge_cost_with(g, e, dir, p, &FactorTable::new(&p.weights))
}

/// `edge_cost` with the profile's factor table built once (`FactorTable::new(&p.weights)`).
pub fn edge_cost_with(
    g: &Graph,
    e: u32,
    dir: Direction,
    p: &Profile,
    t: &FactorTable,
) -> Option<EdgeCost> {
    if !traversable(g, e, p) {
        return None;
    }
    let breakdown = t.breakdown(g, e, dir);
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

/// D44: `100 * min(SCORE_REFERENCE / mean_factor_default, 1)^(1/6)`, the geometric mean over
/// the six cost terms of D32's ratio. A monotone rescale, so rankings and slider stability stay.
pub fn score(mean_factor_default: f64) -> f32 {
    if !mean_factor_default.is_finite() || mean_factor_default <= 0.0 {
        return 0.0;
    }
    (100.0
        * (SCORE_REFERENCE / mean_factor_default)
            .min(1.0)
            .powf(1.0 / 6.0)) as f32
}

/// How well a climb matches its target: `min(C, T) / max(C, T)`, in (0, 1].
pub fn climb_match(climb_m: f64, target_m: f64) -> f64 {
    let c = climb_m.max(1.0);
    c.min(target_m) / c.max(target_m)
}

/// D63: with a climb target the score adds that match as a seventh term of D44's geometric mean.
pub fn score_with_climb(mean_factor_default: f64, climb_match: f64) -> f32 {
    if !mean_factor_default.is_finite() || mean_factor_default <= 0.0 {
        return 0.0;
    }
    let ratio = (SCORE_REFERENCE / mean_factor_default).min(1.0);
    (100.0 * (ratio * climb_match.clamp(0.0, 1.0)).powf(1.0 / 7.0)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn term_minima_follow_the_tables_and_weights() {
        let m = term_minima(&Weights::default());
        assert_eq!(
            (m.slope, m.surface, m.official, m.forest),
            (1.0, 0.95, 0.7, 0.9)
        );
        assert_eq!((m.tech, m.urban), (1.0, 1.0));
        let off = term_minima(&Weights {
            official: 0.0,
            ..Weights::default()
        });
        assert_eq!(off.official, 1.0);
    }

    #[test]
    fn more_than_32_exclusions_are_rejected() {
        let x = Exclusion {
            lat: 46.0,
            lon: 7.0,
            radius_m: 100.0,
            mode: ExclusionMode::Block,
        };
        let mut p = Profile {
            exclude: vec![x; 32],
            ..Profile::default()
        };
        assert!(p.validate().is_ok());
        p.exclude.push(x);
        assert_eq!(p.validate(), Err(ProfileError::BadExclusion));
    }
}
