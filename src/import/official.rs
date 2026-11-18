//! Official hiking trails (swissTLM3D Wanderwege GeoPackage, LV95) matched
//! onto graph edges.

use std::path::Path;

use rayon::prelude::*;
use rstar::{PointDistance, RTree};
use rusqlite::{Connection, OpenFlags};

use super::{sample_lv95, ImportError, Seg};
use crate::graph::GraphWriter;

#[derive(Debug, Clone)]
pub struct OfficialLine {
    pub lv95: Vec<[f64; 2]>,
    pub category: u8,
}

pub const OFFICIAL_TOLERANCE_M: f64 = 20.0;
const SAMPLE_SPACING_M: f64 = 10.0;
const TABLE: &str = "tlm_strassen_strasse";
const RTREE: &str = "rtree_tlm_strassen_strasse_geom";

fn category(v: &str) -> u8 {
    match v {
        "Bergwanderweg" => 2,
        "Alpinwanderweg" => 3,
        _ => 1,
    }
}

/// Read lines whose bbox intersects `clip_lv95` ([min_e, min_n, max_e, max_n]) — filter in SQL via the
/// rtree table if present (`rtree_tlm_strassen_strasse_geom`), else in Rust.
pub fn read_official_gpkg(
    path: &Path,
    clip_lv95: Option<[f64; 4]>,
) -> Result<Vec<OfficialLine>, ImportError> {
    let sql_err = |e: rusqlite::Error| ImportError::Format(format!("{}: {e}", path.display()));
    if !path.is_file() {
        return Err(ImportError::Format(format!(
            "{}: no such file",
            path.display()
        )));
    }
    let db =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(sql_err)?;
    let has_rtree: bool = db
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = ?1",
            [RTREE],
            |r| r.get::<_, i64>(0),
        )
        .map_err(sql_err)?
        > 0;
    let wanted = "t.wanderwege IS NOT NULL AND t.wanderwege <> ''";
    let (sql, params) = match clip_lv95 {
        Some(c) if has_rtree => (
            format!(
                "SELECT t.geom, t.wanderwege FROM {TABLE} t JOIN {RTREE} r ON r.id = t.id \
                 WHERE {wanted} AND r.maxx >= ?1 AND r.minx <= ?3 AND r.maxy >= ?2 AND r.miny <= ?4"
            ),
            c.to_vec(),
        ),
        _ => (
            format!("SELECT t.geom, t.wanderwege FROM {TABLE} t WHERE {wanted}"),
            vec![],
        ),
    };
    let mut stmt = db.prepare(&sql).map_err(sql_err)?;
    let mut rows = stmt
        .query(rusqlite::params_from_iter(params))
        .map_err(sql_err)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(sql_err)? {
        let Some(blob) = row.get::<_, Option<Vec<u8>>>(0).map_err(sql_err)? else {
            continue;
        };
        let cat = category(&row.get::<_, String>(1).map_err(sql_err)?);
        let lines = parse_gpkg_geometry(&blob)
            .ok_or_else(|| ImportError::Format(format!("{}: bad geometry blob", path.display())))?;
        for lv95 in lines {
            let hit = clip_lv95.is_none_or(|c| bbox_overlaps(&lv95, c));
            if lv95.len() >= 2 && hit {
                out.push(OfficialLine {
                    lv95,
                    category: cat,
                });
            }
        }
    }
    Ok(out)
}

fn bbox_overlaps(pts: &[[f64; 2]], [x0, y0, x1, y1]: [f64; 4]) -> bool {
    let (mut a, mut b) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in pts {
        a = [a[0].min(p[0]), a[1].min(p[1])];
        b = [b[0].max(p[0]), b[1].max(p[1])];
    }
    b[0] >= x0 && a[0] <= x1 && b[1] >= y0 && a[1] <= y1
}

/// Bounds-checked little/big-endian reader over a byte slice.
struct Rd<'a> {
    b: &'a [u8],
    pos: usize,
}

