//! swissALTI3D 2 m Cloud-Optimized GeoTIFF tiles (1 km × 1 km, LV95), read in
//! their native grid with the `tiff` crate and decoded lazily.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tiff::decoder::{ChunkType, Decoder};
use tiff::tags::{ByteOrder, Tag};
use tiff::ColorType;

use super::dem::{bilinear, ElevationSource, DEM_ALTI3D};
use super::ImportError;

const CACHE_TILES: usize = 64;
/// Real tiles are 500 px (2 m) or 2000 px (0.5 m).
const MAX_TILE_PX: usize = 4096;
const MAX_CHUNK_BYTES: usize = 64 << 20;

/// Tile key: south-west corner in km (E, N).
type Key = (u32, u32);
/// (use counter, key → (last use, tile; None if it failed to load)).
type Cache = (u64, HashMap<Key, (u64, Option<Arc<Tile>>)>);

pub struct Alti3dTiles {
    paths: HashMap<Key, PathBuf>,
    cache: Mutex<Cache>,
    decodes: AtomicU64,
}

struct Tile {
    size: usize,
    cell: f64,
    nodata: f32,
    /// Row-major, north row first.
    z: Vec<f32>,
}

impl Tile {
    /// Pixel containing the point `dx` east and `dy` south of the tile's
    /// north-west corner.
    fn pixel(&self, dx: f64, dy: f64) -> Option<f32> {
        let idx = |v: f64| ((v / self.cell).floor().max(0.0) as usize).min(self.size - 1);
        let v = self.z[idx(dy) * self.size + idx(dx)];
        (v != self.nodata && v.is_finite()).then_some(v)
    }
}

/// `swissalti3d_<year>_<E>-<N>_2_2056_5728.tif` → (year, key).
fn parse_name(name: &str) -> Option<(u32, Key)> {
    let rest = name
        .strip_prefix("swissalti3d_")?
        .strip_suffix("_2_2056_5728.tif")?;
    let (year, en) = rest.split_once('_')?;
    let (e, n) = en.split_once('-')?;
    Some((year.parse().ok()?, (e.parse().ok()?, n.parse().ok()?)))
}

fn key_of(e: f64, n: f64) -> Option<Key> {
    let k = |v: f64| u32::try_from((v / 1000.0).floor() as i64).ok();
    Some((k(e)?, k(n)?))
}

fn load_tile(path: &Path, (e_km, n_km): Key) -> Result<Tile, ImportError> {
    let bad = |m: String| ImportError::Format(format!("{}: {m}", path.display()));
    let tiff = |e: tiff::TiffError| bad(e.to_string());
    let mut dec = Decoder::new(BufReader::new(File::open(path)?)).map_err(tiff)?;
    let (w, h) = dec.dimensions().map_err(tiff)?;
    if w != h || w == 0 {
        return Err(bad(format!("expected a square tile, got {w}×{h}")));
    }
    let size = w as usize;
    let cell = 1000.0 / w as f64;
    if let Some(tp) = dec.find_tag(Tag::ModelTiepointTag).map_err(tiff)? {
        let tp = tp.into_f64_vec().map_err(tiff)?;
        let (west, north) = (e_km as f64 * 1000.0, (n_km as f64 + 1.0) * 1000.0);
        if tp.len() < 6
            || (tp[3] - tp[0] * cell - west).abs() > 0.5
            || (tp[4] + tp[1] * cell - north).abs() > 0.5
        {
            return Err(bad(format!(
                "tiepoint {tp:?} does not match tile {e_km}-{n_km}"
            )));
        }
    }
    let nodata = match dec.find_tag(Tag::GdalNodata).map_err(tiff)? {
        Some(v) => {
            let s = v.into_string().map_err(tiff)?;
            let s = s.trim_matches(|c: char| c == '\0' || c.is_whitespace());
            s.parse()
                .map_err(|_| bad(format!("bad GDAL_NODATA {s:?}")))?
        }
        None => -9999.0,
    };
    let z = read_band(&mut dec, size).map_err(bad)?;
    Ok(Tile {
        size,
        cell,
        nodata,
        z,
    })
}

