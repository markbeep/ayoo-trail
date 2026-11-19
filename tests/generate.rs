use std::collections::HashMap;

use trail_calc::cost::Profile;
use trail_calc::generate::{generate, GenerateError, GenerateRequest, GeneratedRoute};
use trail_calc::geodesy::haversine_m;
use trail_calc::graph::{EdgeAttrs, Graph, GraphWriter, LatLon, Surface};
use trail_calc::route::route_geometry;

const LAT: f64 = 46.0;
const LON: f64 = 7.0;
const M_LAT: f64 = 111_195.0;

/// Point `x` meters east and `y` meters north of (LAT, LON).
fn at(x: f64, y: f64) -> LatLon {
    LatLon {
        lat: LAT + y / M_LAT,
        lon: LON + x / (M_LAT * LAT.to_radians().cos()),
    }
}

/// n×n gravel grid with `step` meter spacing, origin at (0, 0).
fn grid(n: usize, step: f64) -> Graph {
    Graph::from_bytes(grid_writer(n, step).finish()).unwrap()
}

fn grid_writer(n: usize, step: f64) -> GraphWriter {
    let mut w = GraphWriter::new(0);
    let ids: Vec<Vec<u32>> = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| w.add_node(at(i as f64 * step, j as f64 * step), 500.0))
                .collect()
        })
        .collect();
    let a = EdgeAttrs {
        length_m: step as f32,
        surface: Surface::Gravel,
        sac: 1,
        ..Default::default()
    };
    let link = |w: &mut GraphWriter, (i, j): (usize, usize), (k, l): (usize, usize)| {
        let (p, q) = (
            at(i as f64 * step, j as f64 * step),
            at(k as f64 * step, l as f64 * step),
        );
        w.add_edge(
            ids[i][j],
            ids[k][l],
            a,
            &[(p.lat, p.lon, 500.0), (q.lat, q.lon, 500.0)],
        );
    };
    for i in 0..n {
        for j in 0..n {
            if i + 1 < n {
                link(&mut w, (i, j), (i + 1, j));
            }
            if j + 1 < n {
                link(&mut w, (i, j), (i, j + 1));
            }
        }
    }
    w
}

fn req(start: LatLon, distance_m: f64) -> GenerateRequest {
    GenerateRequest {
        start,
        distance_m,
        climb: None,
        profile: Profile::default(),
        count: 6,
    }
}

/// Edge id → meters covered.
fn coverage(g: &Graph, r: &GeneratedRoute) -> HashMap<u32, f64> {
    let mut m = HashMap::new();
    for s in &r.route.steps {
        *m.entry(s.edge).or_insert(0.0) += s.fraction() * g.edge_len_m(s.edge);
    }
    m
}

#[test]
fn loops_start_and_end_at_the_start_and_hit_the_distance() {
    let g = grid(21, 200.0);
    let start = at(2000.0, 2000.0);
    let routes = generate(&g, &req(start, 6000.0)).unwrap();
    assert!(!routes.is_empty());
    assert!(routes.iter().any(|r| r.distance_met));
    for r in &routes {
        let geom = route_geometry(&g, &r.route);
        let first = LatLon {
            lat: geom[0].0,
            lon: geom[0].1,
        };
        let last = LatLon {
            lat: geom[geom.len() - 1].0,
            lon: geom[geom.len() - 1].1,
        };
        assert!(haversine_m(first, start) < 5.0);
        assert!(haversine_m(last, start) < 5.0);
        assert!(r.waypoints.len() >= 3);
        assert_eq!(
            r.distance_met,
            (r.route.distance_m - 6000.0).abs() / 6000.0 <= 0.10
        );
    }
}

#[test]
fn impossible_target_returns_closest_with_flag() {
    let g = grid(5, 200.0);
    let routes = generate(&g, &req(at(400.0, 400.0), 40_000.0)).unwrap();
    assert!(!routes.is_empty());
    assert!(routes.iter().all(|r| !r.distance_met));
}

#[test]
fn routes_overlap_at_most_half() {
    let g = grid(21, 200.0);
    let routes = generate(&g, &req(at(2000.0, 2000.0), 6000.0)).unwrap();
    assert!(routes.len() >= 2, "need several loops to compare");
    let cov: Vec<_> = routes.iter().map(|r| coverage(&g, r)).collect();
    for i in 0..routes.len() {
        for j in (i + 1)..routes.len() {
            let shared: f64 = cov[i]
                .iter()
                .filter_map(|(e, a)| cov[j].get(e).map(|b| a.min(*b)))
                .sum();
            let shorter = routes[i].route.distance_m.min(routes[j].route.distance_m);
            assert!(
                shared / shorter <= 0.5,
                "routes {i} and {j} overlap {}",
                shared / shorter
            );
        }
    }
}

#[test]
fn off_network_start_is_an_error() {
    let g = grid(5, 200.0);
    assert!(matches!(
        generate(&g, &req(at(50_000.0, 50_000.0), 5000.0)),
        Err(GenerateError::StartOffNetwork)
    ));
}

#[test]
fn start_next_to_an_island_snaps_to_the_main_network() {
    // A 10 m stairs island 10 m from the start, the grid row 30 m away (Zürich HB in the CH graph).
    let mut w = grid_writer(15, 200.0);
    let (p, q) = (at(1050.0, 1230.0), at(1060.0, 1230.0));
    let (a, b) = (w.add_node(p, 500.0), w.add_node(q, 500.0));
    let attrs = EdgeAttrs {
        length_m: 10.0,
        surface: Surface::Gravel,
        ..Default::default()
    };
    w.add_edge(a, b, attrs, &[(p.lat, p.lon, 500.0), (q.lat, q.lon, 500.0)]);
    let g = Graph::from_bytes(w.finish()).unwrap();

    let loops = generate(&g, &req(at(1055.0, 1220.0), 6_000.0)).unwrap();
    assert!(!loops.is_empty(), "start stayed on the island");
}

#[test]
fn components_follow_the_sac_limit() {
    // Grid node 0 –(T5 edge)– x –(easy edge)– y: x–y is an island unless T5 is allowed.
    let mut w = grid_writer(3, 200.0);
    let (px, py) = (at(-200.0, 0.0), at(-400.0, 0.0));
    let (x, y) = (w.add_node(px, 500.0), w.add_node(py, 500.0));
    let edge = |sac| EdgeAttrs {
        length_m: 200.0,
        surface: Surface::Gravel,
        sac,
        ..Default::default()
    };
    let o = at(0.0, 0.0);
    w.add_edge(
        0,
        x,
        edge(5),
        &[(o.lat, o.lon, 500.0), (px.lat, px.lon, 500.0)],
    );
    let island = w.add_edge(
        x,
        y,
        edge(1),
        &[(px.lat, px.lon, 500.0), (py.lat, py.lon, 500.0)],
    );
    let g = Graph::from_bytes(w.finish()).unwrap();

    assert_ne!(g.edge_component(island, 3), g.main_component(3));
    assert_eq!(g.edge_component(island, 5), g.main_component(5));
}
