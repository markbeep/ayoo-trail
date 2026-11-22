use trail_calc::cost::*;
use trail_calc::graph::{
    flags, Direction, EdgeAttrs, Graph, GraphWriter, Highway, LatLon, Surface,
};

fn ll(lat: f64, lon: f64) -> LatLon {
    LatLon { lat, lon }
}

/// One edge from (46, 7) to (46, 7 + len_m east), straight, with the given attrs.
fn one_edge(len_m: f64, attrs: EdgeAttrs) -> Graph {
    let dlon = len_m / (111_195.0 * 46.0f64.to_radians().cos());
    let mut w = GraphWriter::new(0);
    let a = w.add_node(ll(46.0, 7.0), 0.0);
    let b = w.add_node(ll(46.0, 7.0 + dlon), 0.0);
    let attrs = EdgeAttrs {
        length_m: len_m as f32,
        ..attrs
    };
    w.add_edge(a, b, attrs, &[(46.0, 7.0, 0.0), (46.0, 7.0 + dlon, 0.0)]);
    Graph::from_bytes(w.finish()).unwrap()
}

fn gravel() -> EdgeAttrs {
    EdgeAttrs {
        surface: Surface::Gravel,
        sac: 1,
        ..Default::default()
    }
}

const F: Direction = Direction::Forward;
const B: Direction = Direction::Backward;

#[test]
fn steep_descent_is_penalized_only_forward() {
    let mut a = gravel();
    a.grade = [255, 0, 0, 0, 0, 0, 0, 0, 0];
    let g = one_edge(1000.0, a);
    let p = Profile::default();
    let f = edge_cost(&g, 0, F, &p).unwrap();
    let b = edge_cost(&g, 0, B, &p).unwrap();
    assert!((f.breakdown.slope - 2.5).abs() < 1e-5);
    assert!((b.breakdown.slope - 1.0).abs() < 1e-5);
    assert!(f.cost > b.cost);
}

#[test]
fn mixed_histogram_averages_band_factors() {
    let mut a = gravel();
    a.grade = [128, 0, 0, 0, 127, 0, 0, 0, 0];
    let g = one_edge(1000.0, a);
    let w = Weights::default();
    let f = edge_factors(&g, 0, F, &w);
    let b = edge_factors(&g, 0, B, &w);
    assert!((f.slope - 1.75).abs() < 0.01, "{}", f.slope);
    assert!((b.slope - 1.0).abs() < 0.01, "{}", b.slope);
}

