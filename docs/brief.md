# Brief — ayoo

Status: **brainstorming**, 2026-11-16. This file is the live source of truth
until the design is approved; then `docs/design.md` takes over.

## One-liner

A web app that generates trail-running routes from a set of metrics and map
layers, instead of hunting through existing route catalogs.

## Why

The owner plans routes today in a manual six-tool loop. The pain is the
verification and cross-checking, not the idea of a route.

## How the owner plans routes today

1. **Find terrain.** 3D / relief shading (gpx.studio 3D, Coros 3D map, relief
   maps) to spot hills and mountains near home.
2. **Check access.** SBB or Google Maps for travel time; budget is about one
   hour each way.
3. **Generate.** TrailRouter round-trip at a target distance and a lot of
   climb; prefers nature and hilly.
4. **Make it real.** Export GPX into gpx.studio and drag points. Satellite
   view to confirm the trail actually exists (OSM can be stale). Hiking-route
   overlay (yellow/red/blue, Panoramawege) to stay on official trails. 3D view
   to follow ridges and to trade steep climbs against gentle descents.
5. **Verify.** Load into swisstopo: authoritative current trail network,
   closures and warnings. Usually one or two small edits.
6. **Check conditions.** Webcams, snow-height map, ShadeMap for sun/shade,
   SchweizMobil/hikr photos for surface character (gravel, roots, rock).
7. **Logistics.** POIs in gpx.studio (water, toilets, shops), then replicate
   onto the Coros watch for navigation.

