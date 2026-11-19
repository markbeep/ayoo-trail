use trail_calc::cost::{score, Profile};
use trail_calc::geodesy::haversine_m;
use trail_calc::graph::{flags, Direction, EdgeAttrs, Graph, GraphWriter, LatLon, Surface};
use trail_calc::route::*;
use trail_calc::snap::Snap;

const LAT: f64 = 46.0;
const LON: f64 = 7.0;
const M_LAT: f64 = 111_195.0;

fn ll(lat: f64, lon: f64) -> LatLon {
    LatLon { lat, lon }
}

/// Point `x` meters east and `y` meters north of (LAT, LON).
fn at(x: f64, y: f64) -> LatLon {
    ll(LAT + y / M_LAT, LON + x / (M_LAT * LAT.to_radians().cos()))
}

fn attrs(surface: Surface, len: f64) -> EdgeAttrs {
    EdgeAttrs {
        length_m: len as f32,
        surface,
        sac: 1,
        ..Default::default()
    }
}

/// Adds an edge through the given (x, y, z) points; nodes are created per call
/// unless `from`/`to` are given.
fn edge(
    w: &mut GraphWriter,
    from: Option<u32>,
    to: Option<u32>,
    pts: &[(f64, f64, f32)],
    a: EdgeAttrs,
) -> u32 {
    let geom: Vec<_> = pts
        .iter()
        .map(|&(x, y, z)| {
            let p = at(x, y);
            (p.lat, p.lon, z)
        })
        .collect();
    let first = geom[0];
    let last = geom[geom.len() - 1];
    let a_n = from.unwrap_or_else(|| w.add_node(ll(first.0, first.1), first.2));
    let b_n = to.unwrap_or_else(|| w.add_node(ll(last.0, last.1), last.2));
    w.add_edge(a_n, b_n, a, &geom)
}

fn snap(g: &Graph, p: LatLon) -> Snap {
    g.snap(p).unwrap()
}

const DIRECT_EDGE: u32 = 0;
const DETOUR_EDGE: u32 = 1;

/// Direct asphalt edge (1000 m, 2.0/m) and a gravel detour (1150 m, 1.0/m) between the same nodes.
fn two_path_graph() -> Graph {
    let mut w = GraphWriter::new(0);
    let a = w.add_node(at(0.0, 0.0), 0.0);
    let b = w.add_node(at(1000.0, 0.0), 0.0);
    let h = (575.0f64.powi(2) - 500.0f64.powi(2)).sqrt();
    edge(
        &mut w,
        Some(a),
        Some(b),
        &[(0.0, 0.0, 0.0), (1000.0, 0.0, 0.0)],
        attrs(Surface::Asphalt, 1000.0),
    );
    edge(
        &mut w,
        Some(a),
        Some(b),
        &[(0.0, 0.0, 0.0), (500.0, h, 0.0), (1000.0, 0.0, 0.0)],
        attrs(Surface::Gravel, 1150.0),
    );
    Graph::from_bytes(w.finish()).unwrap()
}

/// One straight 1000 m gravel edge east from the origin with z 100 -> 200.
fn straight() -> Graph {
    let mut w = GraphWriter::new(0);
    let mut a = attrs(Surface::Gravel, 1000.0);
    a.gain = 100;
    edge(
        &mut w,
        None,
        None,
        &[(0.0, 0.0, 100.0), (500.0, 0.0, 150.0), (1000.0, 0.0, 200.0)],
        a,
    );
    Graph::from_bytes(w.finish()).unwrap()
}

fn empty(g: &Graph) -> Visited {
    Visited::new(g.edge_count())
}

#[test]
fn picks_the_cheaper_of_two_paths() {
    let g = two_path_graph();
    let start = snap(&g, at(100.0, 0.0));
    let end = snap(&g, at(900.0, 0.0));
    assert_eq!(start.edge, DIRECT_EDGE);
    let r = route(&g, &Profile::default(), start, end, &empty(&g)).unwrap();
    assert!(r.steps.iter().any(|s| s.edge == DETOUR_EDGE), "{r:?}");
    assert!((r.cost - (200.0 * 2.0 + 1150.0)).abs() < 1.0, "{}", r.cost);
}

#[test]
fn visited_penalty_flips_the_choice() {
    let g = two_path_graph();
    let start = snap(&g, at(100.0, 0.0));
    let end = snap(&g, at(900.0, 0.0));
    let mut visited = empty(&g);
    visited.insert(DETOUR_EDGE);
    let r = route(&g, &Profile::default(), start, end, &visited).unwrap();
    assert_eq!(r.steps.len(), 1);
    assert_eq!(r.steps[0].edge, DIRECT_EDGE);
}

