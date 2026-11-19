# navilau Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a personal web app that generates trail-running loop routes in Switzerland from a start point, a distance target and a soft hilliness preference, with drag editing and GPX export.

**Architecture:** One Rust crate with two binaries. `navilau-import` turns OSM + DEM + official-trail data into one mmap'd graph file; `navilau-server` serves an axum API and the static SolidJS + MapLibre client from that graph. Routing is A* over the graph with a request-time cost function; loop generation is anchor seeding plus local refinement.

**Tech Stack:** Rust (axum, tokio, osmpbf, rstar, memmap2, rayon, serde, clap), SolidJS + TypeScript + MapLibre GL + Vite, GDAL/ogr2ogr for data prep only, Docker.

**Spec:** `docs/design.md`. Companions: `docs/brief.md`, `docs/decisions.md`, `docs/layers.md`, `docs/routing-engines.md`.

> **Revision 2026-11-18 (D29–D37).** Tasks 1–8 and 11 run now; 9, 10, 12 wait. Where this text and the decisions differ, the decisions win:
> - Task 1–2: format adds a CSR adjacency section, `grade[9]` histogram and `highway` byte per edge, bbox in the header; `bytemuck` zero-copy reads. `flatgeobuf`, `blake3`, `lru` are not needed yet.
> - Task 3: R-tree over geometry segments; snap takes a traversability predicate.
> - Task 4: slope cost is the length-weighted band factor over `grade`; missing-sac penalty only on `path`/`bridleway`; default `max_sac` 3; score is `100·min(0.63/mean_factor_default, 1)` per metre (the `route_score` tests below are wrong and are replaced).
> - Task 6: highway inference for surface (D31); forest includes multipolygons, excludes `landuse=meadow`.
> - Task 7: no `split_edges_at_extrema` (D30); DEM behind a trait with swissALTI3D tiles (default) and DHM25 ASCII; 10 m sampling; bridge/tunnel interpolation; Z densification.
> - Task 8: official trails read from the GPKG with `rusqlite` and matched in LV95; no GDAL.
> - Task 11: server starts without a graph (`/api/route` → 503); health adds `bbox`.
>
> **Revision 2026-11-19 (D39–D42).** Tasks 9, 10 and the endpoint half of 12 run now. Generation is triangle seeds with radius rescaling (D39), not anchor ranking plus anchor insertion; there is no refinement round and no response cache yet. The frontend gets an OSM vector default base map, hillshade, and toggleable overlays (D40–D42).

## Global Constraints

- Rust stable, edition 2021. Node 26.13.1 and just pinned in `mise.toml`.
- Units: distances in meters, elevations in meters, coordinates WGS84 (EPSG:4326). API JSON uses snake_case.
- Server listens on `0.0.0.0:8080`. Data directory from `DATA_DIR`, default `./data`.
- Graph is built from the Switzerland OSM extract only. No cross-border routing.
- No accounts, no database, no tile proxy, no CH. Base tiles are fetched by the browser from `wmts.geo.admin.ch`.
- Attribution strings: `©swisstopo` under the map, `© OpenStreetMap contributors` in the footer.
- `cargo fmt` clean and `cargo clippy --all-targets -- -D warnings` clean before every commit.
- No `unsafe` outside `memmap2`.
- Every fallible public function returns `Result`; no panics on bad input.
- Routing is forward A* only, never bidirectional. `Route` is a list of `RouteStep`s, never bare edge ids.
- The DEM stays in its native Swiss grid; no raster reprojection at import.
- After every task's `jj commit`, advance the working bookmark: `jj bookmark set main -r @-`.

## Review Focus

The five failure modes most likely to bite a user, each pinned by a test in the task that owns the code:

1. Start point in water, a building, or off-network → snap returns `None` → API `422` with a clear message, never a panic. (Task 11)
2. Target distance unreachable (tiny village, 40 km request) → closest routes returned with `distance_met: false`, never an empty list or a hang. (Task 10)
3. Disconnected graph or an island trail → A* returns `None` → the seed is skipped; another seed still succeeds. (Tasks 5, 9)
4. Malformed or missing OSM tags (`sac_scale=foo`, unknown `surface`) → treated as unknown and penalised, no panic. (Task 8)
5. An exclusion marker covering the start or the only path → `422` "start is blocked", not a silent empty result. (Task 11)

## Milestones

- [ ] **M1 — Engine core:** tasks 1–5. Graph file, snap, cost, A*. Runs on fixture data.
- [ ] **M2 — Import:** tasks 6–8. Real Swiss OSM + DEM + attributes into `graph.bin`.
- [ ] **M3 — Generator:** tasks 9–10. Anchors, seeds, repair, refinement, dedup.
- [ ] **M4 — API:** tasks 11–12. `health`, `route`, `generate`, cache.
- [ ] **M5 — Frontend:** tasks 13–14. Map, form, cards, GPX, drag editing.
- [ ] **M6 — Ship:** tasks 15–16. Docker image, compose stack, end-to-end smoke.

---

### Task 1: Crate skeleton and graph round-trip

**Files:**
- Create: `Cargo.toml`, `src/lib.rs`, `src/graph.rs`, `tests/graph.rs`, `justfile`
- Modify: `mise.toml`, `AGENTS.md` (record that code now exists)

**Interfaces:**
- Consumes: nothing.
- Produces: `navilau::graph::{LatLon, Surface, Flags, EdgeAttrs, GraphWriter, Graph, GraphError}`. `GraphWriter::new(osm_date: i64)`, `add_node(LatLon, f32) -> u32`, `add_edge(u32, u32, EdgeAttrs, geometry: &[(f64, f64, f32)]) -> u32`, `node_mut(u32) -> &mut Node`, `edge_mut(u32) -> &mut Edge`, `add_anchor(LatLon, AnchorKind, f32)`, `write_to(&Path) -> Result<(), GraphError>`, `finish() -> Vec<u8>`. `Graph::from_bytes(Vec<u8>) -> Result<Graph, GraphError>`.

- [ ] **Step 1: Create the crate and the failing round-trip test**

`Cargo.toml`:

```toml
[package]
name = "navilau"
version = "0.1.0"
edition = "2021"

[lib]
name = "navilau"
path = "src/lib.rs"

[[bin]]
name = "navilau-import"
path = "src/bin/import.rs"

[[bin]]
name = "navilau-server"
path = "src/bin/server.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
memmap2 = "0.9"
rstar = "0.12"
rayon = "1"
lru = "0.12"
parking_lot = "0.12"
axum = "0.8"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "signal"] }
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
osmpbf = "0.3"
roxmltree = "0.20"
geo = "0.29"
flatgeobuf = { version = "4", default-features = false, features = ["geozero"] }
blake3 = "1"

[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
tempfile = "3"
```

`mise.toml` becomes:

```toml
[tools]
rust = "1.100.0"
just = "latest"
node = "26.13.1"
```

`tests/graph.rs`:

```rust
use navilau::graph::{EdgeAttrs, Graph, GraphWriter, LatLon, Surface};

#[test]
fn writer_open_round_trip() {
    let mut w = GraphWriter::new(1_700_000_000);
    let a = w.add_node(LatLon { lat: 46.9, lon: 7.4 }, 540.0);
    let b = w.add_node(LatLon { lat: 46.91, lon: 7.41 }, 610.0);
    let attrs = EdgeAttrs {
        length_m: 1400.0,
        gain: 70,
        loss: 0,
        surface: Surface::Gravel,
        sac: 1,
        forest_pct: 80,
        flags: 0,
        urban: 0,
        official_category: 0,
    };
    w.add_edge(a, b, attrs, &[(46.9, 7.4, 540.0), (46.91, 7.41, 610.0)]);
    let g = Graph::from_bytes(w.finish()).unwrap();

    assert_eq!(g.node_count(), 2);
    assert_eq!(g.edge_count(), 1);
    assert_eq!(g.osm_date(), 1_700_000_000);
    assert_eq!(g.node(a), LatLon { lat: 46.9, lon: 7.4 });
    assert!((g.edge_len_m(0) - 1400.0).abs() < 0.01);
    assert_eq!(g.edge_attrs(0).surface, Surface::Gravel);
    assert_eq!(g.edge_geometry(0).count(), 2);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --test graph`
