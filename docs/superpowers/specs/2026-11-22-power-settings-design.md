# Power-user settings, no default start, Churfirsten must-pass loops, dense redesign — design

Status: approved by the owner, 2026-11-22. Decisions D106–D112 in `docs/decisions.md`.

## Context

The owner wants ayoo to work for power use, not only vibe sliders:
- every setting can be avoided or preferred, with a typed multiplier (×0.01, ×100 …)
  instead of an Off…Above-all slider; plus one "clear all" so only distance drives the route;
- no Wattwil default start;
- a real test case: a loop from **Alt St. Johann** over all 7 Churfirsten (Selun, Frümsel,
  Brisi, Zuestoll, Schibenstoll, Hinterrugg, Chäserrugg) as must-pass points, all costs off,
  distance at the minimum, giving a clean route without loops and odd detours;
- a denser, squarer, less "AI-slop" UI (impeccable redesign).

Owner answers (2026-11-22): loop from Alt St. Johann; one × per setting with a
Photoshop-style scrub field (drag left/right); clear all = **everything**; one dense
settings table, score bars become read-only.

What blocks the Churfirsten case today (found in `src/generate.rs`, `src/route.rs`):
1. `MAX_MUST = 5` (server `src/cost.rs:27`, client `web/src/routes/zones.ts`).
2. Musts are ordered by compass bearing from the start, not by route cost.
3. A summit is a dead end: `drop_backtracks` (`src/route.rs:297`) cuts the whole
   out-and-back spur, silently dropping the peak; and `REPEAT_PENALTY` ×5 (`src/route.rs:17`)
   pushes the way back down onto a different, worse trail ("bad ways").
4. Every seed adds 2–3 free vias, so at the minimum distance every loop overshoots or loops.

## My calls (correct them at approval)

- Returning users keep their own last start from localStorage; only the Wattwil fallback
  goes. First open shows all of Switzerland, no start marker, "Find loops" disabled until
  a start is picked.
- Clear all resets: all multipliers ×1, hills and technical neutral, climb target off
  (new "no climb goal" state), all map areas removed, hardest trail back to the default T3.
- Multiplier range ×0.001 … ×1000. Scrub: horizontal drag on a log scale (~×1.01 per px,
  Shift ×10 faster), a click without drag types, arrow keys step, commits on release so
  live regeneration (D55) runs once per gesture.
- Hills and Technical keep their shape controls (uphill/downhill gentle…steep; hardest
  trail + avoid…seek) and their × is strength only (≥ ×1). A negative exponent there would
  e.g. start preferring untagged trails.
- Must-pass limit 5 → 10.
- The "shortest loop through the must-pass areas is X km" error gets a **Use X km** button.
- The settings table stays reachable while loops are shown (live refresh), not only in
  the pre-search query card.

## Design

### Weights: signed exponents, shown as multipliers (server + client)
Keep the API and share links in exponent form (`f^w`, D51); just widen it.
- `src/cost.rs` `Profile::validate`: surface, official, forest, urban, traffic, view in
  `[-100, 100]`; slope, tech in `[0, 100]`. `MAX_WEIGHT` → 100. `term_minima`/`min_weighted`
  already take the min over the table, so the A* bound stays admissible for negative `w`.
- Client `web/src/routes/` new helper: key factor per term (asphalt 2.0, marked 0.7,
  forest 0.9, town 2.0, busy road 2.0, wide view 0.7, steepest band / T4+ for hills/tech),
  `multiplier = key^w`, `w = ln M / ln key`. Constants copied from `cost.rs` with a comment
  naming it; a test pins them.
- `web/src/url.ts:74` weight clamps widened to the same ranges.

### Penalties the user can set (owner, 2026-11-22)
Owner picked: repeat penalty, area strengths (one value for all areas), start stem, loop
overlap. Untagged ×1.3, climb discount and ranking weights stay fixed.

| Setting | Today | Range (my call) | Lives in |
|---|---|---|---|
| Repeat penalty | `REPEAT_PENALTY` 5 (`src/route.rs:17`, used `route.rs:112`, `region.rs:378`) | ×0.01–×1000 (owner: below 1 allowed) | `Profile.repeat` |
| Soft avoid areas | `EXCLUSION_PENALTY` 5 (`cost.rs:23`) | ×1–×1000 | `Profile.avoid_factor` |
| Soft prefer areas | `PREFER_FACTOR` 0.5 (`cost.rs:25`) | ×0.001–×1 | `Profile.prefer_factor` |
| Start stem | `STEM_M` 500 m (`generate.rs:24`) | 0–5000 m | generate body `stem_m` |
| Loop overlap | `MAX_OVERLAP` 0.5 (`generate.rs:30`) | 0–100 % | generate body `max_overlap` |

- The constants become the serde defaults, so old clients and share links behave as before.
  Repeat and area factors sit on `Profile` because drag edits (`/api/route`) use them too.
- A* bound: `min_cost_per_m` also multiplies by `min(1, repeat)` (and by `prefer_factor`
  instead of `PREFER_FACTOR`), so it stays admissible below ×1. Below ×1 the search
  gets slower; measure.
