# Brief — ayoo

Status: v1 built, 2026-11-20. This file keeps the intent, the feature backlog and
the open questions; `docs/design.md` describes what is built, and the README lists
the built and planned routing metrics.

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
| Viewshed score | 360° visibility from a point; rewards ridges and peaks. Overlay and cost term (D100). |
| Remoteness | Distance from cities/settlements. |
| Forest cover | Prefer forest. |
| Surface | Prefer gravel/dirt over asphalt. |
| Technicality | Official hiking difficulty scale (e.g. Swiss T1–T6 / SchweizMobil) and/or derived from OSM tags and imagery (roots, rock, smooth gravel). |

### Access and logistics

| Feature | Notes |
|---|---|
| Start reachability | By car or train, Chronotrains-style; possibly several candidate start locations. |
| POIs | Toilets, grocery stores, water fountains near or on the route. |

### Conditions and media

| Feature | Notes |
|---|---|
| Sun/shade | For a given day and time. |
| Webcams and photos | Along the route, to judge how it looks. Webcams from Windy (D102) and SchweizMobil routes (D103) as clickable map layers; hikr is open (Q27). |

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
| Weight sliders | Per layer. Built as typed multipliers with avoid/prefer (D106). |
| Exclusion markers | Point plus radius ("I've already run here"), lowers the score nearby. Built as avoid/prefer areas, soft or hard (D90, D97). |
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

[zill](https://github.com/markbeep/zill), the owner's Zig + WASM routing tool over a
Swiss walkable graph, is reference material only; ayoo is a fresh Rust
implementation (D10).

## Inspirations / prior art

TrailRouter (loop generation, diverse alternatives), bikerouter (cost-based
routing, profiles), gpx.studio (route dragging), Chronotrains (travel-time
isochrones), SchweizMobil/Wanderland (official network, difficulty scale),
swissmobility (difficulty imagery), MeteoSwiss snow map, SLF, OSM.

## Open questions

Answered questions are recorded in `docs/decisions.md`.

| Q | Question | State |
|---|---|---|
| Q5 | Hosting and monthly cost ceiling for tiles + route computation | answered in part — personal, one image; no cost cap stated |
| Q10 | Licensing: OSM/ODbL, swisstopo, SLF, webcams/photos | answered in part — swisstopo OGD free incl. commercial with attribution, SLF CC BY 4.0, GTFS free with registration; Windy webcam caching terms and SchweizMobil (ASTRA) attribution still to confirm |
| Q27 | Hikr.org reports on the map. No public API, so it means scraping (points or tracks) after checking hikr's terms and robots.txt | open — deferred by the owner, 2026-11-22 |
| Q25 | Generation speed target: low ms for ~20 km on 1–2 vCPU / ≤2 GB, cities too; 50–100 km may be slower. After D94, one thread, 15 km: Wattwil ~40 ms, Bichelsee ~30 ms, Geneva ~130 ms, Zürich HB ~190 ms (2–4× that with a climb target). Remaining idea: (4) bidirectional A* for the via-to-via legs. Hierarchies (CH/CCH) don't fit per-request weights and the 2 GB box | in progress — steps 1–3 done (D70, D94); 4 open; 5 dropped |
