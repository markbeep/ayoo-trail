use std::path::Path;

use trail_calc::graph::{flags, Bbox, Graph, GraphWriter, Highway, Surface};
use trail_calc::import::osm::import_osm;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new("tests/fixtures").join(name)
}

/// The single edge with highway class `hw`.
fn edge_of(w: &GraphWriter, hw: Highway) -> Option<u32> {
    (0..w.edge_count() as u32).find(|&e| w.edge_attrs(e).highway == hw)
}

#[test]
fn splits_at_junctions_and_skips_non_walkable() {
    let (w, stats, _extras) = import_osm(&fixture("tiny.osm"), None, 0).unwrap();
    assert_eq!(stats.edges, 2);
    let g = Graph::from_bytes(w.finish()).unwrap();
    assert_eq!(g.edge_count(), 2);
    assert_eq!(g.edge_geometry(0).count(), 3);
}

#[test]
fn closed_way_does_not_create_a_self_loop() {
    let (w, _stats, _extras) = import_osm(&fixture("loop.osm"), None, 0).unwrap();
    let g = Graph::from_bytes(w.finish()).unwrap();
    assert!(g.edge_count() >= 2);
    for e in 0..g.edge_count() as u32 {
        assert_ne!(g.edge_from(e), g.edge_to(e));
    }
    // The ring's closing segment 4 -> 1 is kept: total length is the full perimeter.
    let total: f64 = (0..g.edge_count() as u32).map(|e| g.edge_len_m(e)).sum();
    assert!(total > 370.0, "perimeter {total}"); // 2 x 76 m + 2 x 111 m
}

#[test]
fn maps_osm_tags_and_penalizes_unknown() {
    let (w, _stats, _extras) = import_osm(&fixture("tags.osm"), None, 0).unwrap();
    let g = Graph::from_bytes(w.finish()).unwrap();
    assert_eq!(g.edge_attrs(0).surface, Surface::Unknown);
    assert_eq!(g.edge_attrs(0).sac, 0);
    assert_ne!(g.edge_attrs(0).flags & flags::MISSING_TAGS, 0);
}

#[test]
fn infers_surface_from_highway() {
    let (w, _stats, _extras) = import_osm(&fixture("infer.osm"), None, 0).unwrap();
    let a = w.edge_attrs(edge_of(&w, Highway::Residential).unwrap());
    assert_eq!(a.surface, Surface::Asphalt);
    assert_ne!(a.flags & flags::SURFACE_INFERRED, 0);
    assert_eq!(a.flags & flags::MISSING_TAGS, 0);

    let a = w.edge_attrs(edge_of(&w, Highway::Track).unwrap());
    assert_eq!(a.surface, Surface::Dirt);
    assert_ne!(a.flags & flags::SURFACE_INFERRED, 0);

    let a = w.edge_attrs(edge_of(&w, Highway::Path).unwrap());
    assert_eq!(a.surface, Surface::Unknown);
    assert_ne!(a.flags & flags::MISSING_TAGS, 0);
    assert_eq!(a.flags & flags::SURFACE_INFERRED, 0);

    let a = w.edge_attrs(edge_of(&w, Highway::Footway).unwrap());
    assert_eq!(a.flags & flags::MISSING_TAGS, 0);
}

#[test]
fn access_and_area_rules() {
    let (w, stats, _extras) = import_osm(&fixture("infer.osm"), None, 0).unwrap();
    assert!(edge_of(&w, Highway::Service).is_none(), "access=private");
    assert!(
        edge_of(&w, Highway::Bridleway).is_some(),
        "access=private foot=yes"
    );
    assert!(edge_of(&w, Highway::Cycleway).is_none(), "foot=no");
    assert!(edge_of(&w, Highway::Pedestrian).is_none(), "area=yes");
    assert_eq!(stats.ways_skipped, 3);
    assert_eq!(stats.ways_used, 5);
}

#[test]
fn forest_multipolygon_with_hole() {
    let (_w, stats, extras) = import_osm(&fixture("forest_mp.osm"), None, 0).unwrap();
    assert_eq!(stats.rings_failed, 0);
    assert_eq!(extras.forest.len(), 1, "meadow is not forest");
    let p = &extras.forest[0];
    assert_eq!(p.exterior().0.len(), 5);
    assert_eq!(p.interiors().len(), 1);
    // x = lon, y = lat
    assert!(p
        .exterior()
        .0
        .iter()
        .all(|c| (7.9999..=8.0101).contains(&c.x)));
    assert!(p
        .exterior()
        .0
        .iter()
        .all(|c| (46.9999..=47.0101).contains(&c.y)));
}

#[test]
fn place_node_is_not_a_settlement() {
    // D77: a hamlet/village/town node used to become a 300 m–2 km circle; only mapped areas count now.
    let (_w, _stats, extras) = import_osm(&fixture("forest_mp.osm"), None, 0).unwrap();
    assert!(extras.residential.is_empty());
}

#[test]
fn bbox_keeps_ways_touching_the_region() {
    // Only node 2 (lon 7.401) of way 10 is inside; way 11 (lon 7.402..7.403) is outside.
    let bbox = Bbox {
        min_lat: 46.89,
        min_lon: 7.4005,
        max_lat: 46.91,
        max_lon: 7.4015,
    };
    let (w, stats, _extras) = import_osm(&fixture("tiny.osm"), Some(bbox), 0).unwrap();
    assert_eq!(stats.ways_used, 1);
    assert_eq!(stats.ways_skipped, 1);
    assert_eq!(w.edge_count(), 1);
    assert_eq!(w.edge_attrs(0).highway, Highway::Path);
    assert_eq!(w.edge_geometry(0).len(), 3, "whole way kept");
}
