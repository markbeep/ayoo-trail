use std::path::Path;

use geo::{LineString, Polygon};
use trail_calc::graph::{flags, AnchorKind, EdgeAttrs, Graph, GraphWriter, LatLon};
use trail_calc::import::ch1903::{lv95_to_wgs84, wgs84_to_lv95};
use trail_calc::import::landcover::{apply_landcover, collect_anchors};
use trail_calc::import::official::{
    apply_official, apply_tlm, read_official_gpkg, read_tlm_shp, OfficialLine, OFFICIAL_TOLERANCE_M,
};
use trail_calc::import::osm::OsmExtras;
use trail_calc::import::region;

/// One edge through the given (lat, lon) vertices.
fn one_edge(pts: &[(f64, f64)]) -> GraphWriter {
    let mut w = GraphWriter::new(0);
    let (a, b) = (pts[0], pts[pts.len() - 1]);
    let from = w.add_node(LatLon { lat: a.0, lon: a.1 }, 0.0);
    let to = w.add_node(LatLon { lat: b.0, lon: b.1 }, 0.0);
    let geom: Vec<(f64, f64, f32)> = pts.iter().map(|&(la, lo)| (la, lo, 0.0)).collect();
    w.add_edge(from, to, EdgeAttrs::default(), &geom);
    w
}

fn lv95(lat: f64, lon: f64) -> [f64; 2] {
    let (e, n) = wgs84_to_lv95(lat, lon);
    [e, n]
}

#[test]
fn marks_edges_near_an_official_line_with_its_category() {
    let mut w = one_edge(&[(46.0, 7.0), (46.0, 7.01)]);
    let lines = vec![OfficialLine {
        lv95: vec![lv95(46.0, 7.0), lv95(46.0, 7.01)],
        category: 1,
    }];
    assert_eq!(apply_official(&mut w, &lines, OFFICIAL_TOLERANCE_M), 1);
    let g = Graph::from_bytes(w.finish()).unwrap();
    assert_eq!(g.edge_attrs(0).official_category, 1);
}

#[test]
fn edge_half_off_the_line_is_not_official() {
    // LV95 offsets from a base point: 400 m on the line, then 100 m away for 600 m.
    let (e0, n0) = (2_600_000.0, 1_200_000.0);
    let at = |de: f64, dn: f64| lv95_to_wgs84(e0 + de, n0 + dn);
    let mut w = one_edge(&[
        at(0.0, 0.0),
        at(400.0, 0.0),
        at(400.0, 100.0),
        at(1000.0, 100.0),
    ]);
    let lines = vec![OfficialLine {
        lv95: vec![[e0 - 100.0, n0], [e0 + 1100.0, n0]],
        category: 2,
    }];
    assert_eq!(apply_official(&mut w, &lines, OFFICIAL_TOLERANCE_M), 0);
    assert_eq!(w.edge_attrs(0).official_category, 0);
}

/// GPKG blob: header with an xy envelope (code 1), then ISO WKB LineString Z.
fn gpkg_linestring_z(pts: &[[f64; 3]]) -> Vec<u8> {
    let mut b = b"GP".to_vec();
    b.push(0); // version
    b.push(0b0000_0011); // little-endian, envelope code 1
    b.extend(2056i32.to_le_bytes());
    let (mut min, mut max) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in pts {
        for i in 0..2 {
            min[i] = min[i].min(p[i]);
            max[i] = max[i].max(p[i]);
        }
    }
    for v in [min[0], max[0], min[1], max[1]] {
        b.extend(v.to_le_bytes());
    }
    b.push(1);
    b.extend(1002u32.to_le_bytes());
    b.extend((pts.len() as u32).to_le_bytes());
    for p in pts {
        for v in p {
            b.extend(v.to_le_bytes());
        }
    }
    b
}

#[test]
fn reads_lines_from_a_gpkg() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("w.gpkg");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE tlm_strassen_strasse (id INTEGER PRIMARY KEY, geom BLOB, wanderwege TEXT);",
    )
    .unwrap();
    let near = gpkg_linestring_z(&[
        [2_600_000.0, 1_200_000.0, 500.0],
        [2_600_100.0, 1_200_050.0, 510.0],
    ]);
    let far = gpkg_linestring_z(&[
        [2_700_000.0, 1_250_000.0, 0.0],
        [2_700_100.0, 1_250_000.0, 0.0],
    ]);
    let mut ins = db
        .prepare("INSERT INTO tlm_strassen_strasse (geom, wanderwege) VALUES (?1, ?2)")
        .unwrap();
    ins.execute(rusqlite::params![near, "Bergwanderweg"])
        .unwrap();
    ins.execute(rusqlite::params![near, None::<String>])
        .unwrap();
    ins.execute(rusqlite::params![far, "Wanderweg"]).unwrap();
    drop(ins);
    drop(db);

    let clip = [2_599_000.0, 1_199_000.0, 2_601_000.0, 1_201_000.0];
    let lines = read_official_gpkg(&path, Some(clip)).unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].category, 2);
    assert_eq!(
        lines[0].lv95,
        vec![[2_600_000.0, 1_200_000.0], [2_600_100.0, 1_200_050.0]]
    );
    assert_eq!(read_official_gpkg(&path, None).unwrap().len(), 2);
}

