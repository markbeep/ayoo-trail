use navilau::graph::{
    flags, AnchorKind, Direction, EdgeAttrs, Graph, GraphError, GraphWriter, Highway, LatLon,
    Surface,
};

fn ll(lat: f64, lon: f64) -> LatLon {
    LatLon { lat, lon }
}

#[test]
fn writer_open_round_trip() {
    let mut w = GraphWriter::new(1_700_000_000);
    let a = w.add_node(ll(46.0, 7.0), 100.0);
    let b = w.add_node(ll(46.001, 7.001), 120.0);
    let attrs = EdgeAttrs {
        length_m: 140.5,
        gain: 20,
        loss: 3,
        grade: [0, 0, 0, 10, 200, 30, 15, 0, 0],
        surface: Surface::Gravel,
        sac: 2,
        highway: Highway::Track,
        forest_pct: 80,
        urban: 1,
        official_category: 3,
        flags: flags::BRIDGE | flags::SURFACE_INFERRED,
    };
    let e = w.add_edge(a, b, attrs, &[(46.0, 7.0, 100.0), (46.001, 7.001, 120.0)]);
    assert_eq!(e, 0);

    let g = Graph::from_bytes(w.finish()).unwrap();
    assert_eq!(g.node_count(), 2);
    assert_eq!(g.edge_count(), 1);
    assert_eq!(g.osm_date(), 1_700_000_000);
    assert!((g.node(b).lat - 46.001).abs() < 1e-5);
    assert!((g.node(b).lon - 7.001).abs() < 1e-5);
    assert_eq!(g.edge_from(0), a);
    assert_eq!(g.edge_to(0), b);
    assert!((g.edge_len_m(0) - 140.5).abs() < 0.01);
    let got = g.edge_attrs(0);
    assert!((got.length_m - 140.5).abs() < 0.01);
    assert_eq!(got.gain, 20);
    assert_eq!(got.loss, 3);
    assert_eq!(got.grade, attrs.grade);
    assert_eq!(got.surface, Surface::Gravel);
    assert_eq!(got.highway, Highway::Track);
    assert_eq!(got.sac, 2);
    assert_eq!(got.flags, attrs.flags);
    assert_eq!(got.forest_pct, 80);
    assert_eq!(got.urban, 1);
    assert_eq!(got.official_category, 3);
}

#[test]
fn open_from_disk_reads_every_section() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("g.bin");
    let mut w = GraphWriter::new(42);
    w.set_dem_source(1);
    let a = w.add_node(ll(46.0, 7.0), 100.0);
    let b = w.add_node(ll(46.001, 7.0), 110.0);
    w.add_anchor(ll(46.0, 7.0), AnchorKind::Peak, 1.0);
    w.add_edge(
        a,
        b,
        EdgeAttrs::default(),
        &[(46.0, 7.0, 100.0), (46.001, 7.0, 110.0)],
    );
    w.write_to(&path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), w.finish());

    let g = Graph::open(&path).unwrap();
    assert_eq!(g.anchors().len(), 1);
    assert_eq!(g.anchors()[0].kind, AnchorKind::Peak);
    assert_eq!(g.dem_source(), 1);
    assert_eq!(g.osm_date(), 42);
    assert!((g.node_elev(b) - 110.0).abs() < 0.5);
    assert_eq!(g.edge_from(0), a);
    assert_eq!(g.edge_to(0), b);
    let geom: Vec<_> = g.edge_geometry(0).collect();
    assert_eq!(geom.len(), 2);
    assert!((geom[1].2 - 110.0).abs() < 0.5);
    assert!((geom[1].0 - 46.001).abs() < 1e-5);
    let rev: Vec<_> = g.edge_geometry(0).rev().collect();
    assert_eq!(rev[0], geom[1]);
    let bb = g.bbox();
    assert!((bb.min_lat - 46.0).abs() < 1e-5 && (bb.max_lat - 46.001).abs() < 1e-5);
    assert!((bb.min_lon - 7.0).abs() < 1e-5 && (bb.max_lon - 7.0).abs() < 1e-5);
}

