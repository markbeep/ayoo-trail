//! 360° view score (D100): per 100 m LV95 cell, "how far you see", the mean
//! log-scaled distance to the skyline over 32 azimuths. Built offline into
//! `view.bin` by `trail-calc-view`; the server samples it per edge and renders
//! the overlay from it.

use std::f64::consts::TAU;
use std::io::{self, Read, Write};
use std::path::Path;

use rayon::prelude::*;

use crate::import::ch1903::wgs84_to_lv95;
use crate::import::dem::ElevationSource;

pub const CELL_M: f64 = 100.0;
const AZIMUTHS: usize = 32;
const MAX_M: f64 = 60_000.0;
/// Eye height above the ground: on flat land it puts the skyline at the ~5 km horizon.
const EYE_M: f64 = 2.0;
/// Rays start this far out: a 100 m DEM can't resolve the ground around you, and a
/// neighbouring cell at your own height would hide every view across a valley.
const NEAR_M: f64 = 300.0;
/// Curvature drop with refraction, d²(1 − k) / 2R.
const R_EARTH_M: f64 = 6_371_000.0;
const K_REFRACT: f64 = 0.13;
const MAGIC: &[u8; 8] = b"AYVIEW01";

/// View scores on an LV95 grid, north row first. 0 = no data, 1..=255 low to high view.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewGrid {
    /// West and north edges of the grid.
    pub e0: f64,
    pub n0: f64,
    pub cell: f64,
    pub w: usize,
    pub h: usize,
    pub v: Vec<u8>,
}

impl ViewGrid {
    /// Score of the cell holding an LV95 point, None outside the grid or without data.
    pub fn at(&self, e: f64, n: f64) -> Option<u8> {
        let x = ((e - self.e0) / self.cell).floor();
        let y = ((self.n0 - n) / self.cell).floor();
        if x < 0.0 || y < 0.0 || x >= self.w as f64 || y >= self.h as f64 {
            return None;
        }
        let v = self.v[y as usize * self.w + x as usize];
        (v > 0).then_some(v)
    }

    pub fn at_wgs84(&self, lat: f64, lon: f64) -> Option<u8> {
        let (e, n) = wgs84_to_lv95(lat, lon);
        self.at(e, n)
    }

    /// A 256 px web-mercator tile of the overlay as RGBA, or None when no pixel has data.
    pub fn tile_rgba(&self, z: u8, x: u32, y: u32) -> Option<Vec<u8>> {
        let n = (1u64 << z) as f64;
        let mut out = vec![0u8; 256 * 256 * 4];
        let mut any = false;
        for py in 0..256 {
            let t = (y as f64 + (py as f64 + 0.5) / 256.0) / n;
            let lat = (std::f64::consts::PI * (1.0 - 2.0 * t))
                .sinh()
                .atan()
                .to_degrees();
            for px in 0..256 {
                let lon = (x as f64 + (px as f64 + 0.5) / 256.0) / n * 360.0 - 180.0;
                if let Some(v) = self.at_wgs84(lat, lon) {
                    let i = (py * 256 + px) * 4;
                    out[i..i + 4].copy_from_slice(&ramp(v));
                    any = true;
                }
            }
        }
        any.then_some(out)
    }

    /// `tile_rgba` as a PNG.
    pub fn tile_png(&self, z: u8, x: u32, y: u32) -> Option<Vec<u8>> {
        let rgba = self.tile_rgba(z, x, y)?;
        let mut buf = Vec::new();
        let mut enc = png::Encoder::new(&mut buf, 256, 256);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        enc.write_header().ok()?.write_image_data(&rgba).ok()?;
        Some(buf)
    }

    pub fn write(&self, path: &Path) -> io::Result<()> {
        let mut f = io::BufWriter::new(std::fs::File::create(path)?);
        f.write_all(MAGIC)?;
        for x in [self.e0, self.n0, self.cell] {
            f.write_all(&x.to_le_bytes())?;
        }
        f.write_all(&(self.w as u32).to_le_bytes())?;
        f.write_all(&(self.h as u32).to_le_bytes())?;
        f.write_all(&self.v)?;
        f.flush()
    }

