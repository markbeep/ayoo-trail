//! Generation timings on the CH graph: `cargo build --release --example prof &&
//! RAYON_NUM_THREADS=1 ./target/release/examples/prof` (KM=20 by default).

use std::path::Path;
use std::time::Instant;

use trail_calc::cost::Profile;
use trail_calc::generate::{generate, ClimbGoal, GenerateRequest};
use trail_calc::graph::{Graph, LatLon};

fn main() {
    let g = Graph::open(Path::new("data/graph.bin")).unwrap();
    g.main_component(3);
    let km: f64 = std::env::var("KM")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20.0);
    let places = [
        ("Wattwil", 47.2985, 9.095),
        ("Bichelsee", 47.448142, 8.922039),
        ("Zurich HB", 47.3779, 8.5403),
        ("Geneva", 46.2044, 6.1432),
    ];
    for (name, lat, lon) in places {
        for climb in [
            None,
            Some(ClimbGoal {
                target_m: Some(km * 60.0),
                strictness: 0.8,
            }),
        ] {
            let req = GenerateRequest {
                start: LatLon { lat, lon },
                distance_m: km * 1000.0,
                climb,
                profile: Profile::default(),
                count: 3,
            };
            let mut best = f64::MAX;
            let (mut n, mut loops) = (0, String::new());
            for _ in 0..3 {
                let t = Instant::now();
                let r = generate(&g, &req).unwrap_or_default();
                n = r.len();
                loops = r
                    .iter()
                    .map(|l| format!("{:.0}m/{:.0}up", l.route.distance_m, l.route.climb_m))
                    .collect::<Vec<_>>()
                    .join(" ");
                best = best.min(t.elapsed().as_secs_f64() * 1e3);
            }
            println!(
                "{name:10} climb={:5} {best:8.1} ms  {n} loops  {loops}",
                climb.is_some()
            );
        }
    }
}
