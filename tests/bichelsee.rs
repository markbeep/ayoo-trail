//! Owner's real 15 km / ~900 m run from Bichelsee (tests/fixtures/bichelsee.gpx): the generator
//! must find a loop like it. Needs a CH `data/graph.bin`:
//! `cargo test --release --test bichelsee -- --ignored --nocapture`.

use std::path::Path;

use navilau::cost::Profile;
use navilau::generate::{generate, ClimbGoal, GenerateRequest};
use navilau::geodesy::haversine_m;
use navilau::graph::{Graph, LatLon};
use navilau::route::{route_stats, route_via};

const START: LatLon = LatLon {
    lat: 47.448142,
    lon: 8.922039,
};

fn gpx_points() -> Vec<LatLon> {
    let t = std::fs::read_to_string("tests/fixtures/bichelsee.gpx").unwrap();
    t.split("<trkpt ")
        .skip(1)
        .map(|s| {
            let attr = |k: &str| -> f64 {
                let i = s.find(k).unwrap() + k.len();
                s[i..i + s[i..].find('"').unwrap()].parse().unwrap()
            };
            LatLon {
                lat: attr("lat=\""),
                lon: attr("lon=\""),
            }
        })
        .collect()
}

#[test]
#[ignore = "needs a CH data/graph.bin"]
fn finds_a_loop_like_the_bichelsee_run() {
    let g = Graph::open(Path::new("data/graph.bin")).unwrap();
    let p = Profile::default();

    // Baseline: the owner's run, re-routed through points every ~1 km.
    let mut pts = vec![START];
    for q in gpx_points() {
        if haversine_m(*pts.last().unwrap(), q) >= 1000.0 {
            pts.push(q);
        }
    }
    pts.push(START);
    let snaps: Vec<_> = pts
        .iter()
        .map(|&q| g.snap(q).expect("gpx point off the graph"))
        .collect();
    let run = route_via(&g, &p, &snaps).expect("gpx not routable");
    let s = route_stats(&g, &run, &p);
    println!(
        "run: {:.1} km, {:.0} m up, cost/m {:.3}, score {:.0}",
        run.distance_m / 1e3,
        run.climb_m,
        run.cost / run.distance_m,
        s.score
    );

    let req = GenerateRequest {
        start: START,
        distance_m: 15_000.0,
        climb: Some(ClimbGoal {
            target_m: Some(900.0),
            strictness: 0.5,
        }),
        profile: p.clone(),
        count: 6,
    };
    let loops = generate(&g, &req).unwrap();
    for l in &loops {
        println!(
            "loop: {:.1} km, {:.0} m up, cost/m {:.3}",
            l.route.distance_m / 1e3,
            l.route.climb_m,
            l.route.cost / l.route.distance_m
        );
    }
    assert!(
        loops
            .iter()
            .any(|l| (l.route.distance_m - 15_000.0).abs() <= 1_500.0
                && (l.route.climb_m - 900.0).abs() <= 135.0),
        "no loop within ±10 % distance and ±15 % climb"
    );

    // More strictness, or "as much as possible", never finds less climb.
    let best = |climb: Option<ClimbGoal>| {
        let r = GenerateRequest {
            climb,
            ..GenerateRequest {
                start: START,
                distance_m: 15_000.0,
                climb: None,
                profile: p.clone(),
                count: 6,
            }
        };
        generate(&g, &r)
            .unwrap()
            .iter()
            .map(|l| l.route.climb_m)
            .fold(0.0, f64::max)
    };
    let max = |strictness| {
        best(Some(ClimbGoal {
            target_m: None,
            strictness,
        }))
    };
    let (none, low, high) = (best(None), max(0.3), max(1.0));
    println!("max climb: none {none:.0}, max@0.3 {low:.0}, max@1 {high:.0}");
    assert!(none <= low && low <= high);
}
