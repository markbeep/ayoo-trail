use std::path::Path;

use navilau::graph::{flags, EdgeAttrs, Graph, GraphWriter, LatLon, GRADE_BANDS};
use navilau::import::alti3d::Alti3dTiles;
use navilau::import::ch1903::{lv95_to_wgs84, wgs84_to_lv95};
use navilau::import::dem::{apply_dem, grade_band, AscGrid, ElevationSource, DEM_ASC};

#[test]
fn projects_the_bern_reference_point() {
    let (e, n) = wgs84_to_lv95(46.951082877, 7.438632495);
    assert!((e - 2_600_000.0).abs() < 2.0);
    assert!((n - 1_200_000.0).abs() < 2.0);
}

#[test]
fn projection_round_trips_away_from_the_origin() {
    let (e, n) = wgs84_to_lv95(46.90, 7.40);
    let (lat, lon) = lv95_to_wgs84(e, n);
    assert!((lat - 46.90).abs() < 1e-6);
    assert!((lon - 7.40).abs() < 1e-6);
}

#[test]
fn bilinear_samples_the_native_grid() {
    let dem = AscGrid::from_asc(Path::new("tests/fixtures/tiny.asc")).unwrap();
    assert!((dem.sample_grid(2_600_000.0, 1_200_000.0).unwrap() - 210.0).abs() < 1.0);
    // Halfway between the 210 and 220 grid points.
    assert!((dem.sample_grid(2_605_000.0, 1_200_000.0).unwrap() - 215.0).abs() < 1e-3);
    assert_eq!(dem.sample_grid(2_700_000.0, 1_200_000.0), None);
}

fn write_asc(dir: &Path, header: &str, rows: &str) -> std::path::PathBuf {
    let p = dir.join("g.asc");
    std::fs::write(&p, format!("{header}{rows}")).unwrap();
    p
}

#[test]
fn lv03_grid_is_detected() {
    let dir = tempfile::tempdir().unwrap();
    let p = write_asc(
        dir.path(),
        "ncols 3\nnrows 3\nxllcenter 590000\nyllcenter 190000\ncellsize 10000\nNODATA_value -9999\n",
        "100 110 120\n200 210 220\n300 310 320\n",
    );
    let lv03 = AscGrid::from_asc(&p).unwrap();
    let lv95 = AscGrid::from_asc(Path::new("tests/fixtures/tiny.asc")).unwrap();
    for (e, n) in [(2_600_000.0, 1_200_000.0), (2_603_000.0, 1_196_500.0)] {
        assert_eq!(lv03.sample_grid(e, n), lv95.sample_grid(e, n));
    }
}

#[test]
fn nodata_corner_falls_back() {
    let dir = tempfile::tempdir().unwrap();
    let p = write_asc(
        dir.path(),
        "ncols 2\nnrows 2\nxllcorner 2600000\nyllcorner 1200000\ncellsize 10\nNODATA_value -9999\n",
        "-9999 110\n200 210\n",
    );
    let g = AscGrid::from_asc(&p).unwrap();
    // Cell centres at E 2600005/2600015, N 1200005/1200015. Near the NODATA
    // (north-west) corner the nearest valid corner wins.
    let v = g.sample_grid(2_600_006.0, 1_200_014.0).unwrap();
    assert!((110.0..=210.0).contains(&v), "got {v}");
    // All four valid → ordinary bilinear.
    assert!((g.sample_grid(2_600_010.0, 1_200_005.0).unwrap() - 205.0).abs() < 1e-3);

    let all = write_asc(
        dir.path(),
        "ncols 1\nnrows 1\nxllcorner 2600000\nyllcorner 1200000\ncellsize 10\nNODATA_value -9999\n",
        "-9999\n",
    );
    assert_eq!(
        AscGrid::from_asc(&all)
            .unwrap()
            .sample_grid(2_600_005.0, 1_200_005.0),
        None
    );
}

#[test]
fn malformed_asc_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let p = write_asc(dir.path(), "ncols 3\nnrows 3\ncellsize 10\n", "1 2 3\n");
    assert!(AscGrid::from_asc(&p).is_err());
}

