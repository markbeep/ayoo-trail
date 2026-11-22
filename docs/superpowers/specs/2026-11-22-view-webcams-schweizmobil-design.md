# Plan: 360° view, webcams, SchweizMobil routes; drop snow; defer hikr

## Context

v1 is done. The owner wants the next layer: the **360° view (viewshed)** as a map
overlay *and* a routing term that pulls vias toward good views. They also want
**webcams** and **SchweizMobil routes** on the map, clickable through to their
own pages. **Snow is dropped** from the plan (too flaky, too niche). **Hikr is
deferred** and goes to the open questions.

Owner answers (2026-11-22):
- Viewshed: overlay + cost term + vias re-picked toward views. Metric is **how
  far you see** (mean distance to the skyline over 360°). Default weight **0 (off)**.
- **Any setting change re-picks the vias of every generated loop.**
- **Editing makes a copy.** Dragging Loop A leaves A as generated and creates
  **A\*** (superscript star). The copy keeps its waypoints: a setting change
  re-routes it between those points and never re-picks them. The copy has a
  **Delete** button instead of Reset. This replaces today's in-place edit plus
  Reset (`web/src/state.ts:185-234, 262`).
- Webcams: **Windy Webcams API**. Clicking one opens a popup with the preview
  image and a link to its Windy page.
- SchweizMobil: **hiking only** (`ch.astra.wanderland`). Clicking opens a popup
  listing the route(s) under the click, each linking to schweizmobil.ch.
- Info popups are off while drag-editing or placing areas.

Facts found:
- Vias already move when weights change: `via_near` (`src/region.rs:313`) picks the
  node with the cheapest out-and-back cost per meter within r·0.5 of the seed
  point. A view cost term therefore pulls vias on its own. A node-level view
  bonus on top of that gives the "pull" that D92 deferred.
- The full swissALTI3D (43k tiles, `data/raw/alti3d`) is already used through
  `Alti3dTiles`/`ElevationSource` (`src/import/alti3d.rs`, `dem.rs`).
- `ch.astra.wanderland`: WMTS
  `https://wmts.geo.admin.ch/1.0.0/ch.astra.wanderland/default/current/3857/{z}/{x}/{y}.png`
  (matrix set `3857_18`). Identify accepts `sr=4326` and returns
  `chmobil_route_number` and `chmobil_title`, but **no schweizmobil URL**: the
  link is built from the route number (pattern to verify).
- Windy free tier: header `x-windy-api-key`, `/webcams/api/v3/webcams`. Image
  URLs expire after about 10 minutes, the docs say to fetch the list on each page
  load, every image must link back, and the "Webcams provided by windy.com"
  credit must be shown. Param names (bbox, include) still need checking against
  their OpenAPI spec.
- Cargo has no HTTP client and no PNG encoder.

## Steps

Each step: failing test → implement → pass → `jj commit`.

0. **Docs first (same turn as the decisions).**
   - `docs/decisions.md`:
     - D99: snow dropped.
     - D100: viewshed scope, metric and default weight 0.
     - D101: setting changes re-pick vias of generated loops. Editing creates
       an A\* copy that keeps its points and has Delete instead of Reset.
       Refines D54/D55.
     - D102: Windy webcams, popup with thumbnail and link, served through a server proxy.
     - D103: SchweizMobil Wanderland overlay with an identify popup.
     - D104: info clicks off while editing.
   - `docs/brief.md`: remove the snow rows, add a hikr open question (Q27),
     narrow Q10 to the Windy and SchweizMobil terms.
   - `docs/layers.md`: remove the Snow section and its precompute row. Rewrite
     Viewshed and Webcams for the chosen design.
   - `docs/design.md`: remove the mentions of snow.
   - Write the spec: `docs/superpowers/specs/2026-11-22-view-webcams-schweizmobil-design.md`.

1. **Edited copies (A\*).** In `web/src/state.ts`:
   - The first drag on a generated loop inserts a copy right after it, with
     `id = <id>*`, a `copyOf` field and the edited waypoints. The original stays
     untouched and the copy is selected. Further drags on the copy edit it in
     place. **Inference to confirm:** Undo stays on the copy through its
     `history`, and undoing past its first edit deletes the copy.
   - `findRoutes` live refresh: generated loops come fresh from `/api/generate`.
     Each copy is re-routed through its own waypoints (the existing `reroute`)
     and kept next to its original, matched by `copyOf`. If its original is gone,
     the copy goes at the end. Copies survive a refresh; `editQuery` ("Change")
     still clears everything.
   - `resetEdit` → `deleteCopy(id)`. In the RouteCard / RouteTiles UI, Reset
     becomes Delete on copies, and the label renders A\* (for example
     `Loop A<sup>*</sup>`).
   - Share link: `wps` are encoded when the selected loop is a copy.
     `openShared` regenerates and then recreates the copy from `loop` + `wps`.
   - Test (vitest, next to `url.test.ts`): copy creation and merging on refresh
     as a pure helper (`mergeCopies(fresh, copies)`).

2. **Drop snow from the UI.** Remove the `snow` entry from `LATER_OVERLAYS`
   (`web/src/map/basemaps.ts:77`).