- Rows in the settings table use the same `ScrubField` (m and % for stem/overlap); url.ts
  carries them.
- Clear all puts these back to their defaults instead of ×1 (my call: they shape the loop,
  they are not preferences; repeat ×1 would allow out-and-backs everywhere).

### Must-pass loops (`src/generate.rs`, `src/route.rs`)
- `MAX_MUST = 10` (both sides).
- Order musts by route cost: leg-cost matrix between start and musts via `region.leg`,
  exact Held-Karp over ≤ 10 points; free vias inserted where they add the least detour.
- Owner (2026-11-22): "when there are must-pass markers, just use them as vias if
  possible". Reading: with musts, the loop through the musts alone (already computed as
  `shortest` for the too-far check) is the route whenever it is within the distance
  tolerance; seeds with free vias run only when that loop is too short, to fill the distance.
- Summit spurs: returning from a must-pass via doesn't pay `REPEAT_PENALTY` on the leg
  that just arrived there, and `drop_backtracks` keeps a spur whose tip is a must-pass.

### Client state (`web/src/state.ts`)
- `start` becomes `LatLon | null`; drop `DEFAULT_START`. Touch `MapView.tsx:74,79,91`
  (fit CH instead of `flyTo(start)`), `StartSearch.tsx:9` `startLabel`, `findRoutes`,
  `shareLink`, the Find button.
- `Climb` gains `"off"` → `generate` sends no climb goal; `climbLabel`/`climbTargetM` handle it.
- `clearAll()` per "my calls" above.

### UI
- `ScrubField` component (`web/src/components/`): `role="spinbutton"`, pointer capture,
  log-scale drag, typing, keyboard, commit on release.
- Settings table component: one row per term (label · what it acts on · × field ·
  avoid/off/prefer word), Hills and Tech rows with their shape controls, Climb row
  (off / preset / m / max + strictness), "Clear all" in the header.
- `RouteCard.tsx`: score bars read-only; remove the Adjust panels and the then-dead
  `WeightSlider`/`HillsPanel`/`TechPanel`/`strength`/`weightAt` code in `Controls.tsx`/`state.ts`.
- Impeccable redesign via the `impeccable` skill against `PRODUCT.md`, `DESIGN.md`,
  `.impeccable/design-full.md`: square corners, tight padding, dense rows, whole app.
  Afterwards `impeccable-documenter` rewrites `DESIGN.md` from the built UI.

## Steps (each: failing test → implement → pass → `jj commit`)

1. Server signed/wide weights + validate tests (`tests/cost.rs`).
1b. Server penalties: `repeat`, `avoid_factor`, `prefer_factor` on `Profile`, `stem_m`,
   `max_overlap` on the generate body; tests: defaults unchanged, repeat ×1 lets a loop reuse
   a trail, out-of-range rejected, A* bound admissible at repeat ×0.01.
2. Must-pass: limit 10, Held-Karp order, musts-as-vias when the distance fits, summit spurs
   kept — tests in `tests/generate.rs` on the fixture graph (dead-end must stays on the loop;
   cost order beats bearing order; musts that fit the distance give a loop with no free vias).
3. Churfirsten check: `#[ignore]` test like `tests/bichelsee.rs`, CH graph
   (`data/graph.bin`), start Alt St. Johann, 7 summit coords from OSM `natural=peak`,
   all weights 0, hardest trail as the summit trails need (report which grade). Assert every
   summit within 50 m of the route, distance ≤ min × 1.1, little reuse outside summit spurs.
   Owner reviews the GPX.
4. Client: no default start.
5. Client: climb off, `clearAll`, url ranges, multiplier helper — vitest.
6. `ScrubField` + settings table; read-only score bars; delete dead slider code.
7. "Use X km" button on the too-far error. Plausible analytics: add
   `<script defer data-domain="ayoo.rkma.dev" src="https://plausible.rkma.dev/js/script.js"></script>`
   to `web/index.html` `<head>` (no test; check it loads in `just dev`).
8. Impeccable redesign pass, then DESIGN.md.
9. Docs: new D106+ in `docs/decisions.md` (each owner answer + my calls once approved),
   `docs/design.md` cost/generation/frontend sections; design spec at
   `docs/superpowers/specs/2026-11-22-power-settings-design.md`.

## Verification

- `just check` (fmt, clippy, cargo test, typecheck) and `cd web && npx vitest run`.
- `just smoke`.
- `cargo test --release --test churfirsten -- --ignored`, plus a GPX export for the owner.
- `just dev`, then in the browser: first open shows CH with no start; scrub a × field
  and watch live refresh; Clear all; build the Churfirsten loop by hand with 7 must-pass
  areas; check phone width (390 px) for the dense table.

## Risks

- Very small multipliers (×0.001) flatten the A* bound → slower generation; measure on CH.
- Held-Karp needs ~(n+1)² legs (121 at 10 musts); measure, cap n if slow.
- Churfirsten summits may be T4+; with the T3 default some peaks are unreachable (the
  error says so), so the user must raise the hardest-trail limit.