#[test]
fn apply_dem_sets_z_and_slope() {
    let dem = AscGrid::from_asc(Path::new("tests/fixtures/tiny.asc")).unwrap();
    let mut w = GraphWriter::new(0);
    let a = w.add_node(
        LatLon {
            lat: 46.95,
            lon: 7.44,
        },
        0.0,
    );
    let b = w.add_node(
        LatLon {
            lat: 46.99,
            lon: 7.44,
        },
        0.0,
    );
    w.add_edge(
        a,
        b,
        EdgeAttrs::default(),
        &[(46.95, 7.44, 0.0), (47.00, 7.44, 0.0)],
    );
    let stats = apply_dem(&mut w, &dem);
    assert_eq!(stats.edges, 1);
    let g = Graph::from_bytes(w.finish()).unwrap();
    assert!(g.edge_attrs(0).gain + g.edge_attrs(0).loss > 50);
    assert!(g.edge_geometry(0).last().unwrap().2 > 0.0);
    assert!(g.node_elev(a) > 0.0 && g.node_elev(b) > 0.0);
    assert_eq!(g.dem_source(), DEM_ASC);
}

/// z = f(E, N) in LV95.
struct Surface<F>(F);

impl<F: Fn(f64, f64) -> f32 + Sync> ElevationSource for Surface<F> {
    fn sample_lv95(&self, e: f64, n: f64) -> Option<f32> {
        Some((self.0)(e, n))
    }
    fn source_id(&self) -> u8 {
        9
    }
}

const E0: f64 = 2_600_000.0;
const N0: f64 = 1_200_000.0;

/// A writer with one edge through the given LV95 vertices.
fn edge_writer(lv95: &[(f64, f64)], edge_flags: u8) -> GraphWriter {
    let geom: Vec<(f64, f64, f32)> = lv95
        .iter()
        .map(|&(e, n)| {
            let (lat, lon) = lv95_to_wgs84(e, n);
            (lat, lon, 0.0)
        })
        .collect();
    let mut w = GraphWriter::new(0);
    let first = geom[0];
    let last = geom[geom.len() - 1];
    let a = w.add_node(
        LatLon {
            lat: first.0,
            lon: first.1,
        },
        0.0,
    );
    let b = w.add_node(
        LatLon {
            lat: last.0,
            lon: last.1,
        },
        0.0,
    );
    let attrs = EdgeAttrs {
        flags: edge_flags,
        ..EdgeAttrs::default()
    };
    w.add_edge(a, b, attrs, &geom);
    w
}

fn straight_1km() -> GraphWriter {
    edge_writer(&[(E0, N0), (E0 + 1000.0, N0)], 0)
}

#[test]
fn grade_band_edges_go_towards_flat() {
    assert_eq!(grade_band(-30.0), 0);
    assert_eq!(grade_band(-25.0), 1);
    assert_eq!(grade_band(-8.0), 4);
    assert_eq!(grade_band(0.0), 4);
    assert_eq!(grade_band(8.0), 4);
    assert_eq!(grade_band(12.0), 5);
    assert_eq!(grade_band(25.0), 7);
    assert_eq!(grade_band(40.0), 8);
}

#[test]
fn uniform_slope_lands_in_one_band() {
    let mut w = straight_1km();
    apply_dem(
        &mut w,
        &Surface(|e: f64, _: f64| (500.0 + 0.12 * (e - E0)) as f32),
    );
    let a = w.edge_attrs(0);
    let mut want = [0u8; GRADE_BANDS];
    want[5] = 255;
    assert_eq!(a.grade, want);
    assert!((a.gain - 120).abs() <= 1, "gain {}", a.gain);
    assert_eq!(a.loss, 0);
}

#[test]
fn half_flat_half_steep_histogram() {
    let mut w = straight_1km();
    let dem = Surface(|e: f64, _: f64| {
        let s = e - E0;
        (800.0 - if s > 500.0 { 0.3 * (s - 500.0) } else { 0.0 }) as f32
    });
    apply_dem(&mut w, &dem);
    let g = w.edge_attrs(0).grade;
    // One ~30 m window straddles the break, so allow 3 shares plus that window
    // (30 / 1000 * 255 ≈ 8) on either side.
    assert!((g[4] as i32 - 127).abs() <= 11, "{g:?}");
    assert!((g[0] as i32 - 127).abs() <= 11, "{g:?}");
    assert_eq!(g[0] as u32 + g[3] as u32 + g[4] as u32, 255, "{g:?}");
    assert!((w.edge_attrs(0).loss - 150).abs() <= 1);
}

