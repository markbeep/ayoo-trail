//! Elevation sources in the native Swiss grid, and applying one to the graph:
//! node and geometry Z, gain/loss and the per-edge grade histogram (D30).

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use rayon::prelude::*;

use super::ch1903::wgs84_to_lv95;
use super::ImportError;
use crate::graph::{flags, GraphWriter, FLAT_GRADE, GRADE_BANDS};

pub const DEM_NONE: u8 = 0;
pub const DEM_ALTI3D: u8 = 1;
pub const DEM_ASC: u8 = 2;

pub trait ElevationSource: Sync {
    /// Elevation at an LV95 coordinate, None outside coverage.
    fn sample_lv95(&self, e: f64, n: f64) -> Option<f32>;
    fn source_id(&self) -> u8;
    fn sample(&self, lat: f64, lon: f64) -> Option<f32> {
        let (e, n) = wgs84_to_lv95(lat, lon);
        self.sample_lv95(e, n)
    }
}

/// Bilinear blend of corners `[sw, se, nw, ne]` at fractions (fx, fy) in 0..=1.
/// A missing corner takes the value of the nearest valid corner (the one with
/// the largest weight); None only when all four are missing.
pub(crate) fn bilinear(c: [Option<f32>; 4], fx: f64, fy: f64) -> Option<f32> {
    let w = [
        (1.0 - fx) * (1.0 - fy),
        fx * (1.0 - fy),
        (1.0 - fx) * fy,
        fx * fy,
    ];
    let nearest = c
        .into_iter()
        .zip(w)
        .filter_map(|(v, w)| v.map(|v| (v, w)))
        .max_by(|a, b| a.1.total_cmp(&b.1))?
        .0;
    let z: f64 = c
        .into_iter()
        .zip(w)
        .map(|(v, w)| v.unwrap_or(nearest) as f64 * w)
        .sum();
    Some(z as f32)
}

/// ESRI ASCII grid (DHM200 / a 25 m DHM25 matrix / any Swiss-grid .asc).
/// Values are cell centres (`xllcorner` is the outer corner, `xllcenter` the
/// first centre). LV03 is detected from xllcorner < 1_000_000.
pub struct AscGrid {
    ncols: usize,
    nrows: usize,
    /// South-west outer corner, LV95.
    west: f64,
    south: f64,
    cell: f64,
    nodata: f32,
    /// Row-major, north row first.
    z: Vec<f32>,
}

