use std::collections::HashMap;

use trail_calc::cost::{Exclusion, ExclusionMode, Profile};
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
    // D87: when a loop fits the distance, only loops that fit come back.
    assert!(routes.iter().all(|r| r.distance_met));
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
    // D87: nothing fits, so only the single closest loop comes back, flagged.
    assert_eq!(routes.len(), 1);
    assert!(!routes[0].distance_met);
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

#[test]
fn loops_go_where_the_user_preferences_are_cheap() {
    // D82: the east half is T3, the west half T1. The default score avoids technical trails;
    // a user who seeks them strongly should get loops that run mostly east.
    let (n, step) = (21, 200.0);
    let mut w = grid_writer(n, step);
    for e in 0..w.edge_count() as u32 {
        let mid = at(2000.0, 0.0).lon + 1e-9;
        if w.edge_geometry(e).iter().any(|&(_, lon, _)| lon > mid) {
            w.edge_attrs_mut(e).sac = 3;
        }
    }
    let g = Graph::from_bytes(w.finish()).unwrap();
    let mut r = req(at(2000.0, 2000.0), 6000.0);
    r.profile.weights.technical = 1.0;
    r.profile.weights.tech = 4.0;
    let routes = generate(&g, &r).unwrap();
    let east = |gr: &GeneratedRoute| {
        let cov = coverage(&g, gr);
        let total: f64 = cov.values().sum();
        cov.iter()
            .filter(|(e, _)| g.edge_attrs(**e).sac == 3)
            .map(|(_, m)| m)
            .sum::<f64>()
            / total
    };
    let best = routes
        .iter()
        .find(|r| r.distance_met)
        .expect("a loop in tolerance");
    assert!(
        east(best) >= 0.8,
        "best loop only {:.0} % on the side the user prefers",
        100.0 * east(best)
    );
}

fn area(c: LatLon, radius_m: f32, mode: ExclusionMode) -> Exclusion {
    Exclusion {
        lat: c.lat,
        lon: c.lon,
        radius_m,
        mode,
    }
}

/// Closest approach of a loop to `p`, in meters.
fn closest(g: &Graph, r: &GeneratedRoute, p: LatLon) -> f64 {
    route_geometry(g, &r.route)
        .into_iter()
        .map(|(lat, lon, _)| haversine_m(LatLon { lat, lon }, p))
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn every_loop_passes_the_must_pass_areas() {
    let g = grid(21, 200.0);
    let mut r = req(at(2000.0, 2000.0), 6000.0);
    let must = at(1300.0, 2900.0);
    r.profile.exclude = vec![area(must, 150.0, ExclusionMode::Must)];
    let routes = generate(&g, &r).unwrap();
    assert!(!routes.is_empty());
    for l in &routes {
        assert!(
            closest(&g, l, must) <= 150.0,
            "loop misses the must-pass area"
        );
        assert!(l.distance_met);
    }
}

#[test]
fn must_pass_too_far_for_the_distance_is_an_error() {
    let g = grid(21, 200.0);
    let mut r = req(at(200.0, 200.0), 4000.0);
    r.profile.exclude = vec![area(at(3800.0, 3800.0), 100.0, ExclusionMode::Must)];
    match generate(&g, &r) {
        Err(GenerateError::MustPassTooFar { min_m }) => {
            assert!((14_000.0..16_000.0).contains(&min_m), "min_m {min_m}")
        }
        other => panic!("expected MustPassTooFar, got {:?}", other.map(|v| v.len())),
    }
}

#[test]
fn must_pass_off_the_network_is_an_error() {
    let g = grid(21, 200.0);
    let mut r = req(at(2000.0, 2000.0), 6000.0);
    r.profile.exclude = vec![area(at(50_000.0, 50_000.0), 100.0, ExclusionMode::Must)];
    assert!(matches!(
        generate(&g, &r),
        Err(GenerateError::MustPassUnreachable)
    ));
}

#[test]
fn must_pass_behind_a_hard_avoid_is_an_error() {
    let g = grid(21, 200.0);
    let mut r = req(at(2000.0, 2000.0), 6000.0);
    let must = at(3600.0, 3600.0);
    r.profile.exclude = vec![
        area(must, 100.0, ExclusionMode::Must),
        area(must, 500.0, ExclusionMode::Block),
    ];
    assert!(matches!(
        generate(&g, &r),
        Err(GenerateError::MustPassUnreachable)
    ));
}

#[test]
fn hard_avoids_that_wall_in_the_start_give_no_loop() {
    // One block circle covers the whole grid but the two edges at the start corner.
    let g = grid(21, 200.0);
    let mut r = req(at(0.0, 0.0), 6000.0);
    r.profile.exclude = vec![area(at(2400.0, 2400.0), 3200.0, ExclusionMode::Block)];
    assert!(matches!(generate(&g, &r), Err(GenerateError::NoLoop)));
}

#[test]
fn soft_prefer_areas_pull_loops_in() {
    let g = grid(21, 200.0);
    let target = at(3200.0, 1000.0);
    let mut r = req(at(2000.0, 2000.0), 6000.0);
    let near = |r: &GenerateRequest| closest(&g, &generate(&g, r).unwrap()[0], target);
    let before = near(&r);
    r.profile.exclude = vec![area(target, 400.0, ExclusionMode::Prefer)];
    let after = near(&r);
    assert!(
        after <= 400.0 && after < before,
        "before {before:.0} m, after {after:.0} m"
    );
}