#[test]
fn disconnected_returns_none() {
    let mut w = GraphWriter::new(0);
    edge(
        &mut w,
        None,
        None,
        &[(0.0, 0.0, 0.0), (500.0, 0.0, 0.0)],
        attrs(Surface::Gravel, 500.0),
    );
    edge(
        &mut w,
        None,
        None,
        &[(0.0, 2000.0, 0.0), (500.0, 2000.0, 0.0)],
        attrs(Surface::Gravel, 500.0),
    );
    let g = Graph::from_bytes(w.finish()).unwrap();
    let a = snap(&g, at(250.0, 0.0));
    let b = snap(&g, at(250.0, 2000.0));
    assert!(route(&g, &Profile::default(), a, b, &empty(&g)).is_none());
}

#[test]
fn same_edge_waypoints_connect_directly() {
    let g = straight();
    let start = snap(&g, at(200.0, 0.0));
    let end = snap(&g, at(800.0, 0.0));
    let r = route(&g, &Profile::default(), start, end, &empty(&g)).unwrap();
    assert!((r.distance_m - 600.0).abs() < 1.0, "{}", r.distance_m);
    assert_eq!(r.steps.len(), 1);
}

#[test]
fn same_edge_backward_is_direct() {
    let g = straight();
    let start = snap(&g, at(800.0, 0.0));
    let end = snap(&g, at(200.0, 0.0));
    let r = route(&g, &Profile::default(), start, end, &empty(&g)).unwrap();
    assert_eq!(r.steps.len(), 1);
    assert!(r.steps[0].from_t > r.steps[0].to_t);
    assert_eq!(r.steps[0].direction(), Direction::Backward);
    assert!((r.distance_m - 600.0).abs() < 1.0, "{}", r.distance_m);
}

#[test]
fn reversed_step_geometry_runs_in_travel_order() {
    let g = straight();
    let start = snap(&g, at(800.0, 0.0));
    let end = snap(&g, at(200.0, 0.0));
    let r = route(&g, &Profile::default(), start, end, &empty(&g)).unwrap();
    let geom = route_geometry(&g, &r);
    let (f, l) = (geom[0], geom[geom.len() - 1]);
    assert!(haversine_m(ll(f.0, f.1), start.pos) < 1.0);
    assert!(haversine_m(ll(l.0, l.1), end.pos) < 1.0);
    assert!(
        (f.2 - 180.0).abs() < 1.0 && (l.2 - 120.0).abs() < 1.0,
        "{geom:?}"
    );
    assert!(geom.windows(2).all(|w| w[1].2 <= w[0].2), "{geom:?}");
    // 180 -> 150 (interior vertex) -> 120.
    assert_eq!(geom.len(), 3);
    assert!((r.descent_m - 60.0).abs() < 1.0 && r.climb_m == 0.0);
}

#[test]
fn route_via_merges_contiguous_steps() {
    let g = straight();
    let snaps: Vec<_> = [100.0, 500.0, 900.0]
        .iter()
        .map(|&x| snap(&g, at(x, 0.0)))
        .collect();
    let r = route_via(&g, &Profile::default(), &snaps).unwrap();
    assert_eq!(r.steps.len(), 1, "{r:?}");
    assert!((r.steps[0].from_t - 0.1).abs() < 1e-3);
    assert!((r.steps[0].to_t - 0.9).abs() < 1e-3);
    assert!(route_via(&g, &Profile::default(), &snaps[..1]).is_none());
}

#[test]
fn route_via_allows_out_and_back() {
    let g = straight();
    let snaps: Vec<_> = [200.0, 800.0, 200.0]
        .iter()
        .map(|&x| snap(&g, at(x, 0.0)))
        .collect();
    let r = route_via(&g, &Profile::default(), &snaps).unwrap();
    assert!((r.distance_m - 1200.0).abs() < 1.0, "{}", r.distance_m);
}