/// The full-resolution float32 band, decoded chunk by chunk. tiff 0.11.3's
/// own LZW reader fails on some valid padded tiles ("no lzw end code found":
/// it drops output that arrives together with end of input), so the chunks
/// go to weezl directly, which also accepts a stream without an end code.
fn read_band(dec: &mut Decoder<BufReader<File>>, size: usize) -> Result<Vec<f32>, String> {
    let s = |e: tiff::TiffError| e.to_string();
    let float = dec.find_tag_unsigned::<u16>(Tag::SampleFormat).map_err(s)? == Some(3);
    if dec.colortype().map_err(s)? != ColorType::Gray(32) || !float {
        return Err("expected one float32 band".into());
    }
    let lzw = match dec.get_tag_unsigned::<u16>(Tag::Compression).map_err(s)? {
        1 => false,
        5 => true,
        c => return Err(format!("unsupported compression {c}")),
    };
    match dec.find_tag_unsigned::<u16>(Tag::Predictor).map_err(s)? {
        None | Some(1) => {}
        Some(p) => return Err(format!("unsupported predictor {p}")),
    }
    let (offs_tag, counts_tag) = match dec.get_chunk_type() {
        ChunkType::Tile => (Tag::TileOffsets, Tag::TileByteCounts),
        ChunkType::Strip => (Tag::StripOffsets, Tag::StripByteCounts),
    };
    let offsets = dec.get_tag_u64_vec(offs_tag).map_err(s)?;
    let counts = dec.get_tag_u64_vec(counts_tag).map_err(s)?;
    let (cw, ch) = dec.chunk_dimensions();
    let (cw, ch) = (cw as usize, ch as usize);
    // Header sizes are untrusted: bound the allocations below before making them.
    if size > MAX_TILE_PX {
        return Err(format!("tile of {size} px exceeds {MAX_TILE_PX}"));
    }
    cw.checked_mul(ch)
        .and_then(|v| v.checked_mul(4))
        .filter(|&b| b > 0 && b <= MAX_CHUNK_BYTES)
        .ok_or_else(|| format!("chunk of {cw}×{ch} px out of range"))?;
    let across = size.div_ceil(cw);
    let chunks = across * size.div_ceil(ch);
    if offsets.len() < chunks || counts.len() < chunks {
        return Err(format!(
            "{} chunk offsets for {chunks} chunks",
            offsets.len()
        ));
    }
    let little = dec.byte_order() == ByteOrder::LittleEndian;
    let mut z = vec![0.0f32; size * size];
    let mut raw = Vec::new();
    for i in 0..chunks {
        let (x0, y0) = (i % across * cw, i / across * ch);
        let rows = ch.min(size - y0);
        // Tiles are padded to `cw` columns; strips are exactly the image width.
        let need = rows * cw * 4;
        let file = dec.inner();
        file.seek(SeekFrom::Start(offsets[i]))
            .map_err(|e| e.to_string())?;
        raw.clear();
        file.take(counts[i])
            .read_to_end(&mut raw)
            .map_err(|e| e.to_string())?;
        let bytes = if lzw {
            // Fixed-size output bounds a malformed stream's expansion.
            let mut out = vec![0u8; need];
            let mut d = weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8);
            let (mut ip, mut op) = (0, 0);
            while op < need {
                let r = d.decode_bytes(&raw[ip..], &mut out[op..]);
                ip += r.consumed_in;
                op += r.consumed_out;
                match r.status {
                    Ok(weezl::LzwStatus::Ok) if r.consumed_in + r.consumed_out > 0 => {}
                    Ok(_) => break,
                    Err(e) => return Err(format!("chunk {i}: LZW {e}")),
                }
            }
            out.truncate(op);
            out
        } else {
            std::mem::take(&mut raw)
        };
        if bytes.len() < need {
            return Err(format!("chunk {i}: {} of {need} bytes", bytes.len()));
        }
        for r in 0..rows {
            for c in 0..cw.min(size - x0) {
                let k = (r * cw + c) * 4;
                let b = [bytes[k], bytes[k + 1], bytes[k + 2], bytes[k + 3]];
                z[(y0 + r) * size + x0 + c] = if little {
                    f32::from_le_bytes(b)
                } else {
                    f32::from_be_bytes(b)
                };
            }
        }
    }
    Ok(z)
}

