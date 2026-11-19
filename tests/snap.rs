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

#[test]
fn grid_snap_matches_brute_force() {
    // A ~3 km web of zig-zag edges spanning many 250 m cells, queried at a lattice of points.
    let mut seed = 7u64;
    let mut rnd = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as f64 / (1u64 << 31) as f64
    };
    let edges: Vec<Vec<(f64, f64)>> = (0..60)
        .map(|_| {
            let (mut la, mut lo) = (46.0 + rnd() * 0.03, 7.0 + rnd() * 0.04);
            (0..1 + (rnd() * 6.0) as usize + 1)
                .map(|_| {
                    let p = (la, lo);
                    la += (rnd() - 0.5) * 0.004;
                    lo += (rnd() - 0.5) * 0.006;
                    p
                })
                .collect()
        })
        .collect();
    let refs: Vec<&[(f64, f64)]> = edges.iter().map(|e| e.as_slice()).collect();
    let g = graph(&refs);
    let proj = navilau::geodesy::LocalProjection::new(46.015);
    for i in 0..40 {
        for j in 0..40 {
            let p = ll(45.995 + i as f64 * 0.001, 6.995 + j as f64 * 0.0013);
            let q = proj.project(p);
            // Nearest over every segment of every edge (f32-rounded like the file).
            let best = (0..g.edge_count() as u32)
                .flat_map(|e| {
                    let pts: Vec<[f64; 2]> = g
                        .edge_geometry(e)
                        .map(|(la, lo, _)| proj.project(ll(la, lo)))
                        .collect();
                    pts.windows(2)
                        .map(|w| {
                            let (c, _) = navilau::geodesy::closest_on_segment(q, w[0], w[1]);
                            haversine_m(p, proj.unproject(c))
                        })
                        .collect::<Vec<_>>()
                })
                .fold(f64::INFINITY, f64::min);
            match g.snap_within(p, 500.0) {
                Some(s) => assert!(
                    (s.dist_m as f64 - best).abs() < 0.5,
                    "{p:?}: snap {} brute {best}",
                    s.dist_m
                ),
                None => assert!(best > 499.0, "{p:?}: missed edge at {best} m"),
            }
        }
    }
}

#[test]
fn far_outside_the_grid_is_none() {
    let g = graph(&[&[(46.0, 7.0), (46.0, 7.01)]]);
    assert!(g.snap_within(ll(40.0, 1.0), 500.0).is_none());
    assert!(g.snap_within(ll(50.0, 12.0), 500.0).is_none());
}