impl Rd<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.pos..self.pos.checked_add(n)?)?;
        self.pos += n;
        Some(s)
    }
    fn u32(&mut self, le: bool) -> Option<u32> {
        let a: [u8; 4] = self.take(4)?.try_into().ok()?;
        Some(if le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    }
    fn f64(&mut self, le: bool) -> Option<f64> {
        let a: [u8; 8] = self.take(8)?.try_into().ok()?;
        Some(if le {
            f64::from_le_bytes(a)
        } else {
            f64::from_be_bytes(a)
        })
    }
}

/// GPKG geometry blob → LV95 polylines (one per LineString, several for a
/// MultiLineString). None on a malformed blob or another geometry type.
fn parse_gpkg_geometry(blob: &[u8]) -> Option<Vec<Vec<[f64; 2]>>> {
    let mut r = Rd { b: blob, pos: 0 };
    if r.take(2)? != b"GP" {
        return None;
    }
    let flags = r.take(2)?[1];
    let envelope = [0, 32, 48, 48, 64].get(((flags >> 1) & 7) as usize)?;
    r.take(4 + envelope)?;
    let mut out = Vec::new();
    wkb(&mut r, &mut out, true)?;
    Some(out)
}

fn wkb(r: &mut Rd, out: &mut Vec<Vec<[f64; 2]>>, allow_multi: bool) -> Option<()> {
    let le = r.take(1)?[0] == 1;
    let t = r.u32(le)?;
    let (ewkb_z, ewkb_m) = (t & 0x8000_0000 != 0, t & 0x4000_0000 != 0);
    let t = t & 0x0FFF_FFFF;
    let (base, iso) = (t % 1000, t / 1000);
    let dims = 2
        + usize::from(ewkb_z || iso == 1 || iso == 3)
        + usize::from(ewkb_m || iso == 2 || iso == 3);
    let n = r.u32(le)? as usize;
    match base {
        2 => {
            // Reject counts the blob cannot hold before allocating.
            if n.checked_mul(dims * 8)? > r.b.len() - r.pos {
                return None;
            }
            let mut line = Vec::with_capacity(n);
            for _ in 0..n {
                let (e, nn) = (r.f64(le)?, r.f64(le)?);
                r.take((dims - 2) * 8)?;
                line.push([e, nn]);
            }
            out.push(line);
        }
        5 if allow_multi => {
            for _ in 0..n {
                wkb(r, out, false)?;
            }
        }
        _ => return None,
    }
    Some(())
}

/// An edge is official when ≥ 50 % of its sample points (every ~10 m along its geometry, endpoints
/// included) lie within tolerance of some official line (R-tree of LV95 line segments); it takes the
/// category of the line nearest to the majority of its matching samples. Returns edges marked.
pub fn apply_official(w: &mut GraphWriter, lines: &[OfficialLine], tolerance_m: f64) -> u64 {
    let segs: Vec<Seg> = lines
        .iter()
        .enumerate()
        .flat_map(|(i, l)| {
            l.lv95.windows(2).map(move |s| Seg {
                a: s[0],
                b: s[1],
                id: i as u32,
            })
        })
        .collect();
    let tree = RTree::bulk_load(segs);
    let tol2 = tolerance_m * tolerance_m;
    let writer = &*w;
    let cats: Vec<u8> = (0..writer.edge_count() as u32)
        .into_par_iter()
        .map(|e| {
            let samples = sample_lv95(writer.edge_geometry(e), SAMPLE_SPACING_M);
            // Votes per category (index 1..=3) of the nearest line within tolerance.
            let mut votes = [0usize; 4];
            for p in &samples {
                if let Some(s) = tree.nearest_neighbor(p).filter(|s| s.distance_2(p) <= tol2) {
                    votes[lines[s.id as usize].category.min(3) as usize] += 1;
                }
            }
            let matched: usize = votes.iter().sum();
            if samples.is_empty() || matched * 2 < samples.len() {
                return 0;
            }
            // Ties go to the lower category.
            (1..=3u8)
                .max_by_key(|&c| (votes[c as usize], std::cmp::Reverse(c)))
                .unwrap_or(0)
        })
        .collect();
    let mut marked = 0;
    for (e, c) in cats.into_iter().enumerate() {
        if c > 0 {
            w.edge_attrs_mut(e as u32).official_category = c;
            marked += 1;
        }
    }
    marked
}
