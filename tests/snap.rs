use navilau::geodesy::haversine_m;
use navilau::graph::{EdgeAttrs, Graph, GraphWriter, LatLon};

fn ll(lat: f64, lon: f64) -> LatLon {
    LatLon { lat, lon }
}

/// Metres per degree of latitude.
const M_LAT: f64 = 111_195.0;

fn graph(edges: &[&[(f64, f64)]]) -> Graph {
    let mut w = GraphWriter::new(0);
    for g in edges {
        let first = g[0];
        let last = g[g.len() - 1];
        let a = w.add_node(ll(first.0, first.1), 0.0);
        let b = w.add_node(ll(last.0, last.1), 0.0);
        let geom: Vec<_> = g.iter().map(|&(la, lo)| (la, lo, 0.0f32)).collect();
        w.add_edge(a, b, EdgeAttrs::default(), &geom);
    }
    Graph::from_bytes(w.finish()).unwrap()
}

#[test]
fn snaps_to_midpoint_of_straight_edge() {
    let g = graph(&[&[(46.0, 7.0), (46.0, 7.01)]]);
    // ~11 m north of the midpoint.
    let s = g.snap(ll(46.0001, 7.005)).unwrap();
    assert_eq!(s.edge, 0);
    assert!((s.t - 0.5).abs() < 0.01, "t = {}", s.t);
    assert!((s.dist_m - 11.1).abs() < 1.0, "dist = {}", s.dist_m);
    assert!((s.pos.lon - 7.005).abs() < 1e-4);
}

#[test]
fn returns_none_far_from_any_edge() {
    let g = graph(&[&[(46.0, 7.0), (46.0, 7.01)]]);
    assert!(g.snap(ll(46.01, 7.005)).is_none());
}

#[test]
fn snap_within_accepts_a_wider_bound() {
    let g = graph(&[&[(46.0, 7.0), (46.0, 7.01)]]);
    let p = ll(46.002, 7.005); // ~222 m away
    assert!(g.snap(p).is_none());
    let s = g.snap_within(p, 300.0).unwrap();
    assert!((s.dist_m - 222.0).abs() < 3.0, "dist = {}", s.dist_m);
}

#[test]
fn t_follows_geometry_not_straight_line() {
    // Long first leg east (~772 m), short second leg north (~111 m).
    let corner = (46.0, 7.01);
    let g = graph(&[&[(46.0, 7.0), corner, (46.001, 7.01)]]);
    let first = haversine_m(ll(46.0, 7.0), ll(corner.0, corner.1));
    let second = haversine_m(ll(corner.0, corner.1), ll(46.001, 7.01));
    let expect = first / (first + second);
    // Beside the corner, on the outside of the L.
    let s = g.snap(ll(45.9999, 7.0101)).unwrap();
    assert!(
        (s.t as f64 - expect).abs() < 0.01,
        "t = {} want {}",
        s.t,
        expect
    );
}

#[test]
fn filtered_snap_skips_rejected_edges() {
    // Parallel edges 20 m and 60 m north of the query point.
    let dlat = |m: f64| m / M_LAT;
    let g = graph(&[
        &[(46.0 + dlat(60.0), 7.0), (46.0 + dlat(60.0), 7.01)],
        &[(46.0 + dlat(20.0), 7.0), (46.0 + dlat(20.0), 7.01)],
    ]);
    let p = ll(46.0, 7.005);
    let nearest = g.snap(p).unwrap();
    assert_eq!(nearest.edge, 1);
    let far = g.snap_filtered(p, 150.0, |e| e != nearest.edge).unwrap();
    assert_eq!(far.edge, 0);
    assert!((far.dist_m - 60.0).abs() < 1.0);
    assert!(g.snap_filtered(p, 150.0, |_| false).is_none());
}

#[test]
fn haversine_known_distance() {
    let d = haversine_m(ll(46.0, 7.0), ll(47.0, 7.0));
    assert!((d - 111_195.0).abs() < 10.0, "d = {d}");
}

#[test]
fn empty_graph_and_degenerate_segments_do_not_panic() {
    let empty = Graph::from_bytes(GraphWriter::new(0).finish()).unwrap();
    assert!(empty.snap(ll(46.0, 7.0)).is_none());
    let g = graph(&[&[(46.0, 7.0), (46.0, 7.0)]]);
    let s = g.snap(ll(46.0, 7.0001)).unwrap();
    assert_eq!(s.t, 0.0);
}