#[test]
fn stats_sum_to_distance() {
    let mut w = GraphWriter::new(0);
    let b = w.add_node(at(500.0, 0.0), 0.0);
    let mut a0 = attrs(Surface::Asphalt, 500.0);
    a0.official_category = 1;
    a0.forest_pct = 50;
    edge(
        &mut w,
        None,
        Some(b),
        &[(0.0, 0.0, 0.0), (500.0, 0.0, 0.0)],
        a0,
    );
    // Stored C -> B, travelled B -> C: grade band 2 forward reads as band 6.
    let mut a1 = attrs(Surface::Gravel, 500.0);
    a1.sac = 2;
    a1.urban = 1;
    a1.flags = flags::SURFACE_INFERRED;
    a1.grade = [0, 0, 255, 0, 0, 0, 0, 0, 0];
    edge(
        &mut w,
        None,
        Some(b),
        &[(1000.0, 0.0, 0.0), (500.0, 0.0, 0.0)],
        a1,
    );
    let g = Graph::from_bytes(w.finish()).unwrap();
    let start = snap(&g, at(100.0, 0.0));
    let end = snap(&g, at(900.0, 0.0));
    let p = Profile::default();
    let r = route(&g, &p, start, end, &empty(&g)).unwrap();
    assert_eq!(r.steps.len(), 2);
    assert!((r.distance_m - 800.0).abs() < 1.0);

    let s = route_stats(&g, &r, &p);
    let d = s.distance_m;
    for sum in [
        s.surface_m.iter().sum::<f64>(),
        s.grade_m.iter().sum(),
        s.sac_m.iter().sum(),
        s.urban_m.iter().sum(),
    ] {
        assert!((sum - d).abs() < 0.5, "{sum} vs {d}");
    }
    assert!((s.grade_m[6] - 400.0).abs() < 1.0, "{:?}", s.grade_m);
    assert!((s.surface_m[Surface::Asphalt as usize] - 400.0).abs() < 1.0);
    assert!((s.official_m - 400.0).abs() < 1.0);
    assert!((0.0..=1.0).contains(&s.official_share));
    assert!((s.forest_m - 200.0).abs() < 1.0);
    assert!((s.surface_inferred_m - 400.0).abs() < 1.0);
    assert_eq!(s.score, score(s.mean_factor_default));
    assert!(s.breakdown.surface > 1.0 && s.breakdown.surface < 2.0);

    // Bars measure the route, not how much the user cares (D73): heavier weights, same breakdown.
    let mut heavy = p.clone();
    heavy.weights.urban = 4.0;
    heavy.weights.surface = 3.0;
    heavy.weights.climb_pressure = 0.5;
    assert_eq!(route_stats(&g, &r, &heavy).breakdown, s.breakdown);
}

#[test]
fn partial_step_climb_from_geometry() {
    let g = straight();
    let start = snap(&g, at(0.0, 0.0));
    let end = snap(&g, at(500.0, 0.0));
    let r = route(&g, &Profile::default(), start, end, &empty(&g)).unwrap();
    assert!((r.climb_m - 50.0).abs() < 1.0, "{}", r.climb_m);
    assert_eq!(r.descent_m, 0.0);
}

#[test]
fn untraversable_start_edge_returns_none() {
    let mut w = GraphWriter::new(0);
    let mut a = attrs(Surface::Rock, 1000.0);
    a.sac = 5;
    edge(
        &mut w,
        None,
        None,
        &[(0.0, 0.0, 0.0), (1000.0, 0.0, 0.0)],
        a,
    );
    let g = Graph::from_bytes(w.finish()).unwrap();
    let start = snap(&g, at(200.0, 0.0));
    let end = snap(&g, at(800.0, 0.0));
    assert!(route(&g, &Profile::default(), start, end, &empty(&g)).is_none());
}

#[test]
fn partial_step_reports_the_last_partial_swing() {
    let mut w = GraphWriter::new(0);
    let pts = [
        (0.0, 0.0, 0.0),
        (100.0, 0.0, 1.0),
        (200.0, 0.0, 2.0),
        (300.0, 0.0, 3.0),
        (400.0, 0.0, 4.0),
        (500.0, 0.0, 4.0),
    ];
    // Stored geometry z is whole meters, so 0 -> 4 stands in for 0 -> 4.5; the old deadband dropped the last 1 m.
    edge(&mut w, None, None, &pts, attrs(Surface::Gravel, 500.0));
    let g = Graph::from_bytes(w.finish()).unwrap();
    let (start, end) = (snap(&g, at(0.0, 0.0)), snap(&g, at(450.0, 0.0)));
    let r = route(&g, &Profile::default(), start, end, &empty(&g)).unwrap();
    assert!((r.climb_m - 4.0).abs() < 1e-3, "{}", r.climb_m);
}