Expected: compile error, `unresolved import navilau::graph`.

- [ ] **Step 3: Implement the file format**

`src/graph.rs`: a 40-byte little-endian header (`magic [u8;4] = *b"TRGR"`, `version u32 = 1`, `osm_date i64`, counts for nodes, edges, geometry and anchors, reserved padding), followed by struct-of-arrays sections in this order, each padded to 8-byte alignment: `node_lat f32`, `node_lon f32`, `node_elev i16`, `edge_from u32`, `edge_to u32`, `edge_len_cm u32`, `edge_gain i16`, `edge_loss i16`, `edge_surface u8`, `edge_sac u8`, `edge_forest u8`, `edge_flags u8`, `edge_urban u8`, `edge_official_category u8`, `edge_geom_start u32`, `edge_geom_len u32`, `geom_lat f32`, `geom_lon f32`, `geom_z i16`, `anchor_lat f32`, `anchor_lon f32`, `anchor_kind u8`, `anchor_weight f32`.

`GraphWriter` is a staged builder, not an in-memory file. It owns struct-of-arrays `Vec<Node>`, `Vec<Edge>`, `Vec<Geom>` and an anchor list, exposes `node_mut`/`edge_mut` for the later import passes, and `write_to` streams each section into the destination file. `finish()` materialises a `Vec<u8>` only for tests. The Swiss import therefore holds one SoA copy plus a small write buffer, not a second full copy of the graph.

`Surface` discriminants: `Asphalt=0, Paved=1, Compacted=2, Gravel=3, Dirt=4, Grass=5, Rock=6, Unknown=7`. `Flags` are `u8` bits: bridge 1, tunnel 2, informal 4, missing_tags 8, ferry 16. `official_category: u8` is 0 for none and 1–3 for the official network class. `GraphError`: `Io`, `Magic`, `Version`, `Truncated`, `Index`.

`Graph::from_bytes` holds the bytes in `enum Backing { Owned(Vec<u8>), Mmap(memmap2::Mmap) }`; `Graph::open(path)` is the mmap constructor used later.

Create `src/bin/import.rs` and `src/bin/server.rs` as `fn main() {}` stubs so the crate compiles.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --test graph`
Expected: `1 passed`.

- [ ] **Step 5: Add the check recipe and commit**

```make
check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
```

```bash
jj commit -m "feat: graph file format and round-trip loader"
jj bookmark set main -r @-
```

---

### Task 2: Graph sections, accessors, and file-backed open

**Files:**
- Modify: `src/graph.rs`, `src/lib.rs`
- Test: `tests/graph.rs`

**Interfaces:**
- Consumes: Task 1 types.
- Produces: `Graph::open(path: &Path) -> Result<Graph, GraphError>`, `node_elev(u32) -> f32`, `edge_from`, `edge_to`, `edge_attrs(u32) -> &EdgeAttrs` (or a copy), `edge_gain`, `edge_loss`, `edge_geometry(u32) -> impl Iterator<Item=(f64,f64,f32)>`, `anchors() -> &[Anchor]`, `Anchor { pos: LatLon, kind: AnchorKind, weight: f32 }`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn open_from_disk_reads_every_section() {
    let dir = std::env::temp_dir().join("navilau_graph_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("g.bin");
    let mut w = GraphWriter::new(42);
    let a = w.add_node(LatLon { lat: 46.0, lon: 7.0 }, 100.0);
    let b = w.add_node(LatLon { lat: 46.001, lon: 7.0 }, 110.0);
    w.add_anchor(LatLon { lat: 46.0, lon: 7.0 }, AnchorKind::Peak, 1.0);
    w.add_edge(a, b, EdgeAttrs::default(), &[(46.0, 7.0, 100.0), (46.001, 7.0, 110.0)]);
    std::fs::write(&path, w.finish()).unwrap();

    let g = Graph::open(&path).unwrap();
    assert_eq!(g.anchors().len(), 1);
    assert!((g.node_elev(b) - 110.0).abs() < 0.5);
    assert_eq!(g.edge_from(0), a);
    assert_eq!(g.edge_to(0), b);
    let geom: Vec<_> = g.edge_geometry(0).collect();
    assert_eq!(geom.len(), 2);
    assert!((geom[1].2 - 110.0).abs() < 0.5);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --test graph open_from_disk`
Expected: compile error, missing `open`/`anchors`/`AnchorKind`.

- [ ] **Step 3: Implement the accessors and mmap open**

Compute section offsets from counts in one shared `layout()` function used by both writer and reader. Validate `magic`, `version`, and that every section fits inside the byte slice (return `Truncated` otherwise). `Graph::open` mmaps the file, copies the header, and keeps the mmap alive. `anchors()` reads the four SoA arrays into a cached `Vec<Anchor>` once at open (`OnceLock`).

- [ ] **Step 4: Run the tests**

Run: `cargo test --test graph`
Expected: `2 passed`.

- [ ] **Step 5: Commit**

```bash
jj commit -m "feat: graph accessors and mmap-backed open"
```

---

### Task 3: Edge snapping

**Files:**
- Modify: `src/graph.rs`
- Test: `tests/snap.rs`

**Interfaces:**
- Consumes: graph geometry.
- Produces: `Snap { edge: u32, t: f32, pos: LatLon, dist_m: f32 }`, `Graph::snap(&self, p: LatLon) -> Option<Snap>` (150 m limit), `Graph::snap_within(&self, p: LatLon, max_m: f32) -> Option<Snap>`. `t` is the fraction along the stored geometry (0 at `from`, 1 at `to`).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn snaps_to_midpoint_of_straight_edge() {
    let g = straight_edge_graph(); // (0,0)->(0, 0.01) roughly 1111 m
    let snap = g.snap(LatLon { lat: 0.0, lon: 0.005 }).unwrap();
    assert_eq!(snap.edge, 0);
    assert!((snap.t - 0.5).abs() < 0.02);
    assert!(snap.dist_m < 5.0);
}

#[test]
fn returns_none_far_from_any_edge() {
    let g = straight_edge_graph();
    assert!(g.snap(LatLon { lat: 1.0, lon: 1.0 }).is_none());
}