#[test]
fn bridge_interpolates_between_endpoints() {
    let mut w = edge_writer(
        &[(E0, N0), (E0 + 500.0, N0), (E0 + 1000.0, N0)],
        flags::BRIDGE,
    );
    // A 200 m deep valley, ends at 700 and 710 m.
    let dem = Surface(|e: f64, _: f64| {
        let s = e - E0;
        (700.0 + 0.01 * s - 200.0 * (1.0 - ((s - 500.0) / 500.0).abs())) as f32
    });
    apply_dem(&mut w, &dem);
    let a = w.edge_attrs(0);
    assert_eq!((a.gain, a.loss), (10, 0));
    let mid = w.edge_geometry(0)[1].2;
    assert!((mid - 705.0).abs() < 0.5, "mid z {mid}");
    assert_eq!(a.grade, navilau::graph::FLAT_GRADE);
}

#[test]
fn densify_adds_vertex_at_hidden_peak() {
    let mut w = straight_1km();
    let ridge = Surface(|e: f64, _: f64| (600.0 + 100.0 - 0.2 * (e - E0 - 500.0).abs()) as f32);
    let stats = apply_dem(&mut w, &ridge);
    let geom = w.edge_geometry(0);
    assert!(geom.len() >= 3 && stats.vertices_added >= 1);
    let peak = geom.iter().map(|v| v.2).fold(f32::MIN, f32::max);
    assert!((peak - 700.0).abs() < 5.0, "peak {peak}");
    assert!((geom[0].2 - 600.0).abs() < 0.5);

    let mut flat = straight_1km();
    let stats = apply_dem(
        &mut flat,
        &Surface(|e: f64, _: f64| (500.0 + 0.1 * (e - E0)) as f32),
    );
    assert_eq!(stats.vertices_added, 0);
    assert_eq!(flat.edge_geometry(0).len(), 2);
}

#[test]
fn shares_sum_to_255() {
    let profiles: [fn(f64) -> f64; 5] = [
        |s| (s / 37.0).sin() * 9.0,
        |s| (s * 0.31) % 17.0,
        |s| -0.45 * s + (s / 11.0).cos() * 3.0,
        |s| {
            if (s as i64 / 60) % 2 == 0 {
                0.2 * s
            } else {
                -0.17 * s
            }
        },
        |s| (s / 5.0).sin() * 40.0,
    ];
    for (i, f) in profiles.iter().enumerate() {
        let len = 130.0 + 377.0 * i as f64;
        let mut w = edge_writer(&[(E0, N0), (E0 + len * 0.6, N0 + len * 0.8)], 0);
        let f = *f;
        apply_dem(
            &mut w,
            &Surface(move |e: f64, n: f64| {
                let s = ((e - E0).powi(2) + (n - N0).powi(2)).sqrt();
                (1000.0 + f(s)) as f32
            }),
        );
        let sum: u32 = w.edge_attrs(0).grade.iter().map(|&v| v as u32).sum();
        assert_eq!(sum, 255, "profile {i}");
    }
    // A zero-length edge is flat.
    let mut w = edge_writer(&[(E0, N0), (E0, N0)], 0);
    apply_dem(&mut w, &Surface(|_: f64, _: f64| 5.0));
    assert_eq!(w.edge_attrs(0).grade, navilau::graph::FLAT_GRADE);
}

/// 10×10 float32 tile with value `f(col)`, tiepoint at its north-west corner.
fn write_tile(dir: &Path, year: u32, e_km: u32, n_km: u32, f: impl Fn(u32) -> f32) {
    use tiff::encoder::{colortype::Gray32Float, TiffEncoder};
    use tiff::tags::Tag;
    let name = format!("swissalti3d_{year}_{e_km}-{n_km}_2_2056_5728.tif");
    let file = std::fs::File::create(dir.join(name)).unwrap();
    let mut enc = TiffEncoder::new(file).unwrap();
    let mut img = enc.new_image::<Gray32Float>(10, 10).unwrap();
    let tie = [
        0.0,
        0.0,
        0.0,
        e_km as f64 * 1000.0,
        (n_km + 1) as f64 * 1000.0,
        0.0,
    ];
    img.encoder()
        .write_tag(Tag::ModelTiepointTag, &tie[..])
        .unwrap();
    let data: Vec<f32> = (0..100).map(|i| f(i % 10)).collect();
    img.write_data(&data).unwrap();
}