    pub fn open(path: &Path) -> io::Result<ViewGrid> {
        let mut b = Vec::new();
        std::fs::File::open(path)?.read_to_end(&mut b)?;
        let bad = || io::Error::new(io::ErrorKind::InvalidData, "not a view.bin");
        if b.len() < 40 || &b[..8] != MAGIC {
            return Err(bad());
        }
        let f = |i: usize| f64::from_le_bytes(b[i..i + 8].try_into().unwrap());
        let u = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize;
        let (w, h) = (u(32), u(36));
        if b.len() != 40 + w * h {
            return Err(bad());
        }
        Ok(ViewGrid {
            e0: f(8),
            n0: f(16),
            cell: f(24),
            w,
            h,
            v: b[40..].to_vec(),
        })
    }
}

/// Overlay colour of a view score: dark in valleys, faint around the CH median (~93),
/// yellow to red toward the best views (owner's step 1: red = excellent in every direction).
/// Stops are a starting point to tune.
fn ramp(v: u8) -> [u8; 4] {
    const STOPS: [(f32, [f32; 4]); 5] = [
        (0.0, [40.0, 40.0, 90.0, 150.0]),
        (0.35, [70.0, 110.0, 170.0, 60.0]),
        (0.55, [250.0, 220.0, 80.0, 120.0]),
        (0.75, [245.0, 130.0, 40.0, 160.0]),
        (0.9, [215.0, 40.0, 40.0, 190.0]),
    ];
    let s = (v.max(1) - 1) as f32 / 254.0;
    let i = STOPS.iter().rposition(|st| st.0 <= s).unwrap_or(0);
    let (a, b) = (STOPS[i], STOPS[(i + 1).min(STOPS.len() - 1)]);
    let t = if b.0 > a.0 {
        ((s - a.0) / (b.0 - a.0)).min(1.0)
    } else {
        0.0
    };
    std::array::from_fn(|k| (a.1[k] + (b.1[k] - a.1[k]) * t).round() as u8)
}

/// Elevation at each cell centre (NaN without data), sampled in 1 km blocks so each
/// swissALTI3D tile is decoded once when `e0`/`n0` lie on the kilometre grid.
pub fn sample_dem(src: &dyn ElevationSource, e0: f64, n0: f64, w: usize, h: usize) -> Vec<f32> {
    const B: usize = (1000.0 / CELL_M) as usize;
    let (bw, bh) = (w.div_ceil(B), h.div_ceil(B));
    let blocks: Vec<Vec<(usize, f32)>> = (0..bw * bh)
        .into_par_iter()
        .map(|b| {
            let (bx, by) = (b % bw, b / bw);
            let mut out = Vec::with_capacity(B * B);
            for y in by * B..((by + 1) * B).min(h) {
                for x in bx * B..((bx + 1) * B).min(w) {
                    let (e, n) = (
                        e0 + (x as f64 + 0.5) * CELL_M,
                        n0 - (y as f64 + 0.5) * CELL_M,
                    );
                    out.push((y * w + x, src.sample_lv95(e, n).unwrap_or(f32::NAN)));
                }
            }
            out
        })
        .collect();
    let mut z = vec![f32::NAN; w * h];
    for (i, v) in blocks.into_iter().flatten() {
        z[i] = v;
    }
    z
}