#[test]
fn csr_lists_each_edge_at_both_endpoints() {
    let mut w = GraphWriter::new(0);
    let a = w.add_node(ll(46.0, 7.0), 0.0);
    let b = w.add_node(ll(46.0, 7.001), 0.0);
    let c = w.add_node(ll(46.0, 7.002), 0.0);
    w.add_edge(
        a,
        b,
        EdgeAttrs::default(),
        &[(46.0, 7.0, 0.0), (46.0, 7.001, 0.0)],
    );
    w.add_edge(
        c,
        b,
        EdgeAttrs::default(),
        &[(46.0, 7.002, 0.0), (46.0, 7.001, 0.0)],
    );
    let g = Graph::from_bytes(w.finish()).unwrap();

    let mut nb: Vec<_> = g.neighbors(b).collect();
    nb.sort_by_key(|x| x.0);
    assert_eq!(nb, vec![(0, Direction::Backward), (1, Direction::Backward)]);
    assert_eq!(
        g.neighbors(a).collect::<Vec<_>>(),
        vec![(0, Direction::Forward)]
    );
    assert_eq!(
        g.neighbors(c).collect::<Vec<_>>(),
        vec![(1, Direction::Forward)]
    );
}

#[test]
fn set_edge_geometry_replaces_geometry() {
    let mut w = GraphWriter::new(0);
    let a = w.add_node(ll(46.0, 7.0), 0.0);
    let b = w.add_node(ll(46.0, 7.002), 0.0);
    let e = w.add_edge(
        a,
        b,
        EdgeAttrs::default(),
        &[(46.0, 7.0, 0.0), (46.0, 7.002, 0.0)],
    );
    let e2 = w.add_edge(
        b,
        a,
        EdgeAttrs::default(),
        &[(46.0, 7.002, 5.0), (46.0, 7.0, 6.0)],
    );
    w.set_edge_geometry(
        e,
        vec![(46.0, 7.0, 0.0), (46.0005, 7.001, 4.0), (46.0, 7.002, 0.0)],
    );
    assert_eq!(w.edge_geometry(e).len(), 3);
    assert_eq!(w.edge_geometry(e2).len(), 2);
    w.edge_attrs_mut(e).sac = 4;
    assert_eq!(w.edge_attrs(e).sac, 4);
    assert_eq!(w.edge_endpoints(e), (a, b));

    let g = Graph::from_bytes(w.finish()).unwrap();
    let geom: Vec<_> = g.edge_geometry(e).collect();
    assert_eq!(geom.len(), 3);
    assert!((geom[1].0 - 46.0005).abs() < 1e-5);
    assert!((geom[1].2 - 4.0).abs() < 0.5);
    let geom2: Vec<_> = g.edge_geometry(e2).collect();
    assert_eq!(geom2.len(), 2);
    assert!((geom2[1].2 - 6.0).abs() < 0.5);
}

#[test]
fn bad_magic_and_truncation_are_errors() {
    let mut w = GraphWriter::new(0);
    let a = w.add_node(ll(46.0, 7.0), 0.0);
    let b = w.add_node(ll(46.0, 7.001), 0.0);
    w.add_edge(
        a,
        b,
        EdgeAttrs::default(),
        &[(46.0, 7.0, 0.0), (46.0, 7.001, 0.0)],
    );
    let bytes = w.finish();

    let mut bad = bytes.clone();
    bad[0] ^= 0xff;
    assert!(matches!(Graph::from_bytes(bad), Err(GraphError::Magic)));

    let mut ver = bytes.clone();
    ver[4] = 9;
    assert!(matches!(
        Graph::from_bytes(ver),
        Err(GraphError::Version(9))
    ));

    // Snap index entry count (header bytes 52..56) larger than the file holds.
    let mut snap = bytes.clone();
    snap[52..56].copy_from_slice(&1_000_000u32.to_le_bytes());
    assert!(matches!(
        Graph::from_bytes(snap),
        Err(GraphError::Truncated)
    ));

    let mut short = bytes.clone();
    short.pop();
    assert!(matches!(
        Graph::from_bytes(short),
        Err(GraphError::Truncated)
    ));
    assert!(matches!(
        Graph::from_bytes(vec![1, 2, 3]),
        Err(GraphError::Truncated)
    ));

    // edge_to[0] sits after node_lat, node_lon (8 B each, 2 nodes), node_elev (4 B ->
    // padded 8), node_first (12 B -> 16), adj (8 B), edge_from (4 B -> 8).
    let edge_to_off = 64 + 8 + 8 + 8 + 16 + 8 + 8;
    let mut idx = bytes.clone();
    idx[edge_to_off..edge_to_off + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(Graph::from_bytes(idx), Err(GraphError::Index)));
}

#[test]
fn empty_graph_round_trips() {
    let w = GraphWriter::new(7);
    let g = Graph::from_bytes(w.finish()).unwrap();
    assert_eq!(g.node_count(), 0);
    assert_eq!(g.edge_count(), 0);
    assert_eq!(g.osm_date(), 7);
    assert!(g.anchors().is_empty());
    let bb = g.bbox();
    assert_eq!(
        (bb.min_lat, bb.min_lon, bb.max_lat, bb.max_lon),
        (0.0, 0.0, 0.0, 0.0)
    );
}