#[test]
fn alti3d_tile_samples_and_seams() {
    let dir = tempfile::tempdir().unwrap();
    write_tile(dir.path(), 2020, 2600, 1200, |c| 100.0 + c as f32);
    write_tile(dir.path(), 2020, 2601, 1200, |c| 110.0 + c as f32);
    write_tile(dir.path(), 2019, 2601, 1200, |_| 9999.0);
    let t = Alti3dTiles::open(dir.path()).unwrap();
    assert_eq!(t.tile_count(), 2);
    // Pixel centres at 2600050 + 100 k → value = 100 + (E - 2600050) / 100.
    let at = |e: f64| t.sample_lv95(e, 1_200_500.0).unwrap();
    assert!((at(2_600_050.0) - 100.0).abs() < 1e-3);
    assert!((at(2_600_500.0) - 104.5).abs() < 1e-3);
    assert!((at(2_601_000.0) - 109.5).abs() < 1e-3, "seam");
    assert!((at(2_601_250.0) - 112.0).abs() < 1e-3, "newest year wins");
    assert_eq!(t.sample_lv95(2_605_000.0, 1_200_500.0), None);
    assert_eq!(t.sample_lv95(2_600_500.0, 1_199_500.0), None);
}

#[test]
fn alti3d_tiepoint_mismatch_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    write_tile(dir.path(), 2020, 2600, 1200, |_| 1.0);
    // Same image under a name claiming a different tile.
    std::fs::rename(
        dir.path()
            .join("swissalti3d_2020_2600-1200_2_2056_5728.tif"),
        dir.path()
            .join("swissalti3d_2020_2602-1200_2_2056_5728.tif"),
    )
    .unwrap();
    let t = Alti3dTiles::open(dir.path()).unwrap();
    assert_eq!(t.sample_lv95(2_602_500.0, 1_200_500.0), None);
}

/// Real data smoke test: `cargo test --test import_dem -- --ignored`.
#[test]
#[ignore]
fn real_alti3d_wattwil_station() {
    let t = Alti3dTiles::open(Path::new("data/raw/alti3d")).unwrap();
    assert!(t.tile_count() > 500);
    let z = t.sample(47.299, 9.087).unwrap();
    eprintln!("Wattwil station: {z} m");
    assert!((z - 615.0).abs() < 15.0, "got {z}");
}

/// Hand-written little-endian tiled float32 LZW GeoTIFF (the `tiff` encoder
/// writes neither tiles nor custom LZW streams), optionally with each tile's
/// end code cut off. Pixel (x, y) from the north-west corner holds `f(x, y)`.
fn write_lzw_tiled(
    cut_end_code: bool,
    path: &Path,
    size: u32,
    tile: u32,
    e_km: u32,
    n_km: u32,
    f: impl Fn(u32, u32) -> f32,
) {
    let across = size.div_ceil(tile);
    let mut chunks = Vec::new();
    for ty in 0..across {
        for tx in 0..across {
            let mut raw = Vec::new();
            for r in 0..tile {
                for c in 0..tile {
                    let (x, y) = (tx * tile + c, ty * tile + r);
                    let v = if x < size && y < size { f(x, y) } else { 0.0 };
                    raw.extend(v.to_le_bytes());
                }
            }
            let mut lzw = weezl::encode::Encoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8)
                .encode(&raw)
                .unwrap();
            if cut_end_code {
                // The end code is at least 9 bits, so dropping the last byte
                // cuts only (part of) the end code, never data.
                lzw.pop();
                let strict = weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8)
                    .decode(&lzw);
                assert!(strict.is_err(), "end code still present");
            }
            chunks.push(lzw);
        }
    }
    let n = chunks.len() as u32;
    assert!(n > 1, "offset arrays must not be inline");
    const ENTRIES: u32 = 12;
    let ifd_end = 8 + 2 + ENTRIES * 12 + 4;
    let (offs_at, counts_at, tie_at) = (ifd_end, ifd_end + 4 * n, ifd_end + 8 * n);
    let mut data_at = tie_at + 48;
    let mut out: Vec<u8> = b"II*\0".to_vec();
    out.extend(8u32.to_le_bytes());
    out.extend((ENTRIES as u16).to_le_bytes());
    let mut entry = |tag: u16, typ: u16, count: u32, value: u32| {
        out.extend(tag.to_le_bytes());
        out.extend(typ.to_le_bytes());
        out.extend(count.to_le_bytes());
        out.extend(value.to_le_bytes());
    };
    let (short, long, double) = (3, 4, 12);
    entry(256, long, 1, size);
    entry(257, long, 1, size);
    entry(258, short, 1, 32);
    entry(259, short, 1, 5); // LZW
    entry(262, short, 1, 1);
    entry(277, short, 1, 1);
    entry(322, long, 1, tile);
    entry(323, long, 1, tile);
    entry(324, long, n, offs_at);
    entry(325, long, n, counts_at);
    entry(339, short, 1, 3); // IEEE float
    entry(33922, double, 6, tie_at);
    out.extend(0u32.to_le_bytes());
    for c in &chunks {
        out.extend(data_at.to_le_bytes());
        data_at += c.len() as u32;
    }
    for c in &chunks {
        out.extend((c.len() as u32).to_le_bytes());
    }
    for v in [
        0.0,
        0.0,
        0.0,
        e_km as f64 * 1000.0,
        (n_km + 1) as f64 * 1000.0,
        0.0,
    ] {
        out.extend(f64::to_le_bytes(v));
    }
    for c in &chunks {
        out.extend(c);
    }
    std::fs::write(path, out).unwrap();
}