3. **Horizon precompute**: `src/import/horizon.rs`, plus an import flag or a
   `just view` recipe.
   - 100 m LV95 grid over the region, sampled from `ElevationSource`.
   - 32 azimuths, ray march with a growing step, earth curvature and refraction,
     cap 60 km. For each azimuth, keep the distance to the point of maximum
     elevation angle.
   - Score = mean of the log-scaled distances, as a `u8`.
   - Write `data/view.bin`: header (magic, origin E/N, cell size, w, h) followed
     by the u8 cells. Run with rayon over rows.
   - Tests on a synthetic DEM: the peak cell scores above the valley cell, and
     the score is monotonic in visible distance.
   - `shortcut: no DEM outside CH, border rays read as open; add Mapterhorn/EU DEM when regions grow.`
   - The runtime cost is an estimate (minutes to an hour). Measure it and record
     the result in layers.md.

4. **View in routing** (`src/cost.rs`, `src/region.rs`, server startup).
   - Load `view.bin` if it is present. When it is missing, the factor stays
     neutral and the API reports the view as unavailable.
   - At startup, give each edge a `view` value: the mean of the grid at its
     geometry vertices, kept in memory with no graph format change. The reason:
     no re-import, and the layer stays additive.
   - `Weights.view` with serde default 0, and a `VIEW_FACTORS` table applied
     like `FOREST_FACTORS`.
   - `via_near`: multiply `ratio` by the node's `view_factor^w`.
   - Score: add a "Views" bar. **Inference to confirm:** the total score
     includes it only when its weight is > 0, so default scores stay unchanged (D44).
   - Tests: the table is monotonic. In a synthetic region with view weight > 0,
     the via picks the high-view node, and at weight 0 the result is unchanged.

5. **View overlay tiles.** `GET /api/tiles/view/{z}/{x}/{y}.png`, rendered from
   the grid: tile pixel → wgs84 → `wgs84_to_lv95` → grid cell → colour ramp
   (red = excellent, per the owner's step 1), transparent outside the grid. Add
   the `png` crate (small, standard). Send a long Cache-Control. Test: a known
   tile pixel has the expected colour.

6. **Client view.**
   - Move `view` from `LATER_OVERLAYS` to `OVERLAYS` as a raster source on our
     endpoint.
   - Add `view: 0` to `Weights` (`web/src/routes/types.ts`) and `DEFAULT_WEIGHTS`.
   - Add a "Views" score bar with its slider in the Adjust panel, following the
     existing bars (`RouteCard`, `Slider`).
   - Old share links keep working through `{...DEFAULT_WEIGHTS, ...s.weights}`.

7. **Webcams.**
   - Server: `GET /api/webcams?bbox=` proxies Windy through `ureq` (blocking,
     inside `spawn_blocking`) with `WINDY_API_KEY` from env/clap. Reason: the key
     stays private.
   - It caches results in memory for 5 minutes, under the 10-minute image
     expiry, and returns GeoJSON (id, title, preview URL, page URL).
   - Without a key it returns 503, and the client hides the overlay.
   - First verify the v3 params against Windy's OpenAPI spec. Test: a fixture
     Windy response parses to the expected GeoJSON.
   - Client: a "Webcams" overlay. While it is on and zoom ≥ 10, fetch on
     `moveend` and draw a circle layer. Clicking opens a MapLibre Popup with the
     image (never stretched), the title, a link to the Windy page and the Windy credit.

8. **SchweizMobil.**
   - A "SchweizMobil hiking" overlay that reuses the `TRAILS_SOURCE` pattern
     with `ch.astra.wanderland` (check maxzoom 18 with one z18 tile).
   - Clicking runs identify on `api3.geo.admin.ch` from the client (check CORS;
     if it's missing, add a thin server proxy next to the webcam one) with
     `sr=4326` and a pixel tolerance.
   - The popup lists the distinct route numbers with their titles, each linking
     to the schweizmobil.ch route page (check the URL pattern).
   - Attribution: ASTRA / SchweizMobil (check the licence on the geocat record).

9. **Click gating.** Webcam and route popups open only when not drag-editing,
   placing an area or picking the start. Reuse the existing mode state in
   `mapController` / the map menu.

## Verification

- `just check` (fmt, clippy, tests, typecheck) and `just smoke`.
- Generate at Wattwil, 15 km, with view weight 0 and then 2. With weight 0, the
  loops match today's. With weight 2, the Views bar rises and the vias sit on
  higher-view nodes.
- `just dev`, then check by eye:
  - the view overlay is red on ridges and dark in valleys;
  - webcam pins open a popup with a live image and a working link;
  - a SchweizMobil route click opens the right schweizmobil.ch page;
  - no popups appear in edit mode;
  - dragging Loop A creates A\*; moving a slider re-picks A's vias, while A\*
    keeps its points and is only re-routed; Delete removes A\*.

## Left for the builder / to verify

- Windy v3 param names and caching terms.
- Wanderland maxzoom, CORS and the schweizmobil URL pattern.
- The VIEW_FACTORS values (to tune, like Q17).
- The horizon runtime at 100 m.