impl Alti3dTiles {
    /// Scans `dir` for tiles; the newest year wins per tile.
    pub fn open(dir: &Path) -> Result<Alti3dTiles, ImportError> {
        let mut best: HashMap<Key, (u32, PathBuf)> = HashMap::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let Some((year, key)) = entry.file_name().to_str().and_then(parse_name) else {
                continue;
            };
            if best.get(&key).is_none_or(|(y, _)| year > *y) {
                best.insert(key, (year, entry.path()));
            }
        }
        if best.is_empty() {
            return Err(ImportError::Format(format!(
                "no swissALTI3D tiles in {}",
                dir.display()
            )));
        }
        Ok(Alti3dTiles {
            paths: best.into_iter().map(|(k, (_, p))| (k, p)).collect(),
            cache: Mutex::new((0, HashMap::new())),
            decodes: AtomicU64::new(0),
        })
    }

    pub fn tile_count(&self) -> usize {
        self.paths.len()
    }

    /// Tile decodes so far, re-decodes after cache eviction included.
    pub fn decode_count(&self) -> u64 {
        self.decodes.load(Ordering::Relaxed)
    }

    /// Cached tile; decoded outside the lock so other threads keep sampling.
    fn tile(&self, key: Key) -> Option<Arc<Tile>> {
        let path = self.paths.get(&key)?;
        {
            let mut guard = self.cache.lock().unwrap_or_else(|p| p.into_inner());
            let (tick, map) = &mut *guard;
            *tick += 1;
            if let Some(entry) = map.get_mut(&key) {
                entry.0 = *tick;
                return entry.1.clone();
            }
        }
        self.decodes.fetch_add(1, Ordering::Relaxed);
        let tile = match load_tile(path, key) {
            Ok(t) => Some(Arc::new(t)),
            Err(err) => {
                tracing::warn!(%err, "skipping swissALTI3D tile");
                None
            }
        };
        let mut guard = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        let (tick, map) = &mut *guard;
        if map.len() >= CACHE_TILES {
            if let Some(old) = map.iter().min_by_key(|(_, v)| v.0).map(|(k, _)| *k) {
                map.remove(&old);
            }
        }
        *tick += 1;
        map.insert(key, (*tick, tile.clone()));
        tile
    }
}

impl ElevationSource for Alti3dTiles {
    /// Bilinear between pixel centres, across tile seams by reading the
    /// neighbouring tile; missing corners fall back like `AscGrid`.
    fn sample_lv95(&self, e: f64, n: f64) -> Option<f32> {
        let home_key = key_of(e, n)?;
        let home = self.tile(home_key)?;
        let cell = home.cell;
        let (gx, gy) = (e / cell - 0.5, n / cell - 0.5);
        let (x0, y0) = (gx.floor(), gy.floor());
        let corner = |x: f64, y: f64| {
            let (ce, cn) = ((x + 0.5) * cell, (y + 0.5) * cell);
            let key = key_of(ce, cn)?;
            let t = if key == home_key {
                home.clone()
            } else {
                self.tile(key)?
            };
            t.pixel(
                ce - key.0 as f64 * 1000.0,
                (key.1 as f64 + 1.0) * 1000.0 - cn,
            )
        };
        bilinear(
            [
                corner(x0, y0),
                corner(x0 + 1.0, y0),
                corner(x0, y0 + 1.0),
                corner(x0 + 1.0, y0 + 1.0),
            ],
            gx - x0,
            gy - y0,
        )
    }

    fn source_id(&self) -> u8 {
        DEM_ALTI3D
    }
}