Also uses their own tool [zill](https://github.com/markbeep/zill) to find
especially steep segments.

Observed defaults: ~15 km, ~1000 m climb (≈65 m/km), round-trip, start within
~1 h of home, nature over city.

What this means for the product:

- The value is collapsing a chain of apps into one screen with the same
  verification data inline.
- Trail existence and official-route membership are quality requirements, not
  cosmetic layers.
- Views matter, but secondary to terrain quality and trail existence.

## Goals

- **Fast.** Open the tool, set a small set of options, get a route in no time.
- **Deep.** Complicated settings and route finding for power users.
- **Extensible.** Layers/metrics can be added later, and layers can later sit
  behind tiers/paywalls. No paywalls initially.

## Product shape

- Web app (owner: "trail route webapp").
- Two modes of use: quick defaults, and a full control surface.
- No accounts or payment in v1; tiering is a later concern that the layer
  system must not block.
- Client is lightweight (SolidJS + MapLibre). Heavy computation runs on the
  server and is cached; nothing large is downloaded to a low-bandwidth client.

## Feature backlog

Grouped as the owner described them. None of this is scoped or prioritized yet.

### Route targets

| Feature | Notes |
|---|---|
| Target distance | TrailRouter-like; km. |
| Target elevation gain | Total ascent. |
| Steepness shaping | Prefer steep uphill over steep downhill. Affects the cost function, not just route totals. |
| Match a target elevation profile | Race preparation: find routes shaped like a known route. |

### Terrain and scenery

| Feature | Notes |
|---|---|
| Viewshed score | 360° visibility from a point; rewards ridges and peaks. |
| Remoteness | Distance from cities/settlements. |
| Forest cover | Prefer forest. |
| Surface | Prefer gravel/dirt over asphalt. |
| Technicality | Official hiking difficulty scale (e.g. Swiss T1–T6 / SchweizMobil) and/or derived from OSM tags and imagery (roots, rock, smooth gravel). |

### Access and logistics

| Feature | Notes |
|---|---|
| Start reachability | By car or train, Chronotrains-style; possibly several candidate start locations. |
| POIs | Toilets, grocery stores, water fountains near or on the route. |
| Snow | Estimate snow cover for a given date: aspect/sun-shade model plus snow-height data (e.g. MeteoSwiss, SLF). |

### Conditions and media

| Feature | Notes |
|---|---|
| Sun/shade | For a given day and time. |
| Webcams and photos | Along the route, to judge how it looks. |

### Route quality

| Feature | Notes |
|---|---|
| Official route matching | Prefer segments that belong to official hiking routes (SchweizMobil/Wanderland, OSM route relations) over abandoned OSM ways. |

### Map layers

| Feature | Notes |
|---|---|
| Per-metric overlays | Each metric viewable as its own map layer before routing (owner's step 1). |

### Controls

| Feature | Notes |
|---|---|
| Weight sliders | Per layer. |
| Exclusion markers | Point plus radius ("I've already run here"), lowers the score nearby. |
| Drag-to-edit | Move route points, with live score/cost (gpx.studio-like). |

## Owner's rough approach

1. Build per-metric map layers (e.g. red = excellent view in every direction).
2. Score path segments with the metrics. Open question: does combining metrics
   into one cost produce good routes, or does it wash out?
3. Cost-based route finder (bikerouter-like) returning several very different
   routes (trailrouter-like).
4. Weight adjustment, plus exclusion markers with radius.
5. Dynamic route editing with a continuously updated score.

Owner expects to focus on route planning/editing first and add layers after.

## Owner's existing infrastructure

[zill](https://github.com/markbeep/zill) is the owner's own tool and is directly
relevant to this project:

- `convert/` (Zig): OSM XML → `.zl`, a struct-of-arrays binary graph. Nodes:
  i64 OSM id, f32 lat/lon, u16 elevation. Edges: u32 endpoint indices, elevation
  gain, elevation loss, distance.
- `find/` (Zig): graph algorithms over `.zl`, compiled to `wasm32-freestanding`.
- `dashboard/`: SolidJS + TypeScript + MapLibre GL; `find` runs in a Web Worker,
  nothing is computed server-side.
- Swiss walkable graph: ~3.9M nodes, ~66 MB compressed; elevation from swisstopo
  DHM25.
- Limitations for this product: edges carry only distance and gain/loss (no
  surface, technicality, official-route membership, land cover); ways are
  chunked at ~100 m, so a GPX cut from the graph cuts corners.

Implication: CH-wide client-side routing is already proven feasible on this
stack, with no routing server.

Decision: the new project is a fresh Rust implementation, not a reuse of zill
(D10). zill's converter and algorithms are reference material.

## Inspirations / prior art

TrailRouter (loop generation, diverse alternatives), bikerouter (cost-based
routing, profiles), gpx.studio (route dragging), Chronotrains (travel-time
isochrones), SchweizMobil/Wanderland (official network, difficulty scale),
swissmobility (difficulty imagery), MeteoSwiss snow map, SLF, OSM.

## Open questions

| Q | Question | State |
|---|---|---|
| Q3 | Who is v1 for: just you, a private beta, or public at launch? | answered — D9 (personal, no accounts) |
| Q4 | Stack: Rust for engine/service; TypeScript + SolidJS + MapLibre for UI | answered in part — D10/D11 |
| Q5 | Hosting and monthly cost ceiling for tiles + route computation | answered in part — personal, one image; no cost cap stated |
| Q6 | Data pipeline: precompute a Swiss graph + layer rasters offline, or compute per request? | answered — D18 (offline graph + PMTiles) |
| Q7 | Route shapes beyond round-trip; how many alternatives and how different | answered — loops only, 6 routes, dedup at 0.5 overlap (design §9) |
| Q9 | Technicality source: official Swiss scale, OSM tags, imagery, or a blend | answered — OSM `sac_scale` + Wanderwege category, missing tags penalized |
| Q10 | Licensing: OSM/ODbL, swisstopo, SLF, webcams/photos | answered in part — swisstopo OGD free incl. commercial with attribution, SLF CC BY 4.0, GTFS free with registration; webcam/photo terms open |
| Q11 | Outputs: GPX (confirmed, for Coros), share links, saved routes, mobile/offline | answered in part — GPX download + URL share; no saved routes, no mobile |
| Q12 | Persistence without accounts: local storage, share links, or nothing in v1 | answered — localStorage + URL, no database |
| Q13 | Route editing: full graph snapping like gpx.studio, or waypoint shaping first | answered in part — drag editing in v1 (D14); snapping semantics in design |
| Q14 | Unmaintained OSM trails: exclude outright, or strongly penalize? | answered — D16 (allow all walkable, bonus official, show share) |
| Q15 | Interaction model: generator-first (options → routes) or workspace-first (layers + manual drawing, generator assists) | answered — D13 (generator-first) |
| Q16 | Routing engine: hand-rolled Rust vs GraphHopper service vs Valhalla bindings (see `docs/routing-engines.md`) | answered — D12 (hand-rolled Rust) |
| Q17 | What counts as "steep uphill" (preferred) vs "too steep downhill" for the cost model? | answered in part — defaults in design §8; continuous uphill/downhill sliders (D53) and climb pressure (D52) now let the user tune it |
| Q18 | Travel-time reachability from home | answered — D19 (deferred) |
| Q19 | Coverage beyond Switzerland | answered — design §6.3 region packs; CH only in v1 |
| Q20 | PWA/offline: installable shell + cached routes, or offline generation via region packs + WASM | answered — D23 (not supported) |
| Q21 | Design review 2026-11-17 (`docs/report/report.pdf`): 28 proposed changes and choices R1–R28 (official-trail source, cost-model form, grade bands, closures, generation budget, personas, 3D terrain, border buffer, data spike first, …) | open — owner to review the decision register (report ch. 17) |
| Q22 | Future layers (snow, heatmap, viewshed, …): separate base maps, overlays on top of Topo/Satellite, or both? The client registry has an exclusive base-map list and an empty overlay slot, so either fits; the UI lists Snow depth and Popular trails as disabled "Later" rows | answered — D41 (overlays stack on an exclusive base map) |
| Q23 | Score-breakdown wording: the choice placeholders are replaced by Off … Above all sliders (D51, D56); the per-term questions ("How much should technical trails count against a loop?") are still placeholders to confirm | open (wording only) |
| Q24 | Should busy roads (primary/secondary/tertiary) get their own road-class/traffic factor? Today an asphalt farm lane and a main road cost the same. The highway class is stored per edge (D31), so this is a cost-table change only | open |
| Q25 | Generation speed target (owner: low ms for ~20 km on 1–2 vCPU / ≤2 GB, cities too; 50–100 km may be slower). After D69, one thread: Wattwil 178 ms, Bichelsee 133 ms, Geneva 0.6 s, Zürich 1.3 s. Proposed steps: (1) per-request region subgraph with precomputed costs, same loops; (2) shared forward/backward trees from the start per climb pressure, same loops; (3) vias placed by network distance from that tree and climb pressure on a shared grid, so fewer retries (loops change); (4) bidirectional A* for the via-to-via legs; (5) cities: separately mapped sidewalks and crossings (`footway=sidewalk/crossing`) are ~55 % of edges near Zürich HB, so drop or merge them at import. Hierarchies (CH/CCH) don't fit per-request weights and the 2 GB box | in progress — steps 1–2 done (D70); 3–5 open |
| Q26 | Preferences should shape where loops go, not just the path between fixed points. Today 24 seeds sit at fixed compass headings and their vias are pulled to the nearest peak or viewpoint; weights only change the A* legs between them, and ranking is mostly distance and climb error (re-ranking the 24 by the user's weighted score moved built-up km by 1–2, tested). Proposed: (1) place vias from the start's shortest-path tree under the user's weights (the D70 shared start tree): in each heading sector, among nodes at the wanted network distance, pick the one whose tree path from the start is cheapest per meter, so a via sits where preferred terrain leads (this is Q25 step 3 too); peaks and viewpoints stay as a bonus, not the only pull; (2) among loops within the distance tolerance, rank by the user's weighted score (D75) before climb and distance fit; (3) acceptance probe at Wattwil, Zürich and Bichelsee, 15 km: built-up km falls and forest share rises monotonically from Off → Normal → Above all, by a clear margin, and generation stays within the Q25 budget. Loops will change for everyone | proposed, awaiting owner approval |