impl AscGrid {
    pub fn from_asc(path: &Path) -> Result<AscGrid, ImportError> {
        let bad = |m: &str| ImportError::Format(format!("{}: {m}", path.display()));
        let (mut ncols, mut nrows, mut cell) = (None, None, None);
        let (mut x, mut y, mut nodata) = (None, None, -9999.0f32);
        let mut x_center = false;
        let mut y_center = false;
        let mut z = Vec::new();
        for line in BufReader::new(File::open(path)?).lines() {
            let line = line?;
            let mut it = line.split_ascii_whitespace();
            let Some(first) = it.next() else { continue };
            if z.is_empty() && first.starts_with(|c: char| c.is_ascii_alphabetic()) {
                let v: f64 = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| bad(&format!("bad header line {line:?}")))?;
                match first.to_ascii_lowercase().as_str() {
                    "ncols" => ncols = Some(v),
                    "nrows" => nrows = Some(v),
                    "cellsize" => cell = Some(v),
                    "xllcorner" => x = Some(v),
                    "yllcorner" => y = Some(v),
                    "xllcenter" => (x, x_center) = (Some(v), true),
                    "yllcenter" => (y, y_center) = (Some(v), true),
                    "nodata_value" => nodata = v as f32,
                    k => return Err(bad(&format!("unknown header key {k:?}"))),
                }
                continue;
            }
            for t in std::iter::once(first).chain(it) {
                z.push(
                    t.parse::<f32>()
                        .map_err(|_| bad(&format!("bad value {t:?}")))?,
                );
            }
        }
        let dim = |v: Option<f64>, k: &str| match v {
            Some(v) if v >= 1.0 && v.fract() == 0.0 && v < 1e7 => Ok(v as usize),
            _ => Err(bad(&format!("missing or bad {k}"))),
        };
        let (ncols, nrows) = (dim(ncols, "ncols")?, dim(nrows, "nrows")?);
        let cell = cell
            .filter(|c| c.is_finite() && *c > 0.0)
            .ok_or_else(|| bad("missing or bad cellsize"))?;
        let (Some(mut x), Some(mut y)) = (x, y) else {
            return Err(bad("missing xll/yll"));
        };
        if !(x.is_finite() && y.is_finite()) {
            return Err(bad("bad xll/yll"));
        }
        if z.len() != ncols * nrows {
            return Err(bad(&format!(
                "{} values for a {ncols}×{nrows} grid",
                z.len()
            )));
        }
        if x_center {
            x -= cell / 2.0;
        }
        if y_center {
            y -= cell / 2.0;
        }
        if x < 1_000_000.0 {
            x += 2_000_000.0;
            y += 1_000_000.0;
        }
        Ok(AscGrid {
            ncols,
            nrows,
            west: x,
            south: y,
            cell,
            nodata,
            z,
        })
    }

    /// Bilinear between cell centres at an LV95 coordinate; clamped to the
    /// outermost centres at the grid edge; None outside the grid.
    pub fn sample_grid(&self, e: f64, n: f64) -> Option<f32> {
        let fx = (e - self.west) / self.cell;
        let fy = (n - self.south) / self.cell;
        if !(0.0..=self.ncols as f64).contains(&fx) || !(0.0..=self.nrows as f64).contains(&fy) {
            return None;
        }
        let gx = (fx - 0.5).clamp(0.0, (self.ncols - 1) as f64);
        let gy = (fy - 0.5).clamp(0.0, (self.nrows - 1) as f64);
        let (c0, r0) = (gx.floor() as usize, gy.floor() as usize);
        let (c1, r1) = ((c0 + 1).min(self.ncols - 1), (r0 + 1).min(self.nrows - 1));
        // `r` counts rows from the south.
        let get = |c: usize, r: usize| {
            let v = self.z[(self.nrows - 1 - r) * self.ncols + c];
            (v != self.nodata && v.is_finite()).then_some(v)
        };
        bilinear(
            [get(c0, r0), get(c1, r0), get(c0, r1), get(c1, r1)],
            gx - c0 as f64,
            gy - r0 as f64,
        )
    }
}

