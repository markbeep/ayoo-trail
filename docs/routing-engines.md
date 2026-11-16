# Routing engine options

Research note, 2026-11-16. Feeds Q16 and the architecture section of the
design. Facts come from the projects' docs and source on this date; effort
figures are estimates.

**Chosen: A, hand-rolled Rust (D12).** GraphHopper is the correctness
reference, not a dependency.

## What the engine must support

For trail running in Switzerland:

1. **Direction-dependent slope cost** — cheap or neutral steep uphill,
   expensive steep downhill.
2. **Per-edge attributes beyond OSM defaults** — surface, technicality
   (sac_scale), official-route membership, forest/land cover, viewshed, snow.
3. **Per-request weights** over those attributes (the sliders).
4. **Exclusion areas** — point plus radius that down-weights or blocks edges.
5. **Hard constraints** on total distance and elevation.
6. **Loop generation** with a target distance and diverse alternatives.
7. **Server-side execution with caching**; the client is SolidJS + MapLibre and
   must never download the graph.
8. New code in Rust is preferred.

Only 2–4 are engine work. 5–6 sit above any engine and are written by us in
every option.

## A. Hand-rolled Rust

**Shape**

- Import: `osmpbf` for planet/extract PBF; walkable ways only; keep full way
  geometry. Elevation sampled from swisstopo DHM25/ALTI3D at import.
- Attributes baked per edge at import: surface, smoothness, `sac_scale`,
  official-route membership (Swisstopo/Wanderland + OSM route relations),
  forest/land-cover fraction, viewshed score, snow model.
- Graph: CSR arrays. Per edge: endpoints, length, gain/loss, per-direction
  slope stats, attribute block, geometry offset. Node coordinates and edge
  geometry in flat arrays.
- Search: A* with haversine heuristic and a per-request cost closure. For
  fixed weight sets, `fast_paths` (Rust CH, MIT/Apache-2.0) gives ~100 µs
  queries on California-sized graphs; dynamic weights (exclusion areas,
  visited-edge penalties) use plain A* over the same graph.
- Snapping: `rstar` over edge segments.
- Service: `axum`; results cached by (start cell, rounded params, weight hash).

**Pros**

- The cost function is the product, and nothing caps it. Asymmetric slope
  cost, any per-edge scalar, any constraint is a function edit.
- One language, one binary, no JVM, cheap to run, easy to cache.
- The owner has already written an OSM→graph converter and graph algorithms
  in Zig (zill), so the shape of the work is known.
- Full geometry is kept, so GPX and 3D display are exact.

**Cons**

- You own correctness: access rules, disconnected components, snapping,
  elevation sampling, ferries, steps, geometry stitching.
- No turn instructions (not needed for trail running; GPX to the watch).
- Weeks of work before the first route exists. Rough estimate: import 1–2
  weeks, graph + search 1–2 weeks, loop generation 1–2 weeks.

**Mitigation**

- Copy GraphHopper's OSM tag rules as the reference for walkable/surface/
  sac_scale parsing rather than inventing them.
- Validate outputs against GraphHopper or BRouter on a sample of routes.

## B. GraphHopper

**Facts**

- Java 25, Apache 2.0. Library or standalone server.
- Encoded values already include `surface`, `smoothness`, `hike_rating`
  (sac_scale 0–6), `average_slope`, `max_slope`, `foot_network`,
  `urban_density`, `country`. Slope values flip sign with direction, so a
  priority rule can penalise steep descent specifically.
- Custom models are JSON per request: `speed`/`priority` rules over encoded
  values, plus `areas` — polygons passed in the request and referenced as
  `in_<id>`. That covers the exclusion-marker feature directly.
- Modes: CH "speed mode" (fixed profiles), flexible mode (request-time
  weights), hybrid in between.
- Elevation, round-trip routing, GTFS transit, isochrones, map matching.

**Pros**

- Closest off-the-shelf fit for the attribute set. Most of requirement 2 is
  already parsed from OSM.
- Request-time polygons and custom models cover 3 and 4.
- Mature and fast; CH handles Switzerland easily.

**Cons**

- No hard constraints on total distance/elevation (5) — external loop layer
  needed, as with everything else.
- New layers (viewshed, official trail, snow) need a custom Java
  `EncodedValue` in the import, i.e. a custom build, not just config.
- JVM service in an otherwise Rust stack; more ops and memory than a single
  binary.
- Loop generation (6) and dedup are ours.

## C. Valhalla

**Facts**

- C++17, MIT. Tile-based, low memory, cross-compiles; Rust bindings exist
  (`valhalla` crate, builds from source).
- Pedestrian costing includes `max_hiking_difficulty` (sac_scale 0–6),
  `use_hills` (symmetric, not up/down-asymmetric), `use_tracks`, `use_lit`,
  `walkway_factor`, `max_distance`.