#[test]
fn slope_preference_is_per_direction() {
    // 100 % of the edge in the steepest descent band forward (so steepest ascent backward).
    let mut a = gravel();
    a.grade = [255, 0, 0, 0, 0, 0, 0, 0, 0];
    let g = one_edge(1000.0, a);
    let with = |up: f32, down: f32| Weights {
        uphill: up,
        downhill: down,
        ..Weights::default()
    };

    let w = with(0.0, 0.0);
    assert!((edge_factors(&g, 0, F, &w).slope - 1.0).abs() < 1e-5);
    assert!((edge_factors(&g, 0, B, &w).slope - 1.0).abs() < 1e-5);

    let w = with(1.0, 1.0);
    assert!(
        edge_factors(&g, 0, F, &w).slope < 1.0,
        "steep descent rewarded"
    );
    assert!(
        edge_factors(&g, 0, B, &w).slope < 1.0,
        "steep climb rewarded"
    );

    let w = with(-1.0, -1.0);
    assert!(
        edge_factors(&g, 0, F, &w).slope > 2.0,
        "steep descent penalized"
    );
    assert!(
        edge_factors(&g, 0, B, &w).slope > 1.5,
        "steep climb penalized"
    );

    // Default is any up, gentle down (D45).
    assert_eq!(
        (Weights::default().uphill, Weights::default().downhill),
        (0.0, -1.0)
    );
    let w: Weights = serde_json::from_str(r#"{"uphill":1,"downhill":0}"#).unwrap();
    assert_eq!((w.uphill, w.downhill), (1.0, 0.0));
    // Halfway between any and steep is halfway between their factors (D53).
    let half = edge_factors(&g, 0, B, &with(0.5, 0.0)).slope;
    let steep = edge_factors(&g, 0, B, &with(1.0, 0.0)).slope;
    assert!(
        (half - (1.0 + steep) / 2.0).abs() < 1e-5,
        "{half} vs {steep}"
    );
}

#[test]
fn min_cost_per_m_stays_admissible_for_every_slope_preference() {
    for up in [-1.0, -0.4, 0.0, 0.7, 1.0] {
        for down in [-1.0, -0.4, 0.0, 0.7, 1.0] {
            for climb_pressure in [0.0, 0.5, 1.0] {
                let p = Profile {
                    weights: Weights {
                        uphill: up,
                        downhill: down,
                        climb_pressure,
                        ..Weights::default()
                    },
                    ..Profile::default()
                };
                let mut a = gravel();
                for band in 0..9 {
                    a.grade = [0; 9];
                    a.grade[band] = 255;
                    a.official_category = 1;
                    a.forest_pct = 100;
                    let g = one_edge(1000.0, a);
                    for dir in [F, B] {
                        let c = edge_cost(&g, 0, dir, &p).unwrap();
                        assert!(
                            c.per_m >= p.min_cost_per_m(),
                            "{up:?}/{down:?}/{climb_pressure} band {band}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn asphalt_costs_more_than_gravel() {
    let mut a = gravel();
    a.surface = Surface::Asphalt;
    let p = Profile::default();
    let asphalt = edge_cost(&one_edge(1000.0, a), 0, F, &p).unwrap();
    let gr = edge_cost(&one_edge(1000.0, gravel()), 0, F, &p).unwrap();
    assert!(asphalt.cost > gr.cost);
}

#[test]
fn weights_scale_around_neutral() {
    let mut a = gravel();
    a.official_category = 2;
    let g = one_edge(1000.0, a);
    let mut w = Weights::default();
    assert!((edge_factors(&g, 0, F, &w).official - 0.7).abs() < 1e-5);
    w.official = 0.0;
    assert!((edge_factors(&g, 0, F, &w).official - 1.0).abs() < 1e-6);
    // Weights are exponents (D51): 2 squares the factor, 4 lets one term dominate.
    w.official = 2.0;
    assert!((edge_factors(&g, 0, F, &w).official - 0.49).abs() < 1e-5);
    w.official = 4.0;
    assert!((edge_factors(&g, 0, F, &w).official - 0.2401).abs() < 1e-5);
}

#[test]
fn climb_pressure_makes_steep_edges_cheaper_than_flat_ones() {
    let mut steep = gravel();
    steep.grade = [0, 0, 0, 0, 0, 0, 0, 255, 0];
    let (gs, gf) = (one_edge(1000.0, steep), one_edge(1000.0, gravel()));
    let mut w = Weights::default();
    let base = edge_factors(&gs, 0, F, &w).slope;
    w.climb_pressure = 1.0;
    // Both directions: a loop descends what it climbs.
    for d in [F, B] {
        assert!(edge_factors(&gs, 0, d, &w).slope < 0.5 * edge_factors(&gf, 0, d, &w).slope);
    }
    assert!((edge_factors(&gs, 0, F, &w).slope - base * 0.2).abs() < 1e-5);
    // Flat edges are untouched, and slope weight 0 does not switch pressure off.
    assert!((edge_factors(&gf, 0, F, &w).slope - 1.0).abs() < 1e-6);
    w.slope = 0.0;
    assert!((edge_factors(&gs, 0, F, &w).slope - 0.2).abs() < 1e-5);
    let bad = Profile {
        weights: Weights {
            climb_pressure: 1.5,
            ..Weights::default()
        },
        ..Profile::default()
    };
    assert_eq!(bad.validate(), Err(ProfileError::WeightOutOfRange));
}

#[test]
fn missing_sac_penalized_only_when_flagged() {
    let mut a = gravel();
    a.sac = 0;
    a.flags = flags::MISSING_TAGS;
    let w = Weights::default();
    let g = one_edge(1000.0, a);
    assert!((edge_factors(&g, 0, F, &w).tech - 1.3).abs() < 1e-5);
    a.flags = 0;
    let g = one_edge(1000.0, a);
    assert!((edge_factors(&g, 0, F, &w).tech - 1.0).abs() < 1e-5);
    // sac 0 is never filtered
    assert!(traversable(&g, 0, &Profile::default()));
}

#[test]
fn sac_above_max_is_not_traversable() {
    let mut a = gravel();
    a.sac = 4;
    let g = one_edge(1000.0, a);
    let mut p = Profile::default();
    assert!(edge_cost(&g, 0, F, &p).is_none());
    p.max_sac = 4;
    assert!(edge_cost(&g, 0, F, &p).is_some());
}

#[test]
fn marker_penalizes_or_blocks() {
    let g = one_edge(1000.0, gravel());
    let mut p = Profile::default();
    let base = edge_cost(&g, 0, F, &p).unwrap();
    assert!(!base.penalized);
    p.exclude = vec![Exclusion {
        lat: 46.0,
        lon: 7.0,
        radius_m: 100.0,
        mode: ExclusionMode::Block,
    }];
    assert!(edge_cost(&g, 0, F, &p).is_none());
    p.exclude[0].mode = ExclusionMode::Penalize;
    let c = edge_cost(&g, 0, F, &p).unwrap();
    assert!(c.penalized);
    assert!((c.cost - base.cost * EXCLUSION_PENALTY).abs() < 1e-6);
    // far away marker: no effect
    p.exclude[0].lat = 46.1;
    assert!(!edge_cost(&g, 0, F, &p).unwrap().penalized);
}

#[test]
fn marker_in_the_middle_of_a_long_segment_still_blocks() {
    let g = one_edge(600.0, gravel());
    let dlon = 300.0 / (111_195.0 * 46.0f64.to_radians().cos());
    let mut p = Profile {
        exclude: vec![Exclusion {
            lat: 46.0,
            lon: 7.0 + dlon,
            radius_m: 100.0,
            mode: ExclusionMode::Block,
        }],
        ..Default::default()
    };
    assert!(edge_cost(&g, 0, F, &p).is_none());
    // 150 m north of the middle is outside a 100 m radius
    p.exclude[0].lat = 46.0 + 150.0 / 111_195.0;
    assert!(edge_cost(&g, 0, F, &p).is_some());
}

#[test]
fn extreme_weight_cannot_make_cost_non_positive() {
    let mut a = gravel();
    a.official_category = 1;
    a.forest_pct = 100;
    let g = one_edge(1000.0, a);
    let mut p = Profile::default();
    p.weights.official = 4.0;
    assert!(edge_cost(&g, 0, F, &p).unwrap().cost > 0.0);
    p.weights = Weights {
        slope: 4.0,
        surface: 4.0,
        tech: 4.0,
        official: 4.0,
        forest: 4.0,
        urban: 4.0,
        climb_pressure: 1.0,
        ..Weights::default()
    };
    let c = edge_cost(&g, 0, F, &p).unwrap();
    assert!(c.cost > 0.0);
    assert!(c.breakdown.official > 0.0);
}

#[test]
fn score_follows_d44() {
    assert!((score(0.63) - 100.0).abs() < 1e-4);
    // Geometric mean over the six terms (D44): twice the reference cost is 100 * 0.5^(1/6).
    assert!((score(1.26) - 89.089_87).abs() < 1e-3);
    assert!((score(0.63 * 64.0) - 50.0).abs() < 1e-3);
    assert_eq!(score(0.5), 100.0);
    assert_eq!(score(0.0), 0.0);
    assert_eq!(score(f64::NAN), 0.0);
    assert_eq!(score(-1.0), 0.0);
    assert_eq!(score(f64::INFINITY), 0.0);
}

#[test]
fn min_cost_per_m_is_a_lower_bound() {
    let surfaces = [
        Surface::Asphalt,
        Surface::Paved,
        Surface::Compacted,
        Surface::Gravel,
        Surface::Dirt,
        Surface::Grass,
        Surface::Rock,
        Surface::Unknown,
    ];
    let mut w = GraphWriter::new(0);
    let mut n = 0u32;
    for (i, &surface) in surfaces.iter().enumerate() {
        for band in 0..9 {
            let mut grade = [0u8; 9];
            grade[band] = 255;
            let a = EdgeAttrs {
                length_m: 100.0,
                surface,
                sac: (i % 7) as u8,
                official_category: (band % 4) as u8,
                forest_pct: (i * 14) as u8,
                urban: (band % 3) as u8,
                flags: if band % 2 == 0 {
                    flags::MISSING_TAGS
                } else {
                    0
                },
                grade,
                ..Default::default()
            };
            let p = ll(46.0, 7.0 + n as f64 * 0.001);
            let na = w.add_node(p, 0.0);
            let nb = w.add_node(ll(p.lat, p.lon + 0.0005), 0.0);
            w.add_edge(
                na,
                nb,
                a,
                &[(p.lat, p.lon, 0.0), (p.lat, p.lon + 0.0005, 0.0)],
            );
            n += 1;
        }
    }
    let g = Graph::from_bytes(w.finish()).unwrap();
    assert!(g.edge_count() >= 20);
    let all = |v: f32| Weights {
        slope: v,
        surface: v,
        tech: v,
        official: v,
        forest: v,
        urban: v,
        ..Weights::default()
    };
    for weights in [
        Weights::default(),
        all(4.0),
        all(0.0),
        Weights {
            climb_pressure: 1.0,
            ..all(2.5)
        },
    ] {
        let p = Profile {
            weights,
            max_sac: 6,
            exclude: vec![],
        };
        let lb = p.min_cost_per_m();
        assert!(lb > 0.0);
        for e in 0..g.edge_count() as u32 {
            for d in [F, B] {
                let c = edge_cost(&g, e, d, &p).unwrap();
                assert!(lb <= c.per_m + 1e-12, "lb {lb} > per_m {}", c.per_m);
            }
        }
    }
    let mut p = Profile::default();
    let base = p.min_cost_per_m();
    p.weights.official = 2.0;
    assert!(p.min_cost_per_m() < base);
}

#[test]
fn profile_json_defaults_and_validation() {
    let p: Profile = serde_json::from_str("{}").unwrap();
    assert_eq!(p, Profile::default());
    assert_eq!(p.max_sac, 3);
    assert!(p.validate().is_ok());

    let bad: Profile = serde_json::from_str(r#"{"weights":{"official":100.5}}"#).unwrap();
    assert_eq!(bad.validate(), Err(ProfileError::WeightOutOfRange));
    let nan = Profile {
        weights: Weights {
            slope: f32::NAN,
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(nan.validate(), Err(ProfileError::WeightOutOfRange));

    let sac: Profile = serde_json::from_str(r#"{"max_sac":7}"#).unwrap();
    assert_eq!(sac.validate(), Err(ProfileError::MaxSacOutOfRange));

    let ex: Profile = serde_json::from_str(
        r#"{"exclude":[{"lat":46.0,"lon":7.0,"radius_m":50,"mode":"block"}]}"#,
    )
    .unwrap();
    assert_eq!(ex.exclude[0].mode, ExclusionMode::Block);
    assert!(ex.validate().is_ok());
    let zero: Profile = serde_json::from_str(
        r#"{"exclude":[{"lat":46.0,"lon":7.0,"radius_m":0,"mode":"penalize"}]}"#,
    )
    .unwrap();
    assert_eq!(zero.validate(), Err(ProfileError::BadExclusion));
}

#[test]
fn technical_preference_avoids_ignores_or_seeks_harder_grades() {
    // D81: −1 avoid (the D31 table), 0 grade doesn't matter, +1 seek (its mirror); untagged stays 1.3.
    let tech = |sac: u8, t: f32| {
        let mut a = gravel();
        a.sac = sac;
        let w = Weights {
            technical: t,
            ..Weights::default()
        };
        edge_factors(&one_edge(1000.0, a), 0, F, &w).tech
    };
    let close = |a: f32, b: f32| (a - b).abs() < 1e-5;
    assert!(close(tech(1, -1.0), 1.0) && close(tech(3, -1.0), 1.15) && close(tech(5, -1.0), 1.4));
    assert!([1, 2, 3, 5].iter().all(|&s| close(tech(s, 0.0), 1.0)));
    assert!(close(tech(1, 1.0), 1.4) && close(tech(3, 1.0), 1.05) && close(tech(5, 1.0), 1.0));
    assert!(
        tech(3, 1.0) < tech(1, 1.0),
        "seeking makes T3 cheaper than T1"
    );
    assert_eq!(
        Weights::default().technical,
        -1.0,
        "default loops unchanged"
    );
    let bad: Profile = serde_json::from_str(r#"{"weights":{"technical":1.5}}"#).unwrap();
    assert_eq!(bad.validate(), Err(ProfileError::WeightOutOfRange));
}

#[test]
fn best_tech_factor_respects_the_grade_limit() {
    let seek = Weights {
        technical: 1.0,
        ..Weights::default()
    };
    assert!(
        (best_tech_factor(&seek, 3) - 1.05).abs() < 1e-5,
        "T3 is the best a T3 limit allows"
    );
    assert!((best_tech_factor(&seek, 6) - 1.0).abs() < 1e-5);
    assert!(
        (best_tech_factor(&Weights::default(), 3) - 1.0).abs() < 1e-5,
        "avoiding: T1"
    );
}

#[test]
fn busy_roads_cost_more_and_weight_zero_switches_it_off() {
    // D93: same asphalt, a main road costs more than a residential street.
    let road = |h: Highway| EdgeAttrs {
        surface: Surface::Asphalt,
        highway: h,
        ..Default::default()
    };
    let cost =
        |h: Highway, p: &Profile| edge_cost(&one_edge(1000.0, road(h)), 0, F, p).unwrap().cost;
    let p = Profile::default();
    let order = [
        Highway::Path,
        Highway::Residential,
        Highway::Tertiary,
        Highway::Secondary,
        Highway::Primary,
    ]
    .map(|h| cost(h, &p));
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    let mut off = Profile::default();
    off.weights.traffic = 0.0;
    assert_eq!(cost(Highway::Primary, &off), cost(Highway::Path, &off));
}

#[test]
fn prefer_marker_discounts_and_keeps_the_bound() {
    let g = one_edge(1000.0, gravel());
    let mut p = Profile::default();
    let base = edge_cost(&g, 0, F, &p).unwrap();
    let bound = p.min_cost_per_m();
    p.exclude = vec![Exclusion {
        lat: 46.0,
        lon: 7.0,
        radius_m: 100.0,
        mode: ExclusionMode::Prefer,
    }];
    let c = edge_cost(&g, 0, F, &p).unwrap();
    assert!((c.cost - base.cost * PREFER_FACTOR).abs() < 1e-6);
    assert!(c.per_m >= p.min_cost_per_m());
    assert!(p.min_cost_per_m() < bound);
    // Must areas don't change the cost.
    p.exclude[0].mode = ExclusionMode::Must;
    assert!((edge_cost(&g, 0, F, &p).unwrap().cost - base.cost).abs() < 1e-6);
    assert!((p.min_cost_per_m() - bound).abs() < 1e-12);
}

#[test]
fn at_most_five_must_areas() {
    let x = Exclusion {
        lat: 46.0,
        lon: 7.0,
        radius_m: 100.0,
        mode: ExclusionMode::Must,
    };
    let mut p = Profile {
        exclude: vec![x; 5],
        ..Profile::default()
    };
    assert!(p.validate().is_ok());
    p.exclude.push(x);
    assert_eq!(p.validate(), Err(ProfileError::BadExclusion));
}

#[test]
fn view_is_neutral_at_weight_zero_and_cheaper_with_a_better_view() {
    use trail_calc::import::ch1903::wgs84_to_lv95;
    use trail_calc::view::ViewGrid;
    let (e, n) = wgs84_to_lv95(46.0, 7.0);
    let factor = |score: u8, weight: f32| {
        let g = one_edge(100.0, gravel());
        // one 10 km cell around the edge
        g.set_view(&ViewGrid {
            e0: e - 5000.0,
            n0: n + 5000.0,
            cell: 10_000.0,
            w: 1,
            h: 1,
            v: vec![score],
        });
        let w = Weights {
            view: weight,
            ..Weights::default()
        };
        edge_factors(&g, 0, F, &w).view
    };
    assert_eq!(factor(255, 0.0), 1.0);
    assert_eq!(factor(0, 4.0), 1.0, "no data stays neutral");
    assert!(factor(255, 1.0) < factor(128, 1.0) && factor(128, 1.0) < factor(1, 1.0));
    assert_eq!(Weights::default().view, 0.0, "off by default (D100)");
}

#[test]
fn negative_weights_flip_a_preference() {
    let mut a = gravel();
    a.forest_pct = 100;
    let g = one_edge(1000.0, a);
    let cost = |forest: f32| {
        let p = Profile {
            weights: Weights {
                forest,
                ..Weights::default()
            },
            ..Profile::default()
        };
        edge_cost(&g, 0, F, &p).unwrap().cost
    };
    assert!(cost(-1.0) > cost(0.0));
    assert!(cost(1.0) < cost(0.0));
}

#[test]
fn weights_validate_in_the_wide_ranges() {
    let check = |w: Weights| {
        Profile {
            weights: w,
            ..Profile::default()
        }
        .validate()
    };
    let d = Weights::default();
    assert!(check(Weights {
        forest: -100.0,
        ..d
    })
    .is_ok());
    assert!(check(Weights { forest: 100.0, ..d }).is_ok());
    assert!(check(Weights {
        traffic: 6.64,
        view: -3.0,
        ..d
    })
    .is_ok());
    let bad = Err(ProfileError::WeightOutOfRange);
    assert_eq!(check(Weights { forest: 100.5, ..d }), bad);
    assert_eq!(check(Weights { slope: -0.1, ..d }), bad);
    assert_eq!(check(Weights { tech: -1.0, ..d }), bad);
    assert_eq!(
        check(Weights {
            forest: f32::NAN,
            ..d
        }),
        bad
    );
}

#[test]
fn min_cost_per_m_stays_admissible_for_negative_weights() {
    for w in [-50.0f32, -1.0, 1.0, 50.0] {
        let p = Profile {
            weights: Weights {
                surface: w,
                official: w,
                forest: w,
                urban: w,
                traffic: w,
                view: w,
                ..Weights::default()
            },
            ..Profile::default()
        };
        for (surface, official, forest, urban, highway) in [
            (Surface::Asphalt, 0, 0, 2, Highway::Primary),
            (Surface::Gravel, 1, 100, 0, Highway::Path),
        ] {
            let mut a = gravel();
            a.surface = surface;
            a.official_category = official;
            a.forest_pct = forest;
            a.urban = urban;
            a.highway = highway;
            let g = one_edge(1000.0, a);
            for dir in [F, B] {
                let c = edge_cost(&g, 0, dir, &p).unwrap();
                assert!(c.per_m >= p.min_cost_per_m(), "w {w}");
            }
        }
    }
}