impl ElevationSource for AscGrid {
    fn sample_lv95(&self, e: f64, n: f64) -> Option<f32> {
        self.sample_grid(e, n)
    }
    fn source_id(&self) -> u8 {
        DEM_ASC
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct DemStats {
    pub edges: u64,
    pub samples: u64,
    pub missing_samples: u64,
    pub vertices_added: u64,
}

pub const SAMPLE_SPACING_M: f64 = 10.0;
pub const GRADE_WINDOW_M: f64 = 25.0;
pub const DENSIFY_TOLERANCE_M: f64 = 3.0;
/// Percent.
pub const GRADE_BAND_EDGES: [f64; 8] = [-25.0, -20.0, -15.0, -8.0, 8.0, 15.0, 20.0, 25.0];

/// Band index 0..=8 for a grade in percent; a value equal to an edge goes to
/// the band nearer to flat (so -25 → 1, 25 → 7).
pub fn grade_band(grade_pct: f64) -> usize {
    let up = GRADE_BAND_EDGES
        .iter()
        .filter(|&&e| e > 0.0 && e < grade_pct)
        .count();
    let down = GRADE_BAND_EDGES
        .iter()
        .filter(|&&e| e < 0.0 && e > grade_pct)
        .count();
    crate::graph::FLAT_BAND + up - down
}

struct EdgeOut {
    geometry: Vec<(f64, f64, f32)>,
    gain: i16,
    loss: i16,
    grade: [u8; GRADE_BANDS],
    samples: u64,
    missing: u64,
    added: u64,
}

/// Samples z along the edge, elevations of the bridge/tunnel deck interpolated.
pub fn apply_dem(w: &mut GraphWriter, dem: &dyn ElevationSource) -> DemStats {
    let writer = &*w;
    let outs: Vec<Option<EdgeOut>> = (0..writer.edge_count() as u32)
        .into_par_iter()
        .map(|e| {
            let deck = writer.edge_attrs(e).flags & (flags::BRIDGE | flags::TUNNEL) != 0;
            process_edge(writer.edge_geometry(e), deck, dem)
        })
        .collect();
    let mut stats = DemStats::default();
    for (e, out) in outs.into_iter().enumerate() {
        let Some(o) = out else { continue };
        let e = e as u32;
        let (from, to) = w.edge_endpoints(e);
        w.set_node_elev(from, o.geometry[0].2);
        w.set_node_elev(to, o.geometry[o.geometry.len() - 1].2);
        let a = w.edge_attrs_mut(e);
        a.gain = o.gain;
        a.loss = o.loss;
        a.grade = o.grade;
        w.set_edge_geometry(e, o.geometry);
        stats.edges += 1;
        stats.samples += o.samples;
        stats.missing_samples += o.missing;
        stats.vertices_added += o.added;
    }
    w.set_dem_source(dem.source_id());
    stats
}

fn process_edge(
    geom: &[(f64, f64, f32)],
    deck: bool,
    dem: &dyn ElevationSource,
) -> Option<EdgeOut> {
    let first = geom.first()?;
    // Resample every SAMPLE_SPACING_M in LV95 metres, keeping the original vertices.
    let lv: Vec<(f64, f64)> = geom
        .iter()
        .map(|&(la, lo, _)| wgs84_to_lv95(la, lo))
        .collect();
    let mut d = vec![0.0];
    let mut pos = vec![(first.0, first.1, lv[0].0, lv[0].1)];
    let mut orig = vec![true];
    for i in 1..geom.len() {
        let (a, b) = (lv[i - 1], lv[i]);
        let len = (b.0 - a.0).hypot(b.1 - a.1);
        let k = (len / SAMPLE_SPACING_M).ceil().max(1.0) as usize;
        let d0 = d[d.len() - 1];
        for j in 1..=k {
            let t = j as f64 / k as f64;
            let lerp = |p: f64, q: f64| p + t * (q - p);
            d.push(d0 + t * len);
            pos.push((
                lerp(geom[i - 1].0, geom[i].0),
                lerp(geom[i - 1].1, geom[i].1),
                lerp(a.0, b.0),
                lerp(a.1, b.1),
            ));
            orig.push(j == k);
        }
    }
    let last = d.len() - 1;
    let raw: Vec<Option<f32>> = (0..d.len())
        .map(|i| {
            (!deck || i == 0 || i == last)
                .then(|| dem.sample_lv95(pos[i].2, pos[i].3))
                .flatten()
        })
        .collect();
    let samples = if deck { 2.min(d.len()) } else { d.len() } as u64;
    let valid = raw.iter().filter(|v| v.is_some()).count() as u64;
    let z = fill(&raw, &d);

    let (gain, loss) = gain_loss(&z);
    let grade = grade_histogram(&d, &z);

    let keep = densify(&d, &z, &orig);
    let geometry: Vec<(f64, f64, f32)> = (0..d.len())
        .filter(|&i| keep[i])
        .map(|i| (pos[i].0, pos[i].1, z[i]))
        .collect();
    let added = (keep.iter().filter(|&&k| k).count() - orig.iter().filter(|&&o| o).count()) as u64;
    Some(EdgeOut {
        geometry,
        gain: round_i16(gain),
        loss: round_i16(loss),
        grade,
        samples,
        missing: samples - valid,
        added,
    })
}

fn round_i16(v: f64) -> i16 {
    v.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16
}

/// Missing samples linearly interpolated (by distance) from valid neighbours;
/// constant beyond the first/last valid one; all zero if none is valid.
fn fill(raw: &[Option<f32>], d: &[f64]) -> Vec<f32> {
    let valid: Vec<usize> = (0..raw.len()).filter(|&i| raw[i].is_some()).collect();
    if valid.is_empty() {
        return vec![0.0; raw.len()];
    }
    let at = |i: usize| raw[i].unwrap_or(0.0);
    let mut k = 0; // valid[k] is the first valid index >= i, or the last one
    (0..raw.len())
        .map(|i| {
            while k + 1 < valid.len() && valid[k] < i {
                k += 1;
            }
            let next = valid[k];
            if let Some(z) = raw[i] {
                z
            } else if next < i || k == 0 {
                at(next)
            } else {
                let prev = valid[k - 1];
                let span = d[next] - d[prev];
                let t = if span > 0.0 {
                    (d[i] - d[prev]) / span
                } else {
                    0.0
                };
                at(prev) + (t * (at(next) - at(prev)) as f64) as f32
            }
        })
        .collect()
}

/// Forward gain and loss with a 2 m deadband: a rise or fall counts once it
/// exceeds 2 m from the last turning point.
fn gain_loss(z: &[f32]) -> (f64, f64) {
    const DEADBAND: f64 = 2.0;
    let Some(&z0) = z.first() else {
        return (0.0, 0.0);
    };
    let (mut gain, mut loss) = (0.0, 0.0);
    let (mut turn, mut ext, mut dir) = (z0 as f64, z0 as f64, 0i8);
    for &z in &z[1..] {
        let z = z as f64;
        match dir {
            0 if z - turn > DEADBAND => (dir, ext) = (1, z),
            0 if turn - z > DEADBAND => (dir, ext) = (-1, z),
            1 if z > ext => ext = z,
            1 if ext - z > DEADBAND => {
                gain += ext - turn;
                (turn, ext, dir) = (ext, z, -1);
            }
            -1 if z < ext => ext = z,
            -1 if z - ext > DEADBAND => {
                loss += turn - ext;
                (turn, ext, dir) = (ext, z, 1);
            }
            _ => {}
        }
    }
    match dir {
        1 => gain += ext - turn,
        -1 => loss += turn - ext,
        _ => {}
    }
    (gain, loss)
}

/// Length shares (sum 255) per grade band, grade measured over consecutive
/// windows of at least GRADE_WINDOW_M; the last window absorbs a short remainder.
fn grade_histogram(d: &[f64], z: &[f32]) -> [u8; GRADE_BANDS] {
    let last = d.len() - 1;
    let mut cuts = vec![0];
    for i in 1..d.len() {
        if d[i] - d[cuts[cuts.len() - 1]] >= GRADE_WINDOW_M {
            cuts.push(i);
        }
    }
    if cuts[cuts.len() - 1] != last {
        let n = cuts.len();
        if n > 1 {
            cuts[n - 1] = last;
        } else {
            cuts.push(last);
        }
    }
    let mut lens = [0.0f64; GRADE_BANDS];
    for c in cuts.windows(2) {
        let len = d[c[1]] - d[c[0]];
        if len > 0.0 {
            let grade = (z[c[1]] - z[c[0]]) as f64 / len * 100.0;
            lens[grade_band(grade)] += len;
        }
    }
    let total: f64 = lens.iter().sum();
    if total <= 0.0 {
        return FLAT_GRADE;
    }
    // Largest-remainder rounding so the shares sum to exactly 255.
    let raw = lens.map(|l| l / total * 255.0);
    let mut out = raw.map(|r| r.floor() as u8);
    let short = 255 - out.iter().map(|&v| v as u32).sum::<u32>();
    let mut order: Vec<usize> = (0..GRADE_BANDS).collect();
    order.sort_by(|&a, &b| (raw[b] - raw[b].floor()).total_cmp(&(raw[a] - raw[a].floor())));
    for &i in order.iter().take(short as usize) {
        out[i] += 1;
    }
    out
}

/// Which samples to keep as geometry: every original vertex, plus samples where
/// the (distance, z) profile deviates more than DENSIFY_TOLERANCE_M from the
/// line between kept points (Douglas–Peucker per original span).
fn densify(d: &[f64], z: &[f32], orig: &[bool]) -> Vec<bool> {
    let mut keep = orig.to_vec();
    let originals: Vec<usize> = (0..orig.len()).filter(|&i| orig[i]).collect();
    let mut stack: Vec<(usize, usize)> = originals.windows(2).map(|w| (w[0], w[1])).collect();
    while let Some((a, b)) = stack.pop() {
        let span = d[b] - d[a];
        let mut worst = None;
        let mut dev = DENSIFY_TOLERANCE_M;
        for i in a + 1..b {
            let t = if span > 0.0 {
                (d[i] - d[a]) / span
            } else {
                0.0
            };
            let line = z[a] as f64 + t * (z[b] - z[a]) as f64;
            let off = (z[i] as f64 - line).abs();
            if off > dev {
                (worst, dev) = (Some(i), off);
            }
        }
        if let Some(i) = worst {
            keep[i] = true;
            stack.push((a, i));
            stack.push((i, b));
        }
    }
    keep
}
