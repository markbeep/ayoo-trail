//! Link-preview image of a shared loop (ADR 0006): the map's land colour, relief shading from
//! Mapterhorn terrain, the 360° view overlay (D100) and the loop in the active-loop red, as a PNG.

use rayon::prelude::*;
use std::collections::HashMap;
use std::f64::consts::PI;

use crate::view::{ramp, ViewGrid};

pub const W: usize = 1200;
pub const H: usize = 630;
const PAD: f64 = 60.0;
/// Mapterhorn terrarium tiles (512 px WebP), as the client's `TERRAIN` tilejson lists them.
const TERRAIN: &str = "https://tiles.mapterhorn.com";
const TILE: f64 = 512.0;
/// `--map-land`, `--map-shade`, `--route-casing`, `--route-active` in `web/src/styles.css`.
const LAND: [f32; 3] = [242.0, 239.0, 233.0];
const SHADE: [f32; 3] = [59.0, 52.0, 47.0];
const CASING: [f32; 3] = [255.0, 255.0, 255.0];
const ROUTE: [f32; 3] = [213.0, 40.0, 27.0];

/// Elevations (m, row-major 512²) of one terrain tile, None when it can't be had.
pub type Terrain<'a> = dyn Fn(u8, u32, u32) -> Option<Vec<f32>> + Sync + 'a;

/// Web-mercator unit square (0..1) of a WGS84 point.
fn merc(lat: f64, lon: f64) -> (f64, f64) {
    let s = lat.to_radians().sin().clamp(-0.9999, 0.9999);
    (
        (lon + 180.0) / 360.0,
        0.5 - ((1.0 + s) / (1.0 - s)).ln() / (4.0 * PI),
    )
}

fn unmerc_lat(y: f64) -> f64 {
    (PI * (1.0 - 2.0 * y)).sinh().atan().to_degrees()
}

