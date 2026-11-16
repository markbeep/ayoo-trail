# Layers — data sources, licences, algorithms

Research note, 2026-11-16. One section per layer. Status column: **v1** is the
first usable version, **later** is the layer milestone.

Licence conclusions checked on this date:

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
  dirt, grass, rock, unknown. Unknown is penalised, not assumed good.
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

- **Source**: DHM25 only.
- **Key idea**: precompute a **horizon profile per grid cell** — the elevation
  angle of the skyline in N azimuth bins (32–64). One raycast per cell per bin.
- **Derivation**: 100–200 m grid over Switzerland (~4.1M cells at 100 m, ~1M at
  200 m). Store as `u8` degrees per bin. Viewshed score = mean horizon distance
  or mean elevation angle; ridges and peaks stand out, valleys are dark.
- **Reuse**: the same profile answers sun/shade for any date and time by
  comparing the sun altitude with the horizon angle at the sun azimuth. One
  precompute, two layers.
- **Cost**: the heaviest offline job in the project; estimate hours on a
  multicore laptop at 100 m, minutes at 200 m. Mark as estimate.
- **Status**: later (overlay and score).

## Sun and shade

- **Source**: horizon profile above + solar position (standard formulas, no
  external data).
- **Derivation**: per query date/time, shade = sun altitude < horizon angle at
  sun azimuth. Sample along the route; show shaded fraction and a map overlay
  for the requested time.
- **Status**: later.

## Snow

- **Source**: SLF station measurements (CC BY 4.0), historical and live.
- **Derivation**: station snow depth + elevation/aspect correction, interpolated
  to the DEM. Present as an estimate with a wide band, never as fact.
- **Caveat**: station density is sparse in some ranges and interpolation across
  ridges is unreliable. Sentinel-2 snow cover is the better source but adds a
  satellite pipeline and cloud handling.
- **Status**: later, and explicitly an estimate.

## POIs

- **Source**: OSM nodes (`amenity=drinking_water|toilets|supermarket|...`).
- **Derivation**: extract at import into a point table; serve as vector tiles;
  nearest-POI query per route.
- **Status**: later.

## Webcams and photos

- **Source**: unclear. Public webcam networks (cantonal, Roundshot) have
  varying APIs and terms; hikr.org photos are user-uploaded with mixed
  licences; SchweizMobil has photos of official routes.
- **Derivation**: link-out rather than embedding unless licensing is clear.
- **Status**: later; needs its own licence investigation.

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
| Horizon profiles | `horizon.bin` / PMTiles | once, DEM changes | hours (estimate) |
| Snow | raster | daily/seasonal | minutes |