#[test]
fn tlm_flag_marks_only_edges_on_a_tlm_line() {
    let mut w = one_edge(&[(46.0, 7.0), (46.0, 7.01)]);
    let off = lv95(46.0, 7.0);
    let from = w.add_node(
        LatLon {
            lat: 46.01,
            lon: 7.0,
        },
        0.0,
    );
    let to = w.add_node(
        LatLon {
            lat: 46.01,
            lon: 7.01,
        },
        0.0,
    );
    w.add_edge(
        from,
        to,
        EdgeAttrs::default(),
        &[(46.01, 7.0, 0.0), (46.01, 7.01, 0.0)],
    );
    let lines = vec![OfficialLine {
        lv95: vec![off, lv95(46.0, 7.01)],
        category: 1,
    }];
    assert_eq!(apply_tlm(&mut w, &lines, OFFICIAL_TOLERANCE_M), 1);
    assert_ne!(w.edge_attrs(0).flags & flags::IN_TLM, 0);
    assert_eq!(w.edge_attrs(1).flags & flags::IN_TLM, 0);
    assert_eq!(
        w.edge_attrs(0).official_category,
        0,
        "the TLM pass leaves categories alone"
    );
}

/// A PolyLineZ shapefile (type 13): one record per line, one part each.
fn shp_polyline_z(lines: &[&[[f64; 2]]]) -> Vec<u8> {
    let mut recs = Vec::new();
    for (i, pts) in lines.iter().enumerate() {
        let mut c = Vec::new();
        c.extend(13i32.to_le_bytes());
        let (xs, ys) = (pts.iter().map(|p| p[0]), pts.iter().map(|p| p[1]));
        for v in [
            xs.clone().fold(f64::MAX, f64::min),
            ys.clone().fold(f64::MAX, f64::min),
            xs.fold(f64::MIN, f64::max),
            ys.fold(f64::MIN, f64::max),
        ] {
            c.extend(v.to_le_bytes());
        }
        c.extend(1i32.to_le_bytes());
        c.extend((pts.len() as i32).to_le_bytes());
        c.extend(0i32.to_le_bytes());
        for p in *pts {
            c.extend(p[0].to_le_bytes());
            c.extend(p[1].to_le_bytes());
        }
        c.extend([0u8; 16]); // z range
        c.extend(vec![0u8; 8 * pts.len()]); // z values
        recs.extend((i as i32 + 1).to_be_bytes());
        recs.extend(((c.len() / 2) as i32).to_be_bytes());
        recs.extend(c);
    }
    let mut out = Vec::new();
    out.extend(9994i32.to_be_bytes());
    out.extend([0u8; 20]);
    out.extend((((100 + recs.len()) / 2) as i32).to_be_bytes());
    out.extend(1000i32.to_le_bytes());
    out.extend(13i32.to_le_bytes());
    out.extend([0u8; 64]); // bbox x/y/z/m
    out.extend(recs);
    out
}

#[test]
fn reads_lines_from_a_shapefile() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.shp");
    let near: &[[f64; 2]] = &[
        [2_600_000.0, 1_200_000.0],
        [2_600_100.0, 1_200_050.0],
        [2_600_200.0, 1_200_050.0],
    ];
    let far: &[[f64; 2]] = &[[2_700_000.0, 1_250_000.0], [2_700_100.0, 1_250_000.0]];
    std::fs::write(&path, shp_polyline_z(&[near, far])).unwrap();
    let clip = [2_599_000.0, 1_199_000.0, 2_601_000.0, 1_201_000.0];
    let lines = read_tlm_shp(&path, Some(clip)).unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].lv95, near);
    assert_eq!(read_tlm_shp(&path, None).unwrap().len(), 2);
}

/// Axis-aligned polygon in lon/lat (x = lon).
fn rect(min_lat: f64, min_lon: f64, max_lat: f64, max_lon: f64) -> Polygon<f64> {
    Polygon::new(
        LineString::from(vec![
            (min_lon, min_lat),
            (max_lon, min_lat),
            (max_lon, max_lat),
            (min_lon, max_lat),
            (min_lon, min_lat),
        ]),
        vec![],
    )
}