/// Render `coords` (`[lon, lat]`) as W×H RGB.
pub fn render(coords: &[[f64; 2]], view: Option<&ViewGrid>, terrain: &Terrain) -> Option<Vec<u8>> {
    let pts: Vec<(f64, f64)> = coords.iter().map(|c| merc(c[1], c[0])).collect();
    let (x0, x1) = pts
        .iter()
        .fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0)));
    let (y0, y1) = pts
        .iter()
        .fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
    if pts.is_empty() {
        return None;
    }
    // Pixels per unit, capped at zoom 15 like a short loop on the map.
    let s = ((W as f64 - 2.0 * PAD) / (x1 - x0).max(1e-12))
        .min((H as f64 - 2.0 * PAD) / (y1 - y0).max(1e-12))
        .min(256.0 * 2f64.powi(15));
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let unit = |px: f64, py: f64| {
        (
            cx + (px - W as f64 / 2.0) / s,
            cy + (py - H as f64 / 2.0) / s,
        )
    };
    let to_px = |(x, y): (f64, f64)| ((x - cx) * s + W as f64 / 2.0, (y - cy) * s + H as f64 / 2.0);

    // Terrain: the tile zoom at or just above the image's resolution, fetched in parallel.
    let z = (s / TILE).log2().ceil().clamp(0.0, 14.0) as u8;
    let n = f64::from(1u32 << z);
    let (u0, v0) = unit(-1.0, -1.0);
    let (u1, v1) = unit(W as f64 + 1.0, H as f64 + 1.0);
    let keys: Vec<(u32, u32)> = ((u0 * n).floor() as u32..=(u1 * n).floor() as u32)
        .flat_map(|x| ((v0 * n).floor() as u32..=(v1 * n).floor() as u32).map(move |y| (x, y)))
        .collect();
    let tiles: HashMap<(u32, u32), Vec<f32>> = keys
        .par_iter()
        .filter_map(|&(x, y)| terrain(z, x, y).map(|t| ((x, y), t)))
        .collect();
    let elev = |px: f64, py: f64| -> f32 {
        let (u, v) = unit(px, py);
        let (tx, ty) = (u * n, v * n);
        let Some(t) = tiles.get(&(tx as u32, ty as u32)) else {
            return f32::NAN;
        };
        let i =
            ((ty.fract() * TILE) as usize).min(511) * 512 + ((tx.fract() * TILE) as usize).min(511);
        t[i]
    };
    let e: Vec<f32> = (0..(W + 2) * (H + 2))
        .into_par_iter()
        .map(|i| elev((i % (W + 2)) as f64 - 1.0, (i / (W + 2)) as f64 - 1.0))
        .collect();

    // Sun from the north-west at 45°, as MapLibre's hillshade.
    let alt = 45f64.to_radians();
    let sun = [
        -(alt.cos()) * 0.5f64.sqrt(),
        alt.cos() * 0.5f64.sqrt(),
        alt.sin(),
    ];
    let mut rgb = vec![0u8; W * H * 3];
    rgb.par_chunks_mut(W * 3).enumerate().for_each(|(py, row)| {
        let lat = unmerc_lat(unit(0.0, py as f64 + 0.5).1);
        let mpp = 40_075_016.686 * lat.to_radians().cos() / s;
        for px in 0..W {
            let mut c = LAND;
            let at = |dx: isize, dy: isize| {
                e[(py as isize + 1 + dy) as usize * (W + 2) + (px as isize + 1 + dx) as usize]
                    as f64
            };
            let (gx, gn) = (
                (at(1, 0) - at(-1, 0)) / (2.0 * mpp),
                (at(0, -1) - at(0, 1)) / (2.0 * mpp),
            );
            if gx.is_finite() && gn.is_finite() {
                let len = (gx * gx + gn * gn + 1.0).sqrt();
                let i = (-gx * sun[0] - gn * sun[1] + sun[2]) / len;
                let flat = sun[2];
                if i < flat {
                    blend(&mut c, SHADE, ((flat - i) / flat * 0.6) as f32);
                } else {
                    blend(&mut c, [255.0; 3], ((i - flat) / (1.0 - flat) * 0.5) as f32);
                }
            }
            if let Some(v) = view {
                let lon = unit(px as f64 + 0.5, 0.0).0 * 360.0 - 180.0;
                if let Some(score) = v.at_wgs84(lat, lon) {
                    let r = ramp(score);
                    blend(
                        &mut c,
                        [r[0] as f32, r[1] as f32, r[2] as f32],
                        r[3] as f32 / 255.0,
                    );
                }
            }
            row[px * 3..px * 3 + 3].copy_from_slice(&c.map(|x| x.round() as u8));
        }
    });

    let line: Vec<(f64, f64)> = pts.into_iter().map(to_px).collect();
    stroke(&mut rgb, &line, 5.5, CASING);
    stroke(&mut rgb, &line, 3.25, ROUTE);

    Some(rgb)
}

/// `render` as a JPEG: ~140 kB where a PNG is ~1 MB, over what chat apps fetch for a preview.
pub fn render_jpeg(
    coords: &[[f64; 2]],
    view: Option<&ViewGrid>,
    terrain: &Terrain,
) -> Option<Vec<u8>> {
    let rgb = render(coords, view, terrain)?;
    let mut buf = Vec::new();
    jpeg_encoder::Encoder::new(&mut buf, 85)
        .encode(&rgb, W as u16, H as u16, jpeg_encoder::ColorType::Rgb)
        .ok()?;
    Some(buf)
}

fn blend(c: &mut [f32; 3], over: [f32; 3], a: f32) {
    let a = a.clamp(0.0, 1.0);
    for k in 0..3 {
        c[k] += (over[k] - c[k]) * a;
    }
}