#[test]
fn route_cost_matches_plain_dijkstra() {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    use trail_calc::cost::edge_cost;

    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut rnd = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    const N: usize = 10;
    let mut w = GraphWriter::new(0);
    let nodes: Vec<u32> = (0..N * N)
        .map(|i| {
            let (x, y) = ((i % N) as f64 * 100.0, (i / N) as f64 * 100.0);
            w.add_node(at(x, y), rnd(40) as f32)
        })
        .collect();
    let surfaces = [
        Surface::Asphalt,
        Surface::Gravel,
        Surface::Dirt,
        Surface::Rock,
    ];
    for i in 0..N * N {
        for j in [i + 1, i + N] {
            if j >= N * N || (j == i + 1 && j % N == 0) {
                continue;
            }
            let mut a = attrs(surfaces[rnd(4)], 100.0);
            a.grade[rnd(9)] = 255;
            a.sac = 1 + rnd(3) as u8;
            let (pa, pb) = (w.node_pos(nodes[i]), w.node_pos(nodes[j]));
            let geom = [(pa.lat, pa.lon, 0.0), (pb.lat, pb.lon, 0.0)];
            w.add_edge(nodes[i], nodes[j], a, &geom);
        }
    }
    let g = Graph::from_bytes(w.finish()).unwrap();
    let p = Profile::default();
    let dijkstra = |src: u32, dst: u32| {
        let mut dist = vec![f64::INFINITY; g.node_count()];
        let mut heap = BinaryHeap::new();
        dist[src as usize] = 0.0;
        heap.push(Reverse((0f64.to_bits(), src)));
        while let Some(Reverse((d, u))) = heap.pop() {
            let d = f64::from_bits(d);
            if d > dist[u as usize] {
                continue;
            }
            for (e, dir) in g.neighbors(u) {
                let Some(c) = edge_cost(&g, e, dir, &p) else {
                    continue;
                };
                let v = match dir {
                    Direction::Forward => g.edge_to(e),
                    Direction::Backward => g.edge_from(e),
                };
                if d + c.cost < dist[v as usize] {
                    dist[v as usize] = d + c.cost;
                    heap.push(Reverse(((d + c.cost).to_bits(), v)));
                }
            }
        }
        dist[dst as usize]
    };
    let node_snap = |n: u32| {
        let e = (0..g.edge_count() as u32)
            .find(|&e| g.edge_from(e) == n || g.edge_to(e) == n)
            .unwrap();
        Snap {
            edge: e,
            t: if g.edge_from(e) == n { 0.0 } else { 1.0 },
            pos: g.node(n),
            dist_m: 0.0,
        }
    };
    for _ in 0..8 {
        let (a, b) = (nodes[rnd(N * N)], nodes[rnd(N * N)]);
        if a == b {
            continue;
        }
        let r = route(&g, &p, node_snap(a), node_snap(b), &empty(&g)).unwrap();
        let want = dijkstra(a, b);
        assert!(
            (r.cost - want).abs() <= 1e-6 * want,
            "{a}->{b}: {} vs {want}",
            r.cost
        );
    }
}

#[test]
fn backtracks_into_a_dead_end_are_dropped() {
    // A-B-C straight east, with a 300 m dead-end spur B-D going north.
    let mut w = GraphWriter::new(0);
    let a = edge(
        &mut w,
        None,
        None,
        &[(0.0, 0.0, 0.0), (500.0, 0.0, 0.0)],
        attrs(Surface::Gravel, 500.0),
    );
    let b = w.edge_endpoints(a).1;
    edge(
        &mut w,
        Some(b),
        None,
        &[(500.0, 0.0, 0.0), (1000.0, 0.0, 0.0)],
        attrs(Surface::Gravel, 500.0),
    );
    edge(
        &mut w,
        Some(b),
        None,
        &[(500.0, 0.0, 0.0), (500.0, 300.0, 0.0)],
        attrs(Surface::Gravel, 300.0),
    );
    let g = Graph::from_bytes(w.finish()).unwrap();
    let snaps = [
        snap(&g, at(100.0, 0.0)),
        snap(&g, at(500.0, 280.0)),
        snap(&g, at(900.0, 0.0)),
    ];
    let r = route_via(&g, &Profile::default(), &snaps).unwrap();
    assert!(
        (r.distance_m - 1360.0).abs() < 5.0,
        "spur is in the raw route: {}",
        r.distance_m
    );

    let t = drop_backtracks(&g, r);
    assert!(
        (t.distance_m - 800.0).abs() < 5.0,
        "spur removed: {}",
        t.distance_m
    );
    assert_eq!(t.steps.len(), 2);
}