#[test]
fn snap_within_accepts_a_wider_bound() {
    let g = straight_edge_graph();
    let p = LatLon { lat: 0.02, lon: 0.005 }; // ~2.2 km from the edge
    assert!(g.snap(p).is_none());
    assert!(g.snap_within(p, 3_000.0).is_some());
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --test snap`
Expected: FAIL, `snap` not found.

- [ ] **Step 3: Implement snapping**

Build an `rstar::RTree` of edge bounding boxes lazily in a `OnceLock` at first snap. For candidates within a 1 km envelope, project the query point onto each segment, take the closest, compute `t` as accumulated distance / total geometry length. Return `None` when the best horizontal distance exceeds 150 m; a tighter threshold is deliberate, because 500 m in alpine terrain can be a cliff face. Elevation is not part of the snap test in v1.

- [ ] **Step 4: Run the tests**

Run: `cargo test --test snap`
Expected: `2 passed`.

- [ ] **Step 5: Commit**

```bash
jj commit -m "feat: edge snapping with spatial index"
```

---

### Task 4: Cost model and score

**Files:**
- Create: `src/cost.rs`, `tests/cost.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `Graph`, `Surface`, `Flags`.
- Produces: `Profile { weights: Weights, max_sac: u8, exclude: Vec<Exclusion> }` with `Default`; `Weights { slope, surface, tech, official, forest, urban: f32 }`; `Exclusion { lat: f64, lon: f64, radius_m: f32, mode: ExclusionMode }`; `ExclusionMode::{Penalize, Block}`; `Direction::{Forward, Backward}`; `EdgeCost { cost: f32, per_m: f32, breakdown: Breakdown }`; `Breakdown { slope, surface, tech, official, forest, urban: f32 }`; `edge_cost(&Graph, u32, Direction, &Profile) -> Option<EdgeCost>`; `Profile::min_cost_per_m(&self) -> f32` (the admissible A* lower bound); `score(cost_per_km: f32) -> f32`; `route_score(cost: f64, distance_m: f64) -> f32`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn steep_descent_is_penalised_only_forward() {
    let g = edge_graph(gain = 0, loss = 300, len = 1000.0, Surface::Gravel, sac = 1, forest = 0);
    let p = Profile::default();
    let fwd = edge_cost(&g, 0, Direction::Forward, &p).unwrap();
    let rev = edge_cost(&g, 0, Direction::Backward, &p).unwrap();
    assert!((fwd.breakdown.slope - 2.5).abs() < 0.01);
    assert!((rev.breakdown.slope - 1.0).abs() < 0.01);
    assert!(fwd.cost > rev.cost);
}

#[test]
fn asphalt_costs_more_than_gravel() {
    let p = Profile::default();
    assert!(edge_cost(&gravel_graph(), 0, Direction::Forward, &p).unwrap().cost
        < edge_cost(&asphalt_graph(), 0, Direction::Forward, &p).unwrap().cost);
}

#[test]
fn weights_scale_around_neutral() {
    let mut p = Profile::default();
    p.weights.official = 0.0;
    let c = edge_cost(&official_graph(), 0, Direction::Forward, &p).unwrap();
    assert!((c.breakdown.official - 1.0).abs() < 0.01);
}

#[test]
fn marker_penalises_or_blocks() {
    let g = gravel_graph();
    let mut p = Profile::default();
    p.exclude = vec![Exclusion { lat: 46.0, lon: 7.0, radius_m: 100.0, mode: ExclusionMode::Block }];
    assert!(edge_cost(&g, 0, Direction::Forward, &p).is_none());
    p.exclude[0].mode = ExclusionMode::Penalize;
    assert!(edge_cost(&g, 0, Direction::Forward, &p).unwrap().cost > 4.0 * 1000.0);
}

#[test]
fn marker_in_the_middle_of_a_long_segment_still_blocks() {
    let g = long_straight_edge_graph(600.0);
    let mid = point_on_edge(0.5);
    let mut p = Profile::default();
    p.exclude = vec![Exclusion { lat: mid.lat, lon: mid.lon, radius_m: 100.0, mode: ExclusionMode::Block }];
    assert!(edge_cost(&g, 0, Direction::Forward, &p).is_none());
}

#[test]
fn extreme_weight_cannot_make_cost_non_positive() {
    let mut p = Profile::default();
    p.weights.official = 4.0;
    assert!(edge_cost(&official_graph(), 0, Direction::Forward, &p).unwrap().cost > 0.0);
}

#[test]
fn score_is_zero_to_hundred() {
    assert!((score(0.63) - 100.0).abs() < 0.01);
    assert!((score(1.0) - 75.0).abs() < 0.01);
    assert!((score(10.0) - 7.5).abs() < 0.01);
}

#[test]
fn score_divides_by_kilometres_once() {
    // 750 cost over 1000 m is 0.75 per km -> 100, not 0.00075
    assert!((route_score(750.0, 1000.0) - 100.0).abs() < 0.01);
    assert!((route_score(1500.0, 2000.0) - 75.0).abs() < 0.01);
}

#[test]
fn min_cost_per_m_tracks_weight_changes() {
    let mut p = Profile::default();
    let base = p.min_cost_per_m();
    assert!(base > 0.0);
    p.weights.official = 2.0;
    assert!(p.min_cost_per_m() < base);
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --test cost`
Expected: compile error, `cost` module missing.

- [ ] **Step 3: Implement the factor table**

Apply the design §8 defaults exactly. `slope` uses `loss/len` forward, `gain/len` backward. Surface multipliers: asphalt 2.0, paved 1.5, compacted 1.1, gravel 1.0, dirt 0.95, grass 1.1, rock 1.2, unknown 1.4. Technicality: sac 0 missing → 1.3, T1 1.0, T2 1.05, T3 1.15, T4 1.4, T5/T6 filtered out when `max_sac < 5`. Official 0.7 when `official_category > 0`, else 1.0. Forest `1.0 - 0.1 * forest_pct/100`. Urban: read the `urban` byte (0 none, 1 within 200 m, 2 inside) and apply 1.0, 1.3, 2.0.

Weight application: `factor' = (1 + (factor - 1) * w).max(0.05)`. The `slope` weight scales the whole slope factor, uphill and downhill alike; label it "slope sensitivity" in the UI. Exclusion: compute the distance from the marker point to each geometry **segment**, not only to vertices; if it is within the radius, `Block` → `None`, `Penalize` → `cost *= 5`. `score = 100.0 * (0.75 / cost_per_km).clamp(0.0, 1.0)` with `cost_per_km = cost / (distance_m / 1000.0)`; `route_score` is that whole formula.

`Profile::min_cost_per_m` is the product of the smallest weighted value of each factor across the enum values and slope classes, computed once per request, with the same `0.05` floor applied per factor. It is the admissible A* lower bound; it must never exceed any edge's actual per-metre cost.

- [ ] **Step 4: Run the tests**

Run: `cargo test --test cost`
Expected: `9 passed`.

- [ ] **Step 5: Commit**

```bash
jj commit -m "feat: cost model, slider weights, and score"
```

---

### Task 5: A* routing with repetition penalty

**Files:**
- Create: `src/route.rs`, `tests/route.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `Graph`, `Profile`, `Snap`, `edge_cost`.
- Produces: `RouteStep { edge: u32, from_t: f32, to_t: f32 }`; `Route { steps: Vec<RouteStep>, distance_m: f64, climb_m: f64, descent_m: f64, cost: f64 }`; `Visited` with `new(edge_count: usize)`, `insert(u32)`, `contains(u32)`; `route(&Graph, &Profile, Snap, Snap, &Visited) -> Option<Route>`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn picks_the_cheaper_of_two_paths() {
    // direct edge costs 2.0/m; detour costs 1.0/m and is 200 m longer
    let g = two_path_graph();
    let start = g.snap(point_on_direct(0.1)).unwrap();
    let end = g.snap(point_on_direct(0.9)).unwrap();
    let r = route(&g, &Profile::default(), start, end, &Visited::new(g.edge_count())).unwrap();
    assert!(r.steps.iter().any(|s| s.edge == DETOUR_EDGE));
}

#[test]
fn visited_penalty_flips_the_choice() {
    let g = two_path_graph();
    let start = g.snap(point_on_direct(0.1)).unwrap();
    let end = g.snap(point_on_direct(0.9)).unwrap();
    let mut visited = Visited::new(g.edge_count());
    visited.insert(DETOUR_EDGE);
    let r = route(&g, &Profile::default(), start, end, &visited).unwrap();
    assert!(r.steps.iter().any(|s| s.edge == DIRECT_EDGE));
}

#[test]
fn disconnected_returns_none() {
    let g = island_graph();
    assert!(route(&g, &Profile::default(), a, b, &Visited::new(g.edge_count())).is_none());
}

#[test]
fn same_edge_waypoints_connect_directly() {
    let g = straight_edge_graph_of_length(1000.0);
    let start = g.snap(point_at(0.2)).unwrap();
    let end = g.snap(point_at(0.8)).unwrap();
    let r = route(&g, &Profile::default(), start, end, &Visited::new(g.edge_count())).unwrap();
    assert!((r.distance_m - 600.0).abs() < 1.0);
    assert_eq!(r.steps.len(), 1);
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --test route`
Expected: compile error, `route` missing.

- [ ] **Step 3: Implement forward A***

Forward A* only. Bidirectional search with asymmetric slope costs and a path-dependent visited penalty is easy to make subtly wrong, and one direction is fast enough for 10–30 km queries. Use a binary heap keyed by `f64` bits. Heuristic = haversine meters × `profile.min_cost_per_m()`; that bound comes from the profile, so it stays admissible when a slider makes a factor below 0.63. If in doubt, use 0 (Dijkstra).

Handle mid-edge starts and ends with a virtual goal. From the start snap, seed both endpoints of its edge with the partial cost toward that endpoint. Do not stop on the first endpoint of the end edge that is popped: relax a virtual `GOAL` from both endpoints with the partial cost from that endpoint to `end.t`, and keep searching until the smallest f in the queue is no smaller than the best `GOAL` cost. If start and end snap to the same edge, the direct partial is one candidate, not an automatic answer; compare it with the search result, which may leave by one endpoint and return by the other. Record every traversed edge as `RouteStep { edge, from_t, to_t }`, including both partials. Accumulate distance, climb and descent over the sliced steps: forward uses `gain`, backward uses `loss`.

`Visited` is a dense bitset indexed by edge id (a `Vec<u64>`), not a `HashSet`, because it is consulted in the hot loop. Visited edges cost ×5.

- [ ] **Step 4: Run the tests**

Run: `cargo test --test route`
Expected: `4 passed`.

- [ ] **Step 5: Commit**

```bash
jj commit -m "feat: A* routing with repetition penalty"
```

---

### Task 6: Import the walkable OSM graph

**Files:**
- Create: `src/import/mod.rs`, `src/import/osm.rs`, `tests/fixtures/tiny.osm`, `tests/import_osm.rs`
- Modify: `src/bin/import.rs`

**Interfaces:**
- Consumes: `GraphWriter`.
- Produces: `import_osm(osm: &Path, osm_date: i64) -> Result<(GraphWriter, ImportStats, OsmExtras), ImportError>`; `ImportStats { nodes: u64, edges: u64, skipped_ways: u64 }`; `OsmExtras { forest: Vec<geo::Polygon<f64>>, residential: Vec<geo::Polygon<f64>>, peaks: Vec<(LatLon, f32)> }`.

- [ ] **Step 1: Write the failing test and fixture**

`tests/fixtures/tiny.osm` (three nodes on a path, one junction, one track, one building node):

```xml
<?xml version="1.0"?>
<osm version="0.6">
  <node id="1" lat="46.9000" lon="7.4000"/>
  <node id="2" lat="46.9000" lon="7.4010"/>
  <node id="3" lat="46.9000" lon="7.4020"/>
  <node id="4" lat="46.9000" lon="7.4030"/>
  <node id="5" lat="46.9010" lon="7.4000"/>
  <way id="10">
    <nd ref="1"/><nd ref="2"/><nd ref="3"/>
    <tag k="highway" v="path"/>
  </way>
  <way id="11">
    <nd ref="3"/><nd ref="4"/>
    <tag k="highway" v="track"/>
  </way>
  <way id="12">
    <nd ref="5"/><nd ref="1"/>
    <tag k="building" v="yes"/>
  </way>
</osm>
```

`tests/fixtures/loop.osm` (closed ring, no junction):

```xml
<?xml version="1.0"?>
<osm version="0.6">
  <node id="1" lat="46.9000" lon="7.4000"/>
  <node id="2" lat="46.9000" lon="7.4010"/>
  <node id="3" lat="46.9010" lon="7.4010"/>
  <node id="4" lat="46.9010" lon="7.4000"/>
  <way id="20">
    <nd ref="1"/><nd ref="2"/><nd ref="3"/><nd ref="4"/><nd ref="1"/>
    <tag k="highway" v="path"/>
  </way>
</osm>
```

```rust
#[test]
fn splits_at_junctions_and_skips_non_walkable() {
    let (w, stats, _extras) = import_osm(Path::new("tests/fixtures/tiny.osm"), 0).unwrap();
    assert_eq!(stats.edges, 2);
    let g = Graph::from_bytes(w.finish()).unwrap();
    assert_eq!(g.edge_count(), 2);
    assert_eq!(g.edge_geometry(0).count(), 3);
}

#[test]
fn closed_way_does_not_create_a_self_loop() {
    let (w, _stats, _extras) = import_osm(Path::new("tests/fixtures/loop.osm"), 0).unwrap();
    let g = Graph::from_bytes(w.finish()).unwrap();
    assert!(g.edge_count() >= 2);
    for e in 0..g.edge_count() {
        assert_ne!(g.edge_from(e), g.edge_to(e));
    }
}

#[test]
fn maps_osm_tags_and_penalises_unknown() {
    let (w, _stats, _extras) = import_osm(Path::new("tests/fixtures/tags.osm"), 0).unwrap();
    let g = Graph::from_bytes(w.finish()).unwrap();
    assert_eq!(g.edge_attrs(0).surface, Surface::Unknown);
    assert_eq!(g.edge_attrs(0).sac, 0);
    assert_ne!(g.edge_attrs(0).flags & FLAG_MISSING_TAGS, 0);
}
```

`tests/fixtures/tags.osm` is two nodes and one path with `surface=unobtainium`, `sac_scale=foobar`.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --test import_osm`
Expected: compile error, `import_osm` missing.

- [ ] **Step 3: Implement the three-pass importer**

`--osm` accepts `.osm` XML and `.pbf`: XML is parsed with `roxmltree` (human-readable fixtures), PBF with `osmpbf` (the real import), and both produce the same `RawWay`/node structures. Pass 1: collect ids of ways whose `highway` is one of `path, footway, track, bridleway, steps, service, residential, unclassified, tertiary, secondary, primary` and whose `access` does not forbid foot. Pass 2: collect coordinates for the referenced nodes. Pass 3: split each way at shared nodes and at way endpoints, emit one edge per segment with the full intermediate geometry, and skip ways with fewer than two usable nodes. `steps` gets flag `missing_tags` off but sac 0; `building` ways are ignored.

Closed ways and self-intersections: drop a trailing duplicate node, then split at every node repeated inside the way and at junction nodes. If a resulting segment starts and ends at the same node, split it at its middle geometry vertex with a synthetic node so A* never sees a self-loop.

Apply the way's tags when the edge is created, before any later splitting: `surface` mapping per the design, `sac_scale` 1–6 (unknown or missing → 0 plus `missing_tags`), bridge/tunnel/informal flags. Split edges inherit these attributes, which is why tag application lives here and not in Task 8.

Pass 1 also collects land cover and peaks from the same PBF, so no extra download is needed: closed `landuse=forest|meadow` / `natural=wood` ways go into `OsmExtras.forest`; closed `landuse=residential` ways and `place=city|town|village|hamlet` nodes into `OsmExtras.residential`, buffering each place node into a circle so the type stays `Polygon` (300 m for hamlet/village, 1 km for town, 2 km for city; not individual buildings — Switzerland has millions and they would dominate memory); `natural=peak` nodes into `OsmExtras.peaks` with their `ele` (or 1.0 when missing).

Also add `just data`: download the Geofabrik CH PBF to `data/raw/ch-latest.osm.pbf`; download the swisstopo DEM and convert it to `data/raw/dhm25.asc` keeping the native Swiss grid — `gdal_translate -of AAIGrid` without `-t_srs`. Verify the current product URL at implementation: DHM25 is legacy and sometimes ships as GeoTIFF or tiles; swissALTI3D 2 m is the fallback. Download the Wanderwege GPKG, then `ogr2ogr -f FlatGeobuf -t_srs EPSG:4326 data/raw/official.fgb <gpkg>`. GDAL is a dev tool only, never a runtime dependency.

Wire `src/bin/import.rs` in this task: clap args `--osm <path> --out <path>`, env `REGION=demo|ch` selecting defaults under `data/raw/`, and a body that calls `import_osm` then `write_to`. Bare `navilau-import` with no args derives every path from `REGION` and `DATA_DIR` (output `$DATA_DIR/graph.bin`) and exits 0 immediately when that output already exists — before reading the PBF or DEM — so the compose import service is safely re-runnable. Tasks 7 and 8 add `--dem` and `--official` and extend this same body, which stays the single orchestration point.

- [ ] **Step 4: Run the test**

Run: `cargo test --test import_osm`
Expected: `3 passed`.

- [ ] **Step 5: Commit**

```bash
jj commit -m "feat: import walkable OSM ways into the graph"
```

---

### Task 7: DEM parsing, slope, and geometry Z

**Files:**
- Create: `src/import/dem.rs`, `src/import/ch1903.rs`, `tests/fixtures/tiny.asc`, `tests/import_dem.rs`
- Modify: `src/import/mod.rs`, `src/bin/import.rs`

**Interfaces:**
- Consumes: graph edges from Task 6.
- Produces: `wgs84_to_lv95(lat, lon) -> (f64, f64)`, `lv95_to_wgs84(e, n) -> (f64, f64)` and `wgs84_to_lv03(lat, lon) -> (f64, f64)` in `ch1903.rs`; `Dem::from_asc(path) -> Result<Dem, ImportError>`, `Dem::sample(lat, lon) -> f32`, `Dem::sample_grid(e, n) -> f32`; `apply_dem(&mut GraphWriter, &Dem)`; `GraphWriter::split_edges_at_extrema(&mut self, min_reversal_m: f32)`.

- [ ] **Step 1: Write the failing test and fixture**

`tests/fixtures/tiny.asc` in its native Swiss grid (LV95, 10 km cells, north row first):

```
ncols 3
nrows 3
xllcorner 2590000
yllcorner 1190000
cellsize 10000
NODATA_value -9999
100 110 120
200 210 220
300 310 320
```

```rust
#[test]
fn projects_the_bern_reference_point() {
    let (e, n) = wgs84_to_lv95(46.951082877, 7.438632495);
    assert!((e - 2_600_000.0).abs() < 2.0);
    assert!((n - 1_200_000.0).abs() < 2.0);
}

#[test]
fn projection_round_trips_away_from_the_origin() {
    let (e, n) = wgs84_to_lv95(46.90, 7.40);
    let (lat, lon) = lv95_to_wgs84(e, n);
    assert!((lat - 46.90).abs() < 1e-6);
    assert!((lon - 7.40).abs() < 1e-6);
}

#[test]
fn bilinear_samples_the_native_grid() {
    let dem = Dem::from_asc(Path::new("tests/fixtures/tiny.asc")).unwrap();
    assert!((dem.sample_grid(2_600_000.0, 1_200_000.0) - 210.0).abs() < 1.0);
}

#[test]
fn apply_dem_sets_z_and_slope() {
    let dem = Dem::from_asc(Path::new("tests/fixtures/tiny.asc")).unwrap();
    let mut w = GraphWriter::new(0);
    let a = w.add_node(LatLon { lat: 46.95, lon: 7.44 }, 0.0);
    let b = w.add_node(LatLon { lat: 46.99, lon: 7.44 }, 0.0);
    w.add_edge(a, b, EdgeAttrs::default(), &[(46.95, 7.44, 0.0), (46.99, 7.44, 0.0)]);
    apply_dem(&mut w, &dem);
    let g = Graph::from_bytes(w.finish()).unwrap();
    assert!(g.edge_attrs(0).gain + g.edge_attrs(0).loss > 50);
    assert!(g.edge_geometry(0).last().unwrap().2 > 0.0);
}

#[test]
fn splits_an_edge_at_a_local_peak() {
    let mut w = GraphWriter::new(0);
    let a = w.add_node(LatLon { lat: 46.0, lon: 7.0 }, 100.0);
    let b = w.add_node(LatLon { lat: 46.0, lon: 7.01 }, 100.0);
    w.add_edge(a, b, EdgeAttrs::default(), &[
        (46.0, 7.0, 100.0), (46.0, 7.005, 200.0), (46.0, 7.01, 100.0),
    ]);
    w.split_edges_at_extrema(15.0);
    let g = Graph::from_bytes(w.finish()).unwrap();
    assert_eq!(g.edge_count(), 2);
    assert!(g.edge_attrs(0).gain > 90 && g.edge_attrs(0).loss == 0);
    assert!(g.edge_attrs(1).gain == 0 && g.edge_attrs(1).loss > 90);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --test import_dem`
Expected: compile error, `Dem` missing.

- [ ] **Step 3: Implement the projection, ASC parser, and DEM application**

Keep the DEM in its native Swiss grid; do not reproject it. `ch1903.rs` holds the official swisstopo approximate WGS84↔LV95/LV03 polynomials (sub-metre, no PROJ dependency); the Bern reference point pins it. `Dem::from_asc` parses the ESRI header and detects LV03 vs LV95 from the magnitude of `xllcorner`, then `sample_grid` is bilinear with edge clamping and nearest-valid fallback for `NODATA`.

`apply_dem` converts each geometry vertex to grid coordinates, samples Z, sets node elevation from the first vertex, and computes gain/loss with a 2 m vertical deadband (count a rise or fall only once it exceeds 2 m from the current turning point).

`split_edges_at_extrema` then walks each edge's Z series, cuts at local extrema where the reversal exceeds `min_reversal_m` (use 15 m), creates a node at each cut vertex, and emits sub-edges with copied attributes. Recompute gain/loss per sub-edge over the deadband walk, so `loss/len` describes one descent instead of an averaged climb-and-descend. The 15 m floor matters: splitting on a 3.5 m bump would create a 15 m sub-edge with a 23% grade and trigger the steep-descent penalty for a negligible dip.

- [ ] **Step 4: Run the tests**

Run: `cargo test --test import_dem`
Expected: `5 passed`.

- [ ] **Step 5: Commit**

```bash
jj commit -m "feat: native-grid DEM, projection, slope, extrema splitting"
```

---

### Task 8: Attributes and anchors

**Files:**
- Create: `src/import/attrs.rs`, `tests/import_attrs.rs`
- Modify: `src/import/mod.rs`, `src/bin/import.rs`

**Interfaces:**
- Consumes: `OsmExtras` from Task 6, official lines read from FlatGeobuf.
- Produces: `OfficialLine { geom: geo::LineString<f64>, category: u8 }`; `read_official_fgb(path) -> Result<Vec<OfficialLine>, ImportError>`; `apply_official(&mut GraphWriter, lines: &[OfficialLine], tolerance_m: f32)`; `apply_landcover(&mut GraphWriter, extras: &OsmExtras)`; `collect_anchors(&mut GraphWriter, peaks: &[(LatLon, f32)], official: &[geo::LineString<f64>])`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn marks_edges_near_an_official_line_with_its_category() {
    let mut w = official_fixture_graph();
    let lines = vec![OfficialLine {
        geom: geo::LineString::from(vec![(7.0, 46.0), (7.01, 46.0)]),
        category: 1,
    }];
    apply_official(&mut w, &lines, 20.0);
    let g = Graph::from_bytes(w.finish()).unwrap();
    assert_eq!(g.edge_attrs(0).official_category, 1);
}

#[test]
fn forest_urban_and_anchors_from_polygons() {
    let mut w = forest_fixture_graph();
    let extras = OsmExtras {
        forest: vec![forest_polygon()],
        residential: vec![residential_polygon_near_the_edge()],
        peaks: vec![(LatLon { lat: 46.9, lon: 7.4 }, 1200.0)],
    };
    apply_landcover(&mut w, &extras);
    collect_anchors(&mut w, &extras.peaks, &[]);
    let g = Graph::from_bytes(w.finish()).unwrap();
    assert!(g.edge_attrs(0).forest_pct > 50);
    assert!(g.edge_attrs(0).urban > 0);
    assert_eq!(g.anchors()[0].kind, AnchorKind::Peak);
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --test import_attrs`
Expected: compile error, functions missing.

- [ ] **Step 3: Implement the attribute passes**

Tags are already on the edges from Task 6; this task adds the joined data.

Official trails: `read_official_fgb` streams the WGS84 FlatGeobuf converted by `just data`. Map the Wanderwege category attribute (`WANDERWEG`, `BERGWANDERWEG`, `ALPINWANDERWEG`; confirm the exact field name in the GPKG) to 1, 2, 3; unknown → 1. Index the lines in an R-tree; an edge is official when at least 50 % of its sampled vertices lie within `tolerance_m` of a line, and it takes that line's category.

Land cover comes from the OSM polygons already in `OsmExtras`, so no extra download. Index polygons in an `rstar::RTree` of bounding boxes, test only candidates, and use `geo::PreparedGeometry` for repeated containment; without this, CH-wide point-in-polygon over every vertex takes hours. Forest fraction is the share of sampled vertices inside forest polygons. Urban is 2 inside a residential polygon and 1 within 200 m of one.

Anchors: peaks passed in, official-route midpoints, and forest centroids; weight is the peak's `ele` when present, else 1.0. Extend `src/bin/import.rs` with `--dem` and `--official`, applying the passes in order: OSM tags → DEM/split → official → land cover → anchors.

- [ ] **Step 4: Run the tests**

Run: `cargo test --test import_attrs`
Expected: `2 passed`.

- [ ] **Step 5: Commit**

```bash
jj commit -m "feat: surface, technicality, official trail, land cover, anchors"
```

---

### Task 9: Generator — anchors and seeds

**Files:**
- Create: `src/generate.rs`, `tests/generate.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `Graph`, `Profile`, `route`.
- Produces: `GenerateRequest { start: LatLon, distance_m: f64, climb_m: Option<f64>, profile: Profile, count: usize }`; `GeneratedRoute { route: Route, waypoints: Vec<LatLon>, score: f32, breakdown: Breakdown, official_share: f32, distance_met: bool }`; `generate(&Graph, &GenerateRequest) -> Vec<GeneratedRoute>`; internal `seed_pairs`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn returns_three_legs_and_respects_the_band() {
    let g = ring_fixture_graph(); // 3 km ring with anchors around it
    let req = GenerateRequest { start: g.node(0), distance_m: 3000.0, climb_m: None, profile: Profile::default(), count: 3 };
    let routes = generate(&g, &req);
    assert!(!routes.is_empty());
    assert!(routes.iter().all(|r| r.route.distance_m > 1500.0));
    assert!(routes[0].distance_met);
}

#[test]
fn flat_fixture_still_returns_something() {
    let routes = generate(&flat_graph(), &req(5000.0));
    assert!(!routes.is_empty());
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --test generate`
Expected: compile error, `generate` missing.

- [ ] **Step 3: Implement anchors and seeds**

Anchor radius `R = (distance_m / 3.0).clamp(2000.0, 12000.0)`. First drop anchors inside `Block` exclusion zones so they never consume seed slots. Filter the rest to `R`, rank by `weight / (1 + distance_from_start_m / 1000.0)`, keep the top 16 spread across headings (at most 4 per 90° sector). Seed pairs need angular separation ≥ 60°. Build `start → A → B → start` with `route` and a `Visited::new(g.edge_count())`, dropping edges within 500 m of the start before inserting them so an access stem can be reused. Heading fallback when fewer than 2 anchors exist: 8 headings × a single ring point at `R`, giving `start → P → start` with the repetition penalty forcing a loop. Skip any seed with a `None` leg, and store the seed's via points in `GeneratedRoute.waypoints` (start, anchors, start).

- [ ] **Step 4: Run the tests**

Run: `cargo test --test generate`
Expected: `2 passed`.

- [ ] **Step 5: Commit**

```bash
jj commit -m "feat: anchor selection and loop seeds"
```

---

### Task 10: Generator — length repair, refinement, dedup, target miss

**Files:**
- Modify: `src/generate.rs`
- Test: `tests/generate.rs`

**Interfaces:**
- Consumes: Task 9 internals.
- Produces: `repair_length`, `refine`, `dedup`, and the final `generate` behaviour: sorted by distance error, `distance_met` computed as `|d − d*| / d* <= 0.10`, closest returned when none is in band.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn short_seed_is_repaired_into_the_band() {
    let routes = generate(&ring_fixture_graph(), &req(6000.0));
    assert!(routes.iter().any(|r| r.distance_met));
}

#[test]
fn impossible_target_returns_closest_with_flag() {
    let routes = generate(&tiny_fixture_graph(), &req(40_000.0));
    assert!(!routes.is_empty());
    assert!(routes.iter().all(|r| !r.distance_met));
}

#[test]
fn overlapping_routes_are_deduplicated() {
    let routes = generate(&ring_fixture_graph(), &req(3000.0));
    let edges: Vec<Vec<u32>> = routes
        .iter()
        .map(|r| r.route.steps.iter().map(|s| s.edge).collect())
        .collect();
    for i in 0..edges.len() {
        for j in (i + 1)..edges.len() {
            let a: HashSet<_> = edges[i].iter().collect();
            let b: HashSet<_> = edges[j].iter().collect();
            let shared = a.intersection(&b).count() as f64;
            let shorter = edges[i].len().min(edges[j].len()) as f64;
            assert!(shared / shorter <= 0.5);
        }
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --test generate`
Expected: FAIL on the repair and dedup assertions.

- [ ] **Step 3: Implement repair, refinement, and dedup**

Length repair: while the seed is more than 10 % short and fewer than 3 anchors were inserted, insert the next-ranked unused anchor between the closest pair of consecutive via points and reroute. Refinement: two rounds; for each via point try offsets of 100 m, 300 m, 1 km along the radial and perpendicular directions, snapping candidates with `snap_within(candidate, 500.0)` so an alpine offset does not become a no-op at the 150 m user threshold; reroute the two adjacent legs; accept when `J = |D−D*|/D* + 0.25·climb_term − 0.2·score/100` decreases. Climb term is 0 when `climb_m` is `None`, otherwise `|C−C*|/C*`. Dedup: overlap = shared-edge distance / shorter route distance; above 0.5 drop the weaker. After repair and refinement, set `waypoints` to the final via-point list that produced the route, so the client can render drag handles. Sort by `|d − d*|`; cap at `count`.

- [ ] **Step 4: Run the tests**

Run: `cargo test --test generate`
Expected: `5 passed`.

- [ ] **Step 5: Commit**

```bash
jj commit -m "feat: length repair, refinement, dedup, closest-match fallback"
```

---

### Task 11: API — health and route

**Files:**
- Create: `src/server.rs`, `src/bin/server.rs`, `tests/api.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `Graph::open`, `snap`, `route`, `cost`.
- Produces: `AppState { graph: Arc<Graph> }`, `app(state: AppState) -> axum::Router`; `POST /api/route` with `{ waypoints: [{lat,lon}], profile }` → route JSON; `GET /api/health` → `{ status: "ok", graph_date }`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn health_reports_graph_date() {
    let app = app(AppState { graph: Arc::new(fixture_graph()) });
    let res = app.oneshot(Request::get("/api/health").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_json(res).await;
    assert!(body["graph_date"].as_i64().is_some());
}

#[tokio::test]
async fn route_between_two_points() {
    let body = json!({"waypoints":[{"lat":46.9,"lon":7.4},{"lat":46.9,"lon":7.402}],"profile":{}});
    let res = post_json(app(), "/api/route", body).await;
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert!(json["distance_m"].as_f64().unwrap() > 0.0);
    assert_eq!(json["waypoints"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn off_network_start_is_422() {
    let body = json!({"waypoints":[{"lat":47.5,"lon":8.5},{"lat":46.9,"lon":7.402}],"profile":{}});
    let res = post_json(app(), "/api/route", body).await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body_json(res).await["error"].as_str().unwrap().contains("no trail"));
}

#[tokio::test]
async fn route_between_two_points_on_the_same_edge_is_direct() {
    let body = json!({"waypoints":[{"lat":46.9,"lon":7.4002},{"lat":46.9,"lon":7.4008}],"profile":{}});
    let res = post_json(app(), "/api/route", body).await;
    assert_eq!(res.status(), StatusCode::OK);
    let d = body_json(res).await["distance_m"].as_f64().unwrap();
    assert!(d < 120.0, "same-edge route should not run to the endpoint and back: {d}");
}

#[tokio::test]
async fn route_rejects_single_waypoint() {
    let body = json!({"waypoints":[{"lat":46.9,"lon":7.4}],"profile":{}});
    assert_eq!(post_json(app(), "/api/route", body).await.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn blocked_start_is_422() {
    let body = json!({
        "waypoints":[{"lat":46.9,"lon":7.4},{"lat":46.9,"lon":7.402}],
        "profile":{"exclude":[{"lat":46.9,"lon":7.4,"radius_m":500,"mode":"block"}]}
    });
    assert_eq!(post_json(app(), "/api/route", body).await.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --test api`
Expected: compile error, `server` missing.

- [ ] **Step 3: Implement the handlers**

Reject fewer than 2 or more than 20 waypoints with `422 {"error":"need 2-20 waypoints"}`. Reject any profile weight outside `0..=3` with `422 {"error":"weight out of range"}`. Snap every waypoint; any `None` → `422 {"error":"no trail near waypoint N"}`. Route between consecutive waypoints with an empty `visited` set, because a manual edit may legitimately be an out-and-back; any `None` → `422 {"error":"no path between waypoints"}`. Echo the input waypoints in the response. Serialise the geometry by slicing each `RouteStep` between `from_t` and `to_t` along the edge's geometry, reversing the sub-array when `from_t > to_t`, so a reverse-traversed step does not zigzag and a mid-edge start is not rounded to the edge endpoint. Emit a GeoJSON `LineString` with `[lon, lat, z]` coordinates. `Profile` deserialises with defaults for every field, so `{}` is valid. Serve the static client from `web/dist` at `/` with a fallback to `index.html`. `src/bin/server.rs` reads `DATA_DIR` (default `./data`), opens `graph.bin`, builds `AppState`, binds `0.0.0.0:8080`, and exits with a readable error when the graph is missing.

- [ ] **Step 4: Run the tests**

Run: `cargo test --test api`
Expected: `6 passed`.

- [ ] **Step 5: Commit**

```bash
jj commit -m "feat: axum server with health and route endpoints"
```

---

### Task 12: API — generate and cache

**Files:**
- Modify: `src/server.rs`, `tests/api.rs`
- Test: `tests/api.rs`

**Interfaces:**
- Consumes: Task 10 `generate`.
- Produces: `POST /api/generate`; response `{ routes: [{geometry, waypoints, distance_m, climb_m, descent_m, score, breakdown, official_share, distance_met}] }`; `X-Cache: hit|miss` debug header; `Cache-Control: no-store`; in-memory `LruCache<[u8; 32], String>` with 256 entries.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn generate_returns_routes_and_marks_the_cache() {
    let body = json!({"start":{"lat":46.9,"lon":7.4},"distance_m":3000,"profile":{},"count":3});
    let first = post_json(app(), "/api/generate", body.clone()).await;
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(first.headers()["x-cache"], "miss");
    let second = post_json(app(), "/api/generate", body).await;
    assert_eq!(second.headers()["x-cache"], "hit");
    let json = body_json(second).await;
    assert!(json["routes"].as_array().unwrap().len() <= 3);
    assert!(json["routes"][0]["distance_met"].as_bool().is_some());
    assert!(json["routes"][0]["waypoints"].as_array().unwrap().len() >= 3);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --test api generate_returns`
Expected: 404.

- [ ] **Step 3: Implement the endpoint**

Key = the 32-byte BLAKE3 digest of `graph.osm_date()` plus a canonical byte encoding of the request with latitude/longitude rounded to 1e-6 and distances to 1 m. Use `LruCache<[u8; 32], String>`; do not truncate to `u64` and do not use `DefaultHasher` or `serde_json::Value`. On miss, call `generate`, serialise once, insert, return with `X-Cache: miss`; on hit, return the stored body with `X-Cache: hit`. Always send `Cache-Control: no-store` on `generate`; `X-Cache` is a debug header only. Validate `distance_m` in `500..=80_000` and `count` in `1..=12`, else `422`.

- [ ] **Step 4: Run the tests**

Run: `cargo test --test api`
Expected: `7 passed`.

- [ ] **Step 5: Commit**

```bash
jj commit -m "feat: generate endpoint with LRU response cache"
```

---

### Task 13: Frontend — shell, map, form, results, GPX

**Files:**
- Create: `web/package.json`, `web/vite.config.ts`, `web/index.html`, `web/src/main.tsx`, `web/src/App.tsx`, `web/src/api.ts`, `web/src/map.ts`, `web/src/components/ControlPanel.tsx`, `web/src/components/RouteList.tsx`, `web/src/components/RouteCard.tsx`, `web/src/components/ElevationProfile.tsx`, `web/src/gpx.ts`, `web/src/url.ts`
- Modify: `justfile`

**Interfaces:**
- Consumes: `/api/generate`, `/api/route`.
- Produces: a working single-page UI; `generate(req)`, `route(waypoints, profile)` in `api.ts`.

- [ ] **Step 1: Scaffold and write the failing build**

`web/package.json` with `solid-js`, `maplibre-gl`, `vite`, `vite-plugin-solid`, `typescript`. `vite.config.ts` proxies `/api` to `http://localhost:8080`.

Run: `cd web && npm install && npm run build`
Expected: FAIL, `src/App.tsx` not found.

- [ ] **Step 2: Implement the map and the shell**

MapLibre style with two raster sources:

```ts
const BASE = {
  version: 8,
  sources: {
    swisstopo: {
      type: "raster",
      tiles: ["https://wmts.geo.admin.ch/1.0.0/ch.swisstopo.pixelkarte-farbe/default/current/3857/{z}/{x}/{y}.jpeg"],
      tileSize: 256,
      maxzoom: 18,
      attribution: "©swisstopo",
    },
  },
  layers: [{ id: "base", type: "raster", source: "swisstopo" }],
};
```

The satellite toggle swaps the tile URL to `ch.swisstopo.swissimage` and sets `maxzoom: 20`. The relief layer is `ch.swisstopo.swisssurface3d-reliefschattierung-monodirektional` (PNG, `maxzoom: 18`; verify whether its WMTS matrix path needs a timestamp instead of `current`). MapLibre then overzooms instead of requesting 404 tiles. The panel holds start (map click or last-used from localStorage), distance (default 15 km), and a hilliness slider (default 60 m/km → `climb_m`). Advanced drawer holds the six weight sliders (label `slope` as "Slope sensitivity"), `max_sac`, and click-to-add exclusion markers. Results are cards in a scrollable rail: distance, climb, score, official share, "closest match" badge when `distance_met` is false. Clicking a card fits the map to the route and shows the elevation profile from the geometry Z values (~200 sampled points), reversing the slice when the step's `from_t > to_t`. GPX builder emits `<trkpt lat lon><ele>` per vertex.

- [ ] **Step 3: Run the build**

Run: `cd web && npm run build`
Expected: `dist/index.html` exists, exit 0.

- [ ] **Step 4: Verify in the browser**

Run: `just dev` (server + `npm run dev`), open `http://localhost:5173`, generate 15 km from a point near Zurich, click a card, download the GPX, confirm the file opens in a GPX viewer and the elevation profile matches the summary.
Expected: routes visible, stats coherent, no console errors.

- [ ] **Step 5: Commit**

```bash
jj commit -m "feat: Solid + MapLibre client with generation and GPX"
```

---

### Task 14: Frontend — drag editing

**Files:**
- Create: `web/src/components/DragHandles.tsx`
- Modify: `web/src/App.tsx`, `web/src/components/RouteCard.tsx`, `web/src/url.ts`

**Interfaces:**
- Consumes: `/api/route`.
- Produces: edit mode with draggable via points; on release the selected route is re-requested and its stats/score replaced.

- [ ] **Step 1: Implement the edit mode**

When a route is selected, an "Edit" button inserts draggable markers at `route.waypoints`, which both API endpoints return. The start and end markers stay fixed; interior anchors move. Dragging a segment midpoint inserts a new interior waypoint at that position, so a 15 km loop can be shaped beyond its two anchors (the API caps the list at 20). `dragend` rebuilds the waypoint list, calls `route(waypoints, profile)` with a 150 ms debounce, and replaces the route geometry and card values. Escape cancels and restores the previous route. After an edit, store the waypoints in the URL as `w=lat,lon;lat,lon;...` together with the serialized profile as `p=<json>`, and drop both when the user regenerates. On load, if `w` is present, call `route(waypoints, profileFromUrl)` instead of `generate`; otherwise the shared link would discard the edit or replay it with default weights.

- [ ] **Step 2: Verify manually**

Run: `just dev`, select a route, drag a via point 300 m sideways.
Expected: the line snaps to the network along a new path within ~1 s, distance/climb/score update, the elevation profile redraws, no console errors.

- [ ] **Step 3: Verify the debug case**

Drag a point into a steep descent and confirm the score drops while the distance stays in band — this is the cost-debugging workflow from the brief.
Expected: score changes in the expected direction.

- [ ] **Step 4: Commit**

```bash
jj commit -m "feat: drag-to-edit with live route rescoring"
```

---

### Task 15: Docker image and compose stack

**Files:**
- Create: `Dockerfile`, `docker-compose.yml`, `.dockerignore`
- Modify: `justfile`, `README.md`

**Interfaces:**
- Consumes: both binaries and `web/dist`.
- Produces: an image running `navilau-server` on 8080 as non-root, and a compose stack that imports a demo graph if absent and then serves.

- [ ] **Step 1: Write the Dockerfile**

Three stages: `node:26-alpine` builds `web/dist`; `rust:1.100` builds `--release` with the musl target; final `gcr.io/distroless/static-debian12:nonroot` copies `/app/navilau-server`, `/app/navilau-import`, `/app/web`, and sets `ENTRYPOINT ["/app/navilau-server"]`, `EXPOSE 8080`, `VOLUME ["/data"]`. Copy `tests/fixtures/` into the image so `REGION=demo` can import without a download. `osmpbf` pulls `flate2` with the pure-Rust `miniz_oxide` backend and `prost`, so the musl build needs no libz-sys or C toolchain; do not enable C-linked compression features. `flatgeobuf`, `geo`, `rstar` and `blake3` are pure Rust.

- [ ] **Step 2: Write the compose file**

`import` service: same image, `entrypoint: ["/app/navilau-import"]`, mounts `./data:/data`, env `REGION=${REGION:-demo}` and `DATA_DIR=/data`, and exits 0 immediately when `/data/graph.bin` exists. Bare import derives `--osm/--dem/--official/--out` from those env vars, so no args are needed. `REGION=demo` builds from the committed `tests/fixtures/tiny.osm` and `tiny.asc` so `docker compose up` needs no download; `REGION=ch` expects `just data` to have run and reads `data/raw/`. `server` service: `depends_on: import: condition: service_completed_successfully`, port `8080:8080`, same volume.

- [ ] **Step 3: Build and smoke-test**

Run: `docker compose up --build -d && until curl -sf localhost:8080/api/health; do sleep 1; done`
Expected: health JSON with a `graph_date`.

- [ ] **Step 4: Verify the API through the stack**

Run: `curl -s -X POST localhost:8080/api/generate -H 'content-type: application/json' -d '{"start":{"lat":47.37,"lon":8.54},"distance_m":15000,"profile":{},"count":3}' | jq '.routes | length'`
Expected: a number from 1 to 3.

- [ ] **Step 5: Commit**

```bash
jj commit -m "build: lightweight image and compose stack"
```

---

### Task 16: End-to-end smoke and docs

**Files:**
- Create: `scripts/smoke.sh`
- Modify: `justfile`, `README.md`, `AGENTS.md`

**Interfaces:**
- Consumes: the demo graph and both binaries.
- Produces: `just smoke`, a README with setup, data, and attribution, and an AGENTS note that implementation has started.

- [ ] **Step 1: Write the smoke script**

`scripts/smoke.sh`: build the demo graph into `data/graph.bin` if missing, start `navilau-server` on port 18080 with `DATA_DIR="$PWD/data"`, poll `/api/health`, assert `POST /api/generate` returns at least one route and `POST /api/route` returns a geometry, then kill the server. The file name must match what `navilau-server` opens (`$DATA_DIR/graph.bin`), and `docker compose` uses the same path at `/data/graph.bin`. Use `jq -e` for assertions and `set -euo pipefail`.

- [ ] **Step 2: Run it**

Run: `just smoke`
Expected: exit 0, prints `health ok`, `generate ok`, `route ok`.

- [ ] **Step 3: Write the README**

Cover: what it is, `just data`, `just import`, `just dev`, `docker compose up`, the data sources and licences (`©swisstopo`, `© OpenStreetMap contributors`, SLF CC BY 4.0 when snow lands), and the deliberate limits (CH only, no accounts, no cross-border loops).

- [ ] **Step 4: Run the full check**

Run: `just check && just smoke`
Expected: both exit 0.

- [ ] **Step 5: Commit**

```bash
jj commit -m "test: end-to-end smoke and project docs"
```

---

## Self-Review Notes

- Spec coverage: §7 graph → tasks 1–2; §8 cost → task 4; §9 generation → tasks 9–10; §10 API → tasks 11–12; §11 frontend → tasks 13–14; §12 setup/deploy → tasks 15–16. §6.3 region packs are deliberately out of v1; the import functions take their sources as arguments, which is the seam to add a region later.
- Types are consistent across tasks: `LatLon`, `EdgeAttrs`, `Profile`, `Snap`, `Route`, `GeneratedRoute` are defined once and reused.
- `GeneratedRoute.waypoints` is the single source for drag handles. `/api/route` uses an empty `visited` set; the generator drops start-stem edges from its visited set. Both endpoints return `waypoints`.
- `Route` is `RouteStep`s, so mid-edge starts and same-edge waypoints are exact; geometry and stats are sliced, never rounded to edge endpoints.
- A* is forward-only with `Profile::min_cost_per_m()` as the bound. The visited set is a bitset, not a hash set.
- The DEM stays in its native Swiss grid; official trails are FlatGeobuf; land cover comes from the OSM PBF.
- Review Focus items 1–5 are pinned in Tasks 11, 10, 5/9, 8, 11 respectively.
- Deliberately not in the plan: viewshed, snow, sun/shade, POIs, webcams, reachability, PMTiles, CH. They belong to later plans.