- Costing is dynamic at runtime but implemented by subclassing `DynamicCost`
  in C++ (`EdgeCost`, `Allowed`, `TransitionCost`); new cost terms need a
  rebuild, not a request parameter. The README's "plugin architecture" is
  this C++ extension point.
- No request-time arbitrary polygon penalty areas; `search_filter` covers
  tunnels, bridges, tolls, ferries, closures, steps, road classes.
- Transit/multimodal, matrix, isochrones, map matching, TSP.

**Pros**

- MIT, fast, low memory, tiles and partial updates.
- `sac_scale` as a hard maximum is built in; strong multi-modal support if
  train-reachability ever needs live routing.
- Rust bindings make it callable from a Rust service.

**Cons**

- Asymmetric steep-up/steep-down cost needs C++ costing work.
- Custom per-edge layers need tile-builder changes in C++.
- No polygon penalty areas at request time.
- No loop generation.

## D. Openrouteservice

**Facts**

- Java, **GPLv3**, built on a fork of GraphHopper 4.0.
- Directions support `round_trip` (points placed on a circle around the
  start, controlled by `length`, `points`, `seed`), green weighting, and
  `extra_info` (surface, waytype, steepness).
- TrailRouter forked and heavily modified it: custom flag encoders, a green
  index computed in PostGIS, visited-edge penalties for anti-repetition,
  feature-based loop candidates in a Postgres stored procedure. Planet
  preprocessing took ~5 days on 256 GB RAM servers.

**Pros**

- Round trip and green weighting exist, and TrailRouter proves the concept.
- `extra_info` gives useful per-edge data for display.

**Cons**

- GPLv3 is a real constraint for a forked, distributed product; hosting is
  fine, but the modified engine cannot be shipped closed.
- Based on an old GraphHopper generation; fork maintenance is permanent.
- No hard constraints either; TrailRouter still wrote all of the loop logic
  outside the engine.
- Most of TrailRouter's value is in its modifications, not the base engine.

## E. BRouter

**Facts**

- Java, MIT, elevation-aware, custom binary segments (`.rd5`).
- Profiles are a small DSL; the cost function is editable as text and is the
  fastest thing here to iterate on.
- Round-trip routing exists since 1.7 (bikerouter ships it).

**Pros**

- MIT, proven by bikerouter, very quick to tune weights.
- Good elevation/energy handling.

**Cons**

- JVM, no CH; slower per query, single-threaded by default.
- Adding a new per-edge attribute (viewshed, official trail) means changing
  the segment format and the profile data model.
- No multi-objective or constraint support.

## Rejected quickly

- **OSRM** — car-centric, no elevation, no per-edge custom costs.
- **Hosted APIs** (GraphHopper, ORS, Mapbox, Google) — no custom layers, no
  loop control, usage limits and cost.

## Matrix

| | A hand-rolled Rust | B GraphHopper | C Valhalla | D ORS | E BRouter |
|---|---|---|---|---|---|
| Language | Rust | Java | C++ (Rust bindings) | Java | Java |
| License | ours | Apache 2.0 | MIT | GPLv3 | MIT |
| Built-in attrs | we bake | strongest | good | good | medium |
| Request-time weights | any | custom model | costing options | profiles | profiles |
| Exclusion polygons | any | yes (request GeoJSON) | no | no | no |
| New custom layer | natural | Java encoded value | C++ tile build | Java + Postgres | segment format |
| Asymmetric slope cost | yes | yes (slope sign) | C++ edit | fork | profile (if attrs) |
| Hard constraints | ours | ours | ours | ours | ours |
| Loop generation | ours | naive built-in, ours better | no | circle-based | yes since 1.7 |
| Transit | separate | GTFS | yes | no | no |
| Ops | 1 binary | JVM | binary | JVM | JVM |

## Recommendation

**A, hand-rolled Rust**, with GraphHopper's tag rules and output as the
reference to validate against.

The deciding argument: every option needs custom import work for the layers
that make this product different (viewshed, official trails, snow), and every
option leaves constraints and loop generation to us. The engine only supplies
OSM parsing, graph storage, snapping and A* — the part the owner has already
built once, in a harder language. Paying a JVM and a forked import layer for
that is a poor trade when the cost function is the product.

Fallbacks, in order:

1. If the import milestone overruns, wrap GraphHopper as a service behind the
   same API and keep the loop/scoring layer in Rust.
2. If multi-modal transit routing becomes a live requirement, Valhalla via
   Rust bindings is the better base — but only then.

## Open sub-decisions

- CH (`fast_paths`) for fixed profiles versus plain A* for everything. Depends
  on how often exclusion areas and visited-edge penalties must apply.
- Where loop candidates come from: heading rings (TrailRouter), feature
  anchors (peaks, ridges, official routes), or both.
- Cache granularity: per start cell, per weight hash, or precomputed candidate
  sets per region.