#[test]
fn forest_urban_and_anchors_from_polygons() {
    let mut w = one_edge(&[(46.9, 7.40), (46.9, 7.41)]);
    let extras = OsmExtras {
        forest: vec![rect(46.89, 7.39, 46.91, 7.42)],
        // Covers the east end of the edge.
        residential: vec![rect(46.895, 7.408, 46.905, 7.42)],
        peaks: vec![
            (
                LatLon {
                    lat: 46.9,
                    lon: 7.405,
                },
                1200.0,
            ),
            (
                LatLon {
                    lat: 45.0,
                    lon: 7.4,
                },
                900.0,
            ),
        ],
        viewpoints: vec![LatLon {
            lat: 46.9,
            lon: 7.402,
        }],
    };
    apply_landcover(&mut w, &extras);
    collect_anchors(&mut w, &extras);
    let g = Graph::from_bytes(w.finish()).unwrap();
    assert!(g.edge_attrs(0).forest_pct > 50);
    assert!(g.edge_attrs(0).urban > 0);
    let anchors = g.anchors();
    assert_eq!(anchors.len(), 2, "the peak outside the bbox is dropped");
    assert_eq!(anchors[0].kind, AnchorKind::Peak);
    assert_eq!(anchors[0].weight, 1200.0);
    assert_eq!(anchors[1].kind, AnchorKind::Viewpoint);
    assert_eq!(anchors[1].weight, 1.0);
}

#[test]
fn urban_within_50m_is_class_1() {
    let (e0, n0) = (2_600_000.0, 1_200_000.0);
    let at = |de: f64, dn: f64| lv95_to_wgs84(e0 + de, n0 + dn);
    // A 500 × 200 m residential block (10 ha) starting `dn` meters north of the edge.
    let block = |dn: f64| {
        let (s, wst) = at(0.0, dn);
        let (n, est) = at(500.0, dn + 200.0);
        rect(s, wst, n, est)
    };
    for (dn, want) in [(30.0, 1), (100.0, 0)] {
        let mut w = one_edge(&[at(0.0, 0.0), at(500.0, 0.0)]);
        let extras = OsmExtras {
            residential: vec![block(dn)],
            ..Default::default()
        };
        apply_landcover(&mut w, &extras);
        assert_eq!(w.edge_attrs(0).urban, want, "block {dn} m away");
        assert_eq!(w.edge_attrs(0).forest_pct, 0);
    }
}

#[test]
fn settlement_under_one_hectare_is_ignored() {
    let (e0, n0) = (2_600_000.0, 1_200_000.0);
    let at = |de: f64, dn: f64| lv95_to_wgs84(e0 + de, n0 + dn);
    // A 60 × 60 m farmstead (0.36 ha) the edge runs through, and a 120 × 120 m (1.44 ha) one.
    for (side, want) in [(60.0, 0), (120.0, 2)] {
        let mut w = one_edge(&[at(0.0, 0.0), at(500.0, 0.0)]);
        let (s, wst) = at(200.0, -side / 2.0);
        let (n, est) = at(200.0 + side, side / 2.0);
        let extras = OsmExtras {
            residential: vec![rect(s, wst, n, est)],
            ..Default::default()
        };
        apply_landcover(&mut w, &extras);
        assert_eq!(w.edge_attrs(0).urban, want, "{side} m square");
    }
}

#[test]
#[ignore = "reads the real swissTLM3D GeoPackage from data/raw"]
fn real_gpkg_smoke() {
    let b = region("demo").unwrap().bbox;
    let (min_e, min_n) = wgs84_to_lv95(b.min_lat, b.min_lon);
    let (max_e, max_n) = wgs84_to_lv95(b.max_lat, b.max_lon);
    let lines = read_official_gpkg(
        Path::new("data/raw/wanderwege/SWISSTLM3D_WANDERWEGE.gpkg"),
        Some([min_e, min_n, max_e, max_n]),
    )
    .unwrap();
    assert!(lines.len() > 1000, "{} lines", lines.len());
    assert!(lines.iter().all(|l| (1..=3).contains(&l.category)));
}

#[test]
fn import_binary_skips_existing_output() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("graph.bin");
    std::fs::write(&out, b"keep me").unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_trail-calc-import"))
        .args([
            "--region",
            "demo",
            "--osm",
            "/nonexistent.pbf",
            "--dem",
            "/nonexistent",
        ])
        .args(["--official", "/nonexistent.gpkg", "--out"])
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(std::fs::read(&out).unwrap(), b"keep me");
}

#[test]
#[ignore = "reads the real swissTLM3D TLM_STRASSE.shp from data/raw"]
fn real_tlm_shp_smoke() {
    let b = region("demo").unwrap().bbox;
    let (min_e, min_n) = wgs84_to_lv95(b.min_lat, b.min_lon);
    let (max_e, max_n) = wgs84_to_lv95(b.max_lat, b.max_lon);
    let lines = read_tlm_shp(
        Path::new("data/raw/tlm/TLM_STRASSE.shp"),
        Some([min_e, min_n, max_e, max_n]),
    )
    .unwrap();
    assert!(lines.len() > 10_000, "{} lines", lines.len());
    assert!(lines.iter().all(|l| l
        .lv95
        .iter()
        .all(|p| p[0] > 2_480_000.0 && p[1] > 1_070_000.0)));
}