/// An antialiased polyline with round joins: per pixel the best coverage over all segments, blended once.
fn stroke(rgb: &mut [u8], line: &[(f64, f64)], half: f64, color: [f32; 3]) {
    let mut cov = vec![0f32; W * H];
    let segs = line
        .windows(2)
        .map(|w| (w[0], w[1]))
        .chain(line.first().map(|&p| (p, p)));
    for (a, b) in segs {
        let r = half + 1.0;
        let (xa, xb) = (
            (a.0.min(b.0) - r).max(0.0) as usize,
            (a.0.max(b.0) + r).min(W as f64 - 1.0).max(0.0) as usize,
        );
        let (ya, yb) = (
            (a.1.min(b.1) - r).max(0.0) as usize,
            (a.1.max(b.1) + r).min(H as f64 - 1.0).max(0.0) as usize,
        );
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l2 = dx * dx + dy * dy;
        for y in ya..=yb {
            for x in xa..=xb {
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                let t = if l2 > 0.0 {
                    (((px - a.0) * dx + (py - a.1) * dy) / l2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let d = ((px - a.0 - t * dx).powi(2) + (py - a.1 - t * dy).powi(2)).sqrt();
                let c = (half + 0.5 - d).clamp(0.0, 1.0) as f32;
                let k = &mut cov[y * W + x];
                *k = k.max(c);
            }
        }
    }
    for (i, &a) in cov.iter().enumerate() {
        if a > 0.0 {
            let mut c = [
                rgb[i * 3] as f32,
                rgb[i * 3 + 1] as f32,
                rgb[i * 3 + 2] as f32,
            ];
            blend(&mut c, color, a);
            rgb[i * 3..i * 3 + 3].copy_from_slice(&c.map(|x| x.round() as u8));
        }
    }
}

/// One Mapterhorn terrarium tile as elevations. Blocking.
pub fn fetch_terrain(z: u8, x: u32, y: u32) -> Option<Vec<f32>> {
    let res = ureq::get(&format!("{TERRAIN}/{z}/{x}/{y}.webp"))
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .ok()?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(
        &mut std::io::Read::take(res.into_reader(), 4 << 20),
        &mut bytes,
    )
    .ok()?;
    let mut dec = image_webp::WebPDecoder::new(std::io::Cursor::new(bytes)).ok()?;
    if dec.dimensions() != (512, 512) {
        return None;
    }
    let ch = if dec.has_alpha() { 4 } else { 3 };
    let mut px = vec![0u8; dec.output_buffer_size()?];
    dec.read_image(&mut px).ok()?;
    Some(
        px.chunks_exact(ch)
            .map(|p| p[0] as f32 * 256.0 + p[1] as f32 + p[2] as f32 / 256.0 - 32768.0)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_the_loop_in_red_over_shaded_land() {
        // A cone, so every slope direction shows up.
        let terrain = |z: u8, x: u32, y: u32| -> Option<Vec<f32>> {
            let n = f64::from(1u32 << z);
            let (cx, cy) = merc(46.9, 7.4);
            Some(
                (0..512 * 512)
                    .map(|i| {
                        let u = (f64::from(x) + (i % 512) as f64 / 512.0) / n;
                        let v = (f64::from(y) + (i / 512) as f64 / 512.0) / n;
                        (3000.0 - ((u - cx).hypot(v - cy)) * 4e7) as f32
                    })
                    .collect(),
            )
        };
        let lp = [
            [7.38, 46.89],
            [7.42, 46.89],
            [7.42, 46.91],
            [7.38, 46.91],
            [7.38, 46.89],
        ];
        let buf = render(&lp, None, &terrain).unwrap();
        assert!(render_jpeg(&lp, None, &terrain)
            .unwrap()
            .starts_with(&[0xFF, 0xD8]));
        let px = |x: usize, y: usize| &buf[(y * W + x) * 3..(y * W + x) * 3 + 3];
        // The loop's left edge sits on the image's vertical middle.
        let (lx, _) = {
            let s = (H as f64 - 2.0 * PAD) / (merc(46.89, 7.38).1 - merc(46.91, 7.38).1);
            let w = (merc(46.9, 7.42).0 - merc(46.9, 7.38).0) * s;
            ((W as f64 / 2.0 - w / 2.0) as usize, 0)
        };
        assert_eq!(px(lx, H / 2), &[213, 40, 27]);
        // NW slopes are lit and SE slopes shaded, both away from plain land.
        let (nw, se) = (px(W / 2 - 40, H / 2 - 40), px(W / 2 + 40, H / 2 + 40));
        assert!(nw[0] > se[0] + 20, "{nw:?} {se:?}");
    }
}