#[test]
fn alti3d_decodes_lzw_tiles_without_end_code() {
    // 24 px in 16 px tiles: the right and bottom tiles are padded, like the
    // real 500 px tiles in 128 px blocks. With this noise, tiff 0.11.3's own
    // reader fails on the padded tile ("no lzw end code found") with and
    // without the end code, as it did on 82 of the 595 real tiles.
    let f = |x: u32, y: u32| {
        let mut h = 14 ^ ((x as u64) << 20) ^ ((y as u64) << 40);
        h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        h ^= h >> 29;
        600.0 + x as f32 * 0.37 + y as f32 * 0.21 + (h >> 50) as f32 * 0.001
    };
    for cut in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("swissalti3d_2020_2600-1200_2_2056_5728.tif");
        write_lzw_tiled(cut, &path, 24, 16, 2600, 1200, f);
        let t = Alti3dTiles::open(dir.path()).unwrap();
        let cell = 1000.0 / 24.0;
        for (x, y) in [(0, 0), (5, 9), (20, 3), (3, 20), (23, 23)] {
            let e = 2_600_000.0 + (x as f64 + 0.5) * cell;
            let n = 1_201_000.0 - (y as f64 + 0.5) * cell;
            let z = t.sample_lv95(e, n).expect("tile decodes");
            assert!((z - f(x, y)).abs() < 0.01, "cut {cut} ({x}, {y}): {z}");
        }
    }
}

/// Every real tile decodes: `cargo test --release --test import_dem -- --ignored real_alti3d_all`.
#[test]
#[ignore]
fn real_alti3d_all_tiles_decode() {
    let dir = Path::new("data/raw/alti3d");
    let t = Alti3dTiles::open(dir).unwrap();
    let mut failed = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        let Some(en) = name
            .strip_suffix("_2_2056_5728.tif")
            .and_then(|s| s.rsplit('_').next())
        else {
            continue;
        };
        let (e, n) = en.split_once('-').unwrap();
        let (e, n): (f64, f64) = (e.parse().unwrap(), n.parse().unwrap());
        if t.sample_lv95(e * 1000.0 + 500.0, n * 1000.0 + 500.0)
            .is_none()
        {
            failed.push(name);
        }
    }
    eprintln!(
        "{} tiles, {} failed: {failed:?}",
        t.tile_count(),
        failed.len()
    );
    assert!(failed.is_empty());
}

#[test]
fn alti3d_huge_tile_header_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir
        .path()
        .join("swissalti3d_2020_2600-1200_2_2056_5728.tif");
    write_lzw_tiled(false, &path, 24, 16, 2600, 1200, |_, _| 1.0);
    let mut bytes = std::fs::read(&path).unwrap();
    // IFD entry i starts at 8 + 2 + 12 i: tag, type, count (+4), value (+8).
    let mut patch = |i: usize, tag: u16, at: usize, v: u32| {
        let e = 10 + 12 * i;
        assert_eq!(u16::from_le_bytes([bytes[e], bytes[e + 1]]), tag);
        bytes[e + at..e + at + 4].copy_from_slice(&v.to_le_bytes());
    };
    // One 4e9 × 4e9 tile, so the tile count stays consistent for `tiff`.
    patch(6, 322, 8, 4_000_000_000);
    patch(7, 323, 8, 4_000_000_000);
    patch(8, 324, 4, 1);
    patch(9, 325, 4, 1);
    std::fs::write(&path, bytes).unwrap();
    let t = Alti3dTiles::open(dir.path()).unwrap();
    // A load error samples as None; the process must not abort on allocation.
    assert_eq!(t.sample_lv95(2_600_500.0, 1_200_500.0), None);
}