/// The view score of every cell of a `w × h` elevation grid (north row first, NaN = no data).
/// shortcut: a ray stops where the DEM ends, so cells near the border see less than they do; feed a wider DEM when regions grow.
pub fn view_scores(z: &[f32], w: usize, h: usize, cell: f64) -> Vec<u8> {
    let dirs: [(f64, f64); AZIMUTHS] = std::array::from_fn(|i| {
        let a = i as f64 * TAU / AZIMUTHS as f64;
        (a.sin(), a.cos()) // (east, north)
    });
    let norm = (MAX_M / NEAR_M).ln();
    (0..w * h)
        .into_par_iter()
        .map(|i| {
            let z0 = z[i];
            if !z0.is_finite() {
                return 0;
            }
            let (x0, y0) = ((i % w) as f64 + 0.5, (i / w) as f64 + 0.5);
            let eye = z0 as f64 + EYE_M;
            let mut sum = 0.0;
            for &(de, dn) in &dirs {
                let (mut d, mut best, mut best_d) = (NEAR_M, f64::NEG_INFINITY, NEAR_M);
                while d <= MAX_M {
                    let (x, y) = (x0 + de * d / cell, y0 - dn * d / cell);
                    if x < 0.0 || y < 0.0 || x >= w as f64 || y >= h as f64 {
                        break;
                    }
                    let zz = z[y as usize * w + x as usize];
                    if !zz.is_finite() {
                        break;
                    }
                    // tangent of the elevation angle; the skyline is where it peaks
                    let t = (zz as f64 - eye - d * d * (1.0 - K_REFRACT) / (2.0 * R_EARTH_M)) / d;
                    if t > best {
                        (best, best_d) = (t, d);
                    }
                    d += cell.max(d * 0.02);
                }
                sum += (best_d / NEAR_M).ln() / norm;
            }
            1 + (254.0 * sum / AZIMUTHS as f64).round().clamp(0.0, 254.0) as u8
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cone 2 km high in the middle of a 30 km × 30 km plain.
    fn cone() -> (Vec<f32>, usize) {
        let n = 300;
        let c = n as f64 / 2.0;
        let z = (0..n * n)
            .map(|i| {
                let r = (((i % n) as f64 + 0.5 - c).hypot((i / n) as f64 + 0.5 - c)) * CELL_M;
                (2000.0 - r * 0.4).max(0.0) as f32
            })
            .collect();
        (z, n)
    }

    #[test]
    fn peak_sees_further_than_its_foot_and_the_plain() {
        let (z, n) = cone();
        let v = view_scores(&z, n, n, CELL_M);
        let at = |x: usize, y: usize| v[y * n + x];
        let (peak, foot, plain) = (at(150, 150), at(150, 150 + 52), at(150, 290));
        assert!(peak > foot && foot > 0, "peak {peak}, foot {foot}");
        assert!(peak > plain, "peak {peak}, plain {plain}");
    }

    #[test]
    fn a_narrow_valley_sees_less_than_the_ridge_above_it() {
        // A V valley running north–south, walls 600 m high 1.5 km out.
        let (n, mid) = (200usize, 100.0);
        let z: Vec<f32> = (0..n * n)
            .map(|i| (((i % n) as f64 + 0.5 - mid).abs() * CELL_M * 0.4).min(600.0) as f32)
            .collect();
        let v = view_scores(&z, n, n, CELL_M);
        let (floor, ridge) = (v[100 * n + 100], v[100 * n + 100 + 20]);
        assert!(ridge > floor, "ridge {ridge}, floor {floor}");
    }

    #[test]
    fn no_data_scores_zero() {
        let mut z = vec![500.0f32; 100];
        z[55] = f32::NAN;
        let v = view_scores(&z, 10, 10, CELL_M);
        assert_eq!(v[55], 0);
        assert!(v[0] > 0);
    }

    #[test]
    fn tile_pixels_follow_the_grid_and_ramp() {
        use crate::import::ch1903::wgs84_to_lv95;
        // A 20 km grid around Wattwil, best view everywhere.
        let (e, n) = wgs84_to_lv95(47.3, 9.09);
        let g = ViewGrid {
            e0: e - 10_000.0,
            n0: n + 10_000.0,
            cell: CELL_M,
            w: 200,
            h: 200,
            v: vec![255; 40_000],
        };
        // z12 tile holding Wattwil: x = 2151, y = 1435.
        let rgba = g.tile_rgba(12, 2151, 1435).expect("data in this tile");
        assert!(rgba.chunks(4).any(|p| p == ramp(255)));
        assert_eq!(ramp(255), [215, 40, 40, 190], "best views are red");
        assert!(g.tile_rgba(12, 0, 0).is_none(), "nothing outside the grid");
        assert_eq!(&g.tile_png(12, 2151, 1435).unwrap()[1..4], b"PNG");
    }

    #[test]
    fn file_round_trip_and_lookup() {
        let g = ViewGrid {
            e0: 2_700_000.0,
            n0: 1_250_000.0,
            cell: CELL_M,
            w: 3,
            h: 2,
            v: vec![1, 2, 3, 4, 0, 6],
        };
        let p = std::env::temp_dir().join(format!("view-{}.bin", std::process::id()));
        g.write(&p).unwrap();
        let r = ViewGrid::open(&p).unwrap();
        std::fs::remove_file(&p).ok();
        assert_eq!(r, g);
        assert_eq!(r.at(2_700_250.0, 1_249_950.0), Some(3));
        assert_eq!(r.at(2_700_150.0, 1_249_850.0), None); // 0 = no data
        assert_eq!(r.at(2_699_999.0, 1_249_950.0), None);
    }
}
