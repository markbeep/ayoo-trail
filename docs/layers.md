# Layers — data sources, licenses, algorithms

Research note, 2026-11-16. One section per layer. Status column: **v1** is the
first usable version, **later** is the layer milestone.

License conclusions checked on this date:

- **swisstopo OGD**: free use, distribution, enrichment, and **commercial**
  use; source reference mandatory (`©swisstopo`). No per-dataset fee.
- **swissTLM3D Wanderwege**: published under the same OGD terms; GPKG download,
  updated yearly, plus WMS/WMTS.
- **SLF (WSL)**: **CC BY 4.0**, free; attribution with a link; live data via
  `measurement-api.slf.ch`, registration recommended.
- **OpenStreetMap**: ODbL; attribution `© OpenStreetMap contributors`.
- **opentransportdata.swiss**: free GTFS, registration required.

## Terrain, slope, elevation

- **Source**: swisstopo DHM25 (25 m) for the horizon work; optionally
  swissALTI3D (0.5–2 m) for local detail. OGD, free.
- **Derivation**: sample the DEM along full way geometry at import; store
  per-edge length, gain, loss, and slope histogram. Per-direction slope is the
  same histogram traversed backwards, so no second field is needed.
- **Cost**: one pass over the network at import.
- **Status**: v1, first thing the importer does.

## Surface

- **Source**: OSM tags on ways (`highway`, `surface`, `smoothness`, `tracktype`).
- **Derivation**: map to a small enum at import: paved, compacted, gravel,
  dirt, grass, rock, unknown. Unknown is penalized, not assumed good.
- **Status**: v1.

## Technicality

- **Source**: OSM `sac_scale` (T1–T6) and `trail_visibility`; cross-check
  against the Wanderwege category if the dataset carries one.
- **Derivation**: store the numeric sac level (0 = missing) plus a confidence
  flag. Missing tags are a penalty, not a default.
- **Status**: v1.

## Official trails

- **Source**: `swissTLM3D Wanderwege` GPKG
  (`data.geo.admin.ch/ch.swisstopo.swisstlm3d-wanderwege/...gpkg.zip`).
- **Derivation**: match way geometry to official route geometry within a small
  tolerance; store official route id and category. Compute the per-route share
  of distance on official trails for display.
- **Cost**: one spatial join at import; `rstar` or a prepared geometry index.
- **Status**: v1.

## Forest and land cover

- **Source**: swissTLM3D land cover, or OSM `landuse`/`natural` polygons as a
  fallback.
- **Derivation**: TrailRouter's green-index idea — 30 m buffer around each way,
  fraction covered by forest. Store a per-edge fraction; display as a raster
  layer from the same polygons.
- **Cost**: spatial join at import; moderate.
- **Status**: v1 for the cost term, later for the overlay.

## Urban and remoteness

- **Source**: OSM `place`/`landuse=residential` or swissTLM3D settlement
  polygons; OSM roads for the noise proxy.
- **Derivation**: distance-to-settlement raster at import; per-edge penalty
  from the raster. The user's stance is strong: cities are a last resort.
- **Status**: v1.

## Start reachability

- **Source**: GTFS from opentransportdata.swiss (SBB + regional), plus the
  walking network for the last leg.
- **Derivation**: one-off precompute per home location: travel minutes to every
  stop (RAPTOR or a simple GTFS journey scan), plus walking time from the stop
  to the candidate start. Store a station table and a coarse travel-time
  raster.
- **Caveat**: this is a small project of its own and is deferred (D19). Any map
  point is a start in v1.
- **Status**: later.

## Viewshed and horizon profiles

- **Source**: swissALTI3D (already imported for elevation), resampled.
- **Key idea**: precompute a **horizon per grid cell**: in each of 32 azimuths,
  march a ray (earth curvature and refraction included, capped at 60 km) and
  keep the distance to the skyline, the point of maximum elevation angle.
- **Derivation**: 100 m LV95 grid over the region. Viewshed score = mean of the
  log-scaled skyline distances, stored as one `u8` per cell in `data/view.bin`
  ("how far you see", D100). Ridges and peaks stand out, valleys are dark.
- **Use (D100)**: a map overlay (`/api/tiles/view/{z}/{x}/{y}.png`, red =
  excellent), and a cost term `view` (default weight 0) sampled per edge at
  server start, so no graph re-import. With the weight up, vias also lean to
  high-view nodes.
- **Caveat**: no DEM outside CH, so rays leaving the country read as open.
  This is a shortcut until regions grow.
- **Cost**: the heaviest offline job in the project; estimate hours on a
  multicore laptop at 100 m, minutes at 200 m. Mark as estimate.
- **Status**: in progress (overlay and score).


## POIs

- **Source**: OSM nodes (`amenity=drinking_water|toilets|supermarket|...`).
- **Derivation**: extract at import into a point table; serve as vector tiles;
  nearest-POI query per route.
- **Status**: later.

## Webcams and photos

- **Webcams (D102)**: Windy Webcams API v3, free tier, key in
  `WINDY_API_KEY`. The server proxies `GET /api/webcams?bbox=` (the key stays
  private) and caches for 5 minutes, because free image URLs expire after about
  10 minutes. On the map: pins at zoom ≥ 10. A pin opens a popup with the preview
  image (not stretched) and a link to the Windy page, with the "Webcams
  provided by windy.com" credit.
- **SchweizMobil routes (D103)**: geo.admin.ch WMTS `ch.astra.wanderland`
  (hiking only). A click runs geo.admin identify (`chmobil_route_number`,
  `chmobil_title`) and lists the routes, each linking to its schweizmobil.ch page.
- **Hikr**: no API; deferred (Q27).
- **Status**: in progress.

## Elevation-profile matching

- **Source**: none; uses the target GPX and the graph.
- **Derivation**: compare candidate ascent/descent sequences with the target
  using a coarse histogram or dynamic time warping; score similarity as a cost
  term or as a filter.
- **Status**: later.

## Precompute summary

| Job | Output | Frequency | Rough cost |
|---|---|---|---|
| OSM import + attributes | `graph.bin` | monthly | minutes–hours |
| Official trail join | graph attribute | yearly | minutes |
| Land cover raster | PMTiles + graph attribute | yearly | minutes |
| Reachability from home | station table + raster | on demand | minutes |
| Horizon / view score | `view.bin` (8.7 MB) | once, DEM changes | 80 s for CH, 16 threads |
