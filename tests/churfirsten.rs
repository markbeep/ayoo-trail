//! A loop from Alt St. Johann over all seven Churfirsten with every cost off and the shortest
//! distance (D110). Needs a CH `data/graph.bin`:
//! `cargo test --release --test churfirsten -- --ignored --nocapture`. Writes
//! `target/churfirsten.gpx` for review.

use std::collections::HashMap;
use std::path::Path;

use trail_calc::cost::{Exclusion, ExclusionMode, Profile, Weights};
use trail_calc::generate::{generate, GenerateError, GenerateRequest};
use trail_calc::geodesy::haversine_m;
use trail_calc::graph::{Graph, LatLon};
use trail_calc::route::route_geometry;

const fn ll(lat: f64, lon: f64) -> LatLon {
    LatLon { lat, lon }
}

/// OSM place node 2312185934.
const START: LatLon = ll(47.194093, 9.284057);
/// OSM natural=peak nodes, west to east.
const PEAKS: [(&str, LatLon); 7] = [
    ("Selun", ll(47.152584, 9.255506)),        // 362777522
    ("Frümsel", ll(47.152323, 9.266052)),      // 452385867
    ("Brisi", ll(47.153190, 9.277424)),        // 452385868
    ("Zuestoll", ll(47.154100, 9.285479)),     // 452385869
    ("Schibenstoll", ll(47.154355, 9.295485)), // 452385870
    ("Hinterrugg", ll(47.153635, 9.305254)),   // 452385871
    ("Chäserrugg", ll(47.154808, 9.312775)),   // 243845595
];

fn request(max_sac: u8, distance_m: f64) -> GenerateRequest {
    let off = Weights {
        slope: 0.0,
        uphill: 0.0,
        downhill: 0.0,
        surface: 0.0,
        tech: 0.0,
        technical: 0.0,
        official: 0.0,
        forest: 0.0,
        urban: 0.0,
        traffic: 0.0,
        view: 0.0,
        climb_pressure: 0.0,
    };
    GenerateRequest {
        start: START,
        distance_m,
        climb: None,
        profile: Profile {
            weights: off,
            max_sac,
            exclude: PEAKS
                .iter()
                .map(|(_, p)| Exclusion {
                    lat: p.lat,
                    lon: p.lon,
                    radius_m: 100.0,
                    mode: ExclusionMode::Must,
                })
                .collect(),
            ..Profile::default()
        },
        count: 6,
    }
}

#[test]
#[ignore = "needs a CH data/graph.bin"]
fn loops_over_the_seven_churfirsten() {
    let g = Graph::open(Path::new("data/graph.bin")).unwrap();
    // The easiest grade that reaches every summit, and the shortest loop over them.
    let (sac, min_m) = (3..=6)
        .find_map(|sac| match generate(&g, &request(sac, 5000.0)) {
            Err(GenerateError::MustPassTooFar { min_m }) => Some((sac, min_m)),
            Err(GenerateError::MustPassUnreachable) => None,
            other => panic!("T{sac}: {:?}", other.map(|v| v.len())),
        })
        .expect("no grade up to T6 reaches every summit");
    let routes = generate(&g, &request(sac, min_m)).unwrap();
    let r = &routes[0];
    let geom: Vec<LatLon> = route_geometry(&g, &r.route)
        .into_iter()
        .map(|(lat, lon, _)| LatLon { lat, lon })
        .collect();
    println!(
        "T{sac}: {:.1} km (min {:.1}), {:.0} m up, {} waypoints",
        r.route.distance_m / 1e3,
        min_m / 1e3,
        r.route.climb_m,
        r.waypoints.len()
    );

    let gpx: String = geom
        .iter()
        .map(|p| format!("<trkpt lat=\"{}\" lon=\"{}\"/>", p.lat, p.lon))
        .collect();
    std::fs::write(
        "target/churfirsten.gpx",
        format!("<?xml version=\"1.0\"?><gpx version=\"1.1\" creator=\"ayoo\"><trk><name>Churfirsten</name><trkseg>{gpx}</trkseg></trk></gpx>"),
    )
    .unwrap();

    for (name, p) in PEAKS {
        let d = geom
            .iter()
            .map(|q| haversine_m(*q, p))
            .fold(f64::INFINITY, f64::min);
        println!("{name}: {d:.0} m");
        assert!(d <= 50.0, "{name} is {d:.0} m off the loop");
    }
    assert!(r.route.distance_m <= min_m * 1.1);

    // Meters on edges run more than once, away from the summit spurs and the start.
    let mut cov: HashMap<u32, f64> = HashMap::new();
    for s in &r.route.steps {
        *cov.entry(s.edge).or_default() += s.fraction();
    }
    let near = |e: u32| {
        let a = g.node(g.edge_from(e));
        haversine_m(a, START) < 1500.0 || PEAKS.iter().any(|(_, p)| haversine_m(a, *p) < 1500.0)
    };
    let reused: f64 = cov
        .iter()
        .filter(|(e, f)| **f > 1.01 && !near(**e))
        .map(|(e, f)| (f - 1.0) * g.edge_len_m(*e))
        .sum();
    println!("reused away from summits: {:.0} m", reused);
    assert!(reused < 0.1 * r.route.distance_m);
}
