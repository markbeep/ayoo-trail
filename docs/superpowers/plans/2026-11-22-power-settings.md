# Power-user settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Typed avoid/prefer multipliers and user-set penalties in one dense settings table, a clear-all, no default start, must-pass loops good enough for a 7-peak Churfirsten loop, and a denser UI.

**Architecture:** The server keeps D51 exponent weights, only widened and signed; penalties become `Profile` fields with today's constants as serde defaults, so old clients and links are unchanged. The generator orders must-pass points by route cost, returns the must-only loop when it fits, and keeps summit spurs. The client converts exponents to multipliers in a pure module, edits them with a scrub field, and shows everything in one settings card.

**Tech Stack:** Rust (axum, rayon), SolidJS 2 + MapLibre, Node's built-in test runner for pure client modules.

**Spec:** `docs/superpowers/specs/2026-11-22-power-settings-design.md` (decisions D106–D112 in `docs/decisions.md`).

## Global Constraints

- Version control is jj only: `jj commit -m "..."`. No git commands.
- Every task: failing test → implement → pass → commit. `just check` must pass at each commit (fmt, clippy `-D warnings`, cargo test, typecheck).
- Client unit tests are pure modules run with `node --test web/src/<file>.test.ts` (no state/solid imports in tested modules, as in `web/src/url.ts`).
- No new dependencies.
- Defaults must keep today's loops: weights as `DEFAULT_WEIGHTS`, repeat 5, avoid ×5, prefer ×0.5, stem 500 m, overlap 0.5.
- Multiplier range ×0.001–×1000; hills/tech × ≥ 1. Repeat ×0.01–×1000; avoid areas ×1–×1000; prefer areas ×0.001–×1; stem 0–5000 m; overlap 0–100 %.
- Must-pass limit 10 (server and client).
- Plan deviation from the spec table: `stem_m` and `max_overlap` live on `Profile` too (one serde-defaulted struct, no new request fields); `/api/route` ignores them.

## Review Focus

1. Old share links and stored state (no `n=` param, `w=` with old-range values) must still open with defaults. → Task 5 test.
2. Typed garbage in a × field ("", "0", "-3", "abc", "1e9") must clamp or be ignored, never send NaN/out-of-range to the server. → Task 5 test on `parseMultiplier`.
3. One must-pass area (n = 1) and none (n = 0) must behave as before after the Held-Karp change. → Task 3 keeps `every_loop_passes_the_must_pass_areas` green and adds an n = 1 assertion.
4. Repeat below ×1 must keep A* exact (bound admissible in both `route::route` and `Region::leg`). → Task 2 test.
5. Clear all while loops are shown must remove the areas from the map and regenerate once. → Task 7 manual check (no UI test harness).

---

### Task 1: Signed, wide weights (server)

**Files:**
- Modify: `src/cost.rs` (`MAX_WEIGHT`, `Profile::validate`, error text)
- Test: `tests/cost.rs`

**Interfaces:**
- Produces: `cost::MAX_WEIGHT: f32 = 100.0`. Valid: surface, official, forest, urban, traffic, view ∈ [−100, 100]; slope, tech ∈ [0, 100]; uphill, downhill, technical ∈ [−1, 1]; climb_pressure ∈ [0, 1].

- [ ] **Step 1: Failing tests** in `tests/cost.rs`:
  - `negative_weights_flip_a_preference`: on a one-edge graph with `forest_pct = 100`, `edge_cost` with `forest: -1.0` is `> ` the cost with `forest: 0.0`, and with `forest: 1.0` is `<` it.
  - `weights_validate_in_the_wide_ranges`: `forest: -100.0` ok, `forest: 100.0` ok, `forest: 100.5` → `WeightOutOfRange`, `slope: -0.1` → err, `tech: -1.0` → err, `forest: f32::NAN` → err.
  - `min_cost_per_m_stays_admissible_for_negative_weights`: reuse the loop in `min_cost_per_m_stays_admissible_for_every_slope_preference` (tests/cost.rs:113) with each simple weight at −50 and 50; every `c.per_m >= p.min_cost_per_m()`.
- [ ] **Step 2:** `cargo test --test cost` → the range tests FAIL.
- [ ] **Step 3:** In `validate`, split `ws` into `signed = [surface, official, forest, urban, traffic, view]` checked against `-MAX_WEIGHT..=MAX_WEIGHT` and `[slope, tech]` against `0.0..=MAX_WEIGHT`. Update the `#[error]` text to "terms −100–100 (slope, tech 0–100), uphill/downhill/technical −1–1, climb_pressure 0–1".
- [ ] **Step 4:** `cargo test` → PASS.
- [ ] **Step 5:** `jj commit -m "feat(cost): signed weights up to ±100 (D106)"`

### Task 2: User-set penalties on `Profile`

**Files:**
- Modify: `src/cost.rs` (`Profile`, `Default`, `validate`, `zone_factor`, `min_cost_per_m`, `ProfileError`), `src/route.rs:100-120` (`route` uses `p.repeat`), `src/region.rs:378-440` (`leg` uses `p.repeat`; landmark terms scaled), `src/generate.rs` (`STEM_M` → `p.stem_m`, `MAX_OVERLAP` → `p.max_overlap`), `src/server.rs:109` (`check_profile` maps the new error)
- Test: `tests/cost.rs`, `tests/route.rs`, `tests/generate.rs`

**Interfaces:**
- Produces on `Profile` (all `f64`, serde default = today's constant):
  `repeat` (default `route::REPEAT_PENALTY` = 5.0), `avoid_factor` (`EXCLUSION_PENALTY` = 5.0), `prefer_factor` (`PREFER_FACTOR` = 0.5), `stem_m` (500.0), `max_overlap` (0.5).
- Produces `ProfileError::PenaltyOutOfRange`, message "repeat 0.01-1000, avoid_factor 1-1000, prefer_factor 0.001-1, stem_m 0-5000, max_overlap 0-1"; server text "penalty out of range".
- The constants stay as the defaults; nothing else reads them after this task.

- [ ] **Step 1: Failing tests**
  - `tests/cost.rs` `penalties_default_to_todays_constants`: `Profile::default()` has 5.0 / 5.0 / 0.5 / 500.0 / 0.5; `serde_json::from_str::<Profile>("{}")` equals `Profile::default()`.
  - `tests/cost.rs` `penalties_validate`: each field just outside its range → `PenaltyOutOfRange`; each bound itself ok; NaN rejected.
  - `tests/cost.rs` `area_factors_come_from_the_profile`: the existing soft-avoid test (tests/cost.rs:241) with `avoid_factor: 20.0` gives `base.cost * 20.0`; the prefer test (tests/cost.rs:506) with `prefer_factor: 0.1` gives `base.cost * 0.1` and `per_m >= min_cost_per_m()`.
  - `tests/route.rs` `repeat_below_one_is_exact`: on a small grid, mark a detour's edges in `Visited`, route with `repeat: 0.01`; the result equals a brute-force Dijkstra over the same costs (or: its cost ≤ the cost of the direct path, and it uses the visited edges). Same with `repeat: 1.0` gives the same path as an empty `Visited`.
  - `tests/generate.rs` `zero_overlap_keeps_one_loop_per_shape`: `max_overlap: 0.0` returns loops sharing no edge pairwise; `stem_m: 0.0` still returns loops.
- [ ] **Step 2:** `cargo test` → new tests FAIL (unknown fields).
- [ ] **Step 3:** Implement:
  - `zone_factor`: `p.avoid_factor` / `p.prefer_factor` instead of the constants.
  - `min_cost_per_m`: `× p.prefer_factor` when a prefer area exists, and `× p.repeat.min(1.0)`.
  - `route::route` `full`: `p.repeat` instead of `REPEAT_PENALTY`.
  - `Region::leg`: `pen` uses `p.repeat`; in `h`, multiply the two landmark terms (`f_end - tr.from[v]`, `tr.to[v] - t_end`) by `p.repeat.min(1.0)` (they assume penalties only raise costs; one-line comment saying so).
  - `generate.rs`: `away_from_start` reads `p.stem_m`; the overlap check reads `req.profile.max_overlap`.
- [ ] **Step 4:** `cargo test` → PASS; `just smoke` → passes.
- [ ] **Step 5:** `jj commit -m "feat(cost): user-set repeat, area, stem and overlap penalties (D109)"`

### Task 3: Must-pass loops: limit 10, cost order, must-only loop, summit spurs

**Files:**
- Modify: `src/cost.rs:27` (`MAX_MUST = 10`), `src/generate.rs` (`must_passes`, `with_musts`, `route_loop`, `generate`), `src/route.rs` (`route_legs_with`, `drop_backtracks`, `route_legs`)
- Test: `tests/generate.rs`, `tests/route.rs:456`

**Interfaces:**
- Produces `pub fn drop_backtracks(g: &Graph, r: Route, keep: &[Snap]) -> Route`: never cancels a turnaround at a point equal to a kept snap (`st.edge == k.edge && st.from_t == k.t`). Existing caller `tests/route.rs:456` passes `&[]`.
- Produces `route_legs_with(g, snaps, visited, mark, leg, hold: impl Fn(usize) -> bool)`: when `hold(i)` is true, the edges of the leg ending at `snaps[i]` are inserted into `visited` only after leg `i` → `i+1` is routed (the way back down a summit pays no repeat penalty). `route_legs` passes `|_| false`.
- Internal `fn order_musts(g, region, p, start: Snap, musts: &[Snap]) -> Option<Vec<Snap>>`: pairwise leg costs with an empty `Visited` (`region.leg(..).or_else(|| route(..))`), exact Held-Karp cycle from `start`; None if any leg is unreachable.
- Internal `fn insert_vias(start: Snap, order: &[Snap], vias: &[Snap]) -> Vec<Snap>`: each free via inserted where it adds the least straight-line (haversine) detour. Replaces `with_musts`; `bearing` goes if unused.

- [ ] **Step 1: Failing tests** in `tests/generate.rs` (11×11 grid, step 100 m, points on nodes, `area(.., 50.0, Must)`):
  - `must_pass_order_follows_cost_not_bearing`: start `at(0,0)`, musts at `at(1000,100)`, `at(200,900)`, `at(900,900)`, `at(300,150)`. Compute the optimal tour length by brute force over the 4! orders with Manhattan distances in the test. `distance_m` = that length. Expect `Ok`, the best loop within 2 % of it, and `waypoints.len() == 4 + 2` (no free vias).
  - `fitting_must_passes_are_the_only_vias`: start `at(0,0)`, one must at `at(500,500)`, distance = 2000 m (its out-and-back on the grid fits): `waypoints.len() == 3`.
  - `a_summit_spur_stays_on_the_loop`: grid plus a 3-edge dead-end chain from `at(1000,500)` east to `at(1300,500)`; must at the chain's tip; every loop passes within 5 m of `at(1300,500)` and covers the chain both ways.
  - `ten_must_passes_are_allowed`: `Profile::validate` accepts 10 musts, rejects 11.
- [ ] **Step 2:** `cargo test --test generate` → FAIL.
- [ ] **Step 3:** Implement. In `generate`: after `must_passes`, `order_musts` (unreachable → `MustPassUnreachable`); `shortest = route_loop(.., &ordered)`; `> target·(1+TOLERANCE)` → `MustPassTooFar`; `≥ target·(1−TOLERANCE)` → return `vec![GeneratedRoute]` of `shortest` alone (waypoints start, musts, start; `distance_met` as usual). Otherwise seeds run with `insert_vias(start, &ordered, &free_vias)`. In `route_loop`, pass the must snaps as `keep` to `drop_backtracks` and `hold = |i| snaps[i] is a must`.
- [ ] **Step 4:** `cargo test` → PASS (all old must-pass tests too, including n = 1 in `every_loop_passes_the_must_pass_areas`).
- [ ] **Step 5:** `jj commit -m "feat(generate): up to 10 must-pass points in cost order, kept summit spurs (D110)"`

### Task 4: Churfirsten check on the CH graph

**Files:**
- Create: `tests/churfirsten.rs` (pattern: `tests/bichelsee.rs`)

**Interfaces:**
- Consumes: Tasks 1–3. Start Alt St. Johann; the 7 summits' coordinates taken from OSM `natural=peak` nodes (names Selun, Frümsel, Brisi, Zuestoll, Schibenstoll, Hinterrugg, Chäserrugg) in `data/raw`, hard-coded as constants with their OSM node ids in a comment.

- [ ] **Step 1: Write** `#[ignore = "needs a CH data/graph.bin"] fn loops_over_the_seven_churfirsten()`:
  - Profile: every weight 0 (uphill/downhill/technical 0), musts radius 100 m, `max_sac` the lowest of 3..=6 for which `generate` doesn't return `MustPassUnreachable` (print it).
  - Call with 5 km to get `MustPassTooFar { min_m }`, then with `distance_m = min_m`.
  - Assert: every summit within 50 m of `route_geometry`; `distance_m <= min_m * 1.1`; meters on edges covered more than once, outside 1.5 km of any summit and the start, `< 10 %` of the distance.
  - Write `target/churfirsten.gpx` (trkpts from `route_geometry`) and print km, climb, max_sac.
- [ ] **Step 2:** `cargo test --release --test churfirsten -- --ignored --nocapture`. If it fails, debug with superpowers:systematic-debugging before changing Task 3 code; record findings.
- [ ] **Step 3:** `jj commit -m "test: Churfirsten 7-peak loop on the CH graph (D110)"`. Tell the owner the GPX path and the grade needed.

### Task 5: Client pure modules: multipliers, scrub, share links, climb off, penalties

**Files:**
- Create: `web/src/routes/multipliers.ts`, `web/src/routes/multipliers.test.ts`
- Modify: `web/src/url.ts`, `web/src/url.test.ts`, `web/src/routes/types.ts` (`Penalties`), `web/src/routes/zones.ts` (`MAX_MUST = 10`)

**Interfaces:**
- Produces in `multipliers.ts`:
  - `type Term = "surface" | "official" | "forest" | "urban" | "traffic" | "view" | "slope" | "tech"`
  - `KEY: Record<Term, number>` = surface 2.0, official 0.7, forest 0.9, urban 2.0, traffic 2.0, view 0.7, slope 2.5, tech 1.4 (comment: copied from `src/cost.rs` tables; slope = gentle steepest-descent band, tech = T4+ in the avoid row).
  - `multiplierOf(t: Term, w: number): number` = `KEY[t] ** w`.
  - `weightOf(t: Term, m: number): number` = `ln m / ln KEY[t]`, m clamped to [0.001, 1000] ([1, 1000] for slope/tech), rounded to 0.01.
  - `parseMultiplier(s: string): number | null`: accepts "2", "×2", "x0.01", "1e2"; returns null for empty, ≤ 0, non-finite.
  - `scrub(v: number, dxPx: number, o: { scale: "log" | "linear"; step: number; min: number; max: number; fast?: boolean }): number`: log → `v · 1.01^(dx·k)`, linear → `v + dx·step·k`, k = 10 when fast; clamped.
  - `word(t: Term, w: number): "avoid" | "off" | "prefer"` (multiplier > 1 avoid, < 1 prefer; |w| < 0.005 off).
- Produces in `types.ts`: `interface Penalties { repeat: number; avoid_factor: number; prefer_factor: number; stem_m: number; max_overlap: number }`.
- Produces in `url.ts`: `Share.penalties: Penalties`, `ShareClimb` gains `"off"` (`c=off`), new `n=repeat,avoid,prefer,stem,overlap` (omitted when all defaults; absent → defaults), `w=` ranges from Task 1, `z=` limit unchanged (32).

- [ ] **Step 1: Failing tests**
  - `multipliers.test.ts`: `multiplierOf("surface", 1) === 2`; `weightOf("forest", 0.01)` ≈ 43.71; `weightOf("surface", 1e9)` = `weightOf("surface", 1000)`; `weightOf("slope", 0.5) === 0`; round trip `multiplierOf(t, weightOf(t, 100))` within 1 % of 100 for every term; `parseMultiplier` cases from Review Focus 2 (`""`, `"0"`, `"-3"`, `"abc"` → null; `"x0.01"` → 0.01; `"1e2"` → 100); `scrub(2, 100, log…)` ≈ 2·1.01¹⁰⁰ and clamps at max; `scrub(500, 10, linear step 10)` = 600; `word("forest", 1) === "prefer"`, `word("surface", 1) === "avoid"`, `word(t, 0) === "off"`.
  - `url.test.ts`: round trip with `forest: -43.71`, `traffic: 6.64`, `climb: "off"`, non-default penalties; a link without `n=` decodes to default penalties; an old link (the existing `share` fixture encoded without `n=`) still round-trips; `n=` with repeat 0.001 → null.
- [ ] **Step 2:** `node --test web/src/routes/multipliers.test.ts web/src/url.test.ts` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command → PASS; `cd web && npm run typecheck` (state.ts will need the new `Share` field — pass `penalties: DEFAULT_PENALTIES` there for now).
- [ ] **Step 5:** `jj commit -m "feat(web): multiplier math, scrub math, share links carry penalties and climb off"`

### Task 6: Client state: no default start, climb off, penalties, clear all

**Files:**
- Modify: `web/src/state.ts`, `web/src/api.ts`, `web/src/components/MapView.tsx:74-91`, `web/src/components/StartSearch.tsx:9`, `web/src/components/QueryCard.tsx`

**Interfaces:**
- Consumes: Task 5 `Penalties`, `ShareClimb`.
- Produces in `state.ts`:
  - `start: Accessor<LatLon | null>` from `stored.start ?? null`; `DEFAULT_START` deleted; `startName` default `null`.
  - `Climb = { preset } | { m } | "max" | "off"`; `climbGoal(): ClimbGoal | null` (null for "off"); `climbTargetM()` returns null for "off" and "max" — callers that meant "max" check `climb() === "max"`.
  - `DEFAULT_PENALTIES: Penalties` (5, 5, 0.5, 500, 0.5); `penalties` signal; `setPenalty(k: keyof Penalties, v: number)` (calls `settingsChanged`).
  - `clearAll()`: weights all 0 (uphill/downhill/technical 0), climb "off", zones `[]` (saved), maxSac 3, penalties `DEFAULT_PENALTIES`, then one `settingsChanged()`.
- Produces in `api.ts`: `generate(start, distance_m, climb: ClimbGoal | null, weights, max_sac, zones, penalties)` and `routeVia(waypoints, weights, max_sac, climb_target_m, zones, penalties)`; profile body spreads `penalties`.

- [ ] **Step 1:** No pure unit to test; typecheck is the gate. Write the code.
- [ ] **Step 2:** MapView: `start` null → `getHealth().then(h => h.bbox && c.fitBounds(h.bbox, props.padding()))`, else `flyTo(start, 12)`; `setStart(null)` clears the marker. `startLabel()` → "Pick a start" when null. Find button `disabled` when `!start()`; `findRoutes` returns early without a start; `shareLink` uses `start()!` (only reachable with loops).
- [ ] **Step 3:** `cd web && npm run typecheck` → passes; `just dev`: a fresh browser profile (empty localStorage) opens on Switzerland with no marker and Find disabled; pick a start → Find works.
- [ ] **Step 4:** `jj commit -m "feat(web): no default start, climb off, penalties and clear all in state (D108, D111)"`

### Task 7: Scrub field, settings card, read-only score bars

**Files:**
- Create: `web/src/components/ScrubField.tsx`, `web/src/components/SettingsCard.tsx`
- Modify: `web/src/App.tsx` (render `<SettingsCard />` after `<QueryCard />`, always), `web/src/components/QueryCard.tsx` (drop `ClimbControl`; start, distance, Find only), `web/src/components/RouteCard.tsx:105-130` (bars not buttons; label, track, value, `b.detail` as muted text; no Adjust)
- Delete: `web/src/components/Controls.tsx`; `strength`, `weightAt`, `positionOf`, `FACTORS[].question` in `state.ts` once unused

**Interfaces:**
- Consumes: Task 5 (`multiplierOf`, `weightOf`, `parseMultiplier`, `scrub`, `word`), Task 6 setters.
- Produces `ScrubField(props: { value: number; label: string; format: (v: number) => string; scale: "log" | "linear"; step: number; min: number; max: number; parse?: (s: string) => number | null; onCommit: (v: number) => void })`: `role="spinbutton"`, `aria-valuenow/min/max/text`; pointer down + horizontal move > 3 px = scrub with pointer capture and `cursor: ew-resize`, shows the live value, commits once on pointer up; Shift = fast; a click without movement turns it into a text input (Enter/blur commits via `parse`, Escape cancels, invalid input reverts); ArrowLeft/Right step once (Shift fast) and commit.
- `SettingsCard` rows (label · acts on · field · word):
  Surface "asphalt", Marked trails "marked", Forest "full forest", Towns "in town", Busy roads "busy road", Views "wide view" (log ScrubField over the multiplier, ×0.001–×1000, writes `setWeight(k, weightOf(k, m))`);
  Hills: Uphill and Downhill linear ScrubField −1…1 step 0.05 (format "−1 gentle … 0 any … +1 steep"), strength × (≥ 1) on `slope`;
  Technical: T1–T6 segmented `maxSac`, avoid…seek linear −1…1 on `technical`, strength × (≥ 1) on `tech`;
  Climb: Off/Gentle/Hilly/Mountain/Max segmented, meters linear field (10 m/px, step `CLIMB_STEP_M`), strictness linear 0–1;
  Penalties: Repeat ×0.01–×1000 (log), Avoid areas ×1–×1000, Prefer areas ×0.001–×1, Start stem 0–5000 m (linear 10 m/px), Overlap 0–100 % (linear).
  Header: "Settings" + "Clear all" (`clearAll`). The View row only shows when health reports `view.bin` (as the bar does now).

- [ ] **Step 1:** Build `ScrubField`, then `SettingsCard`, then rewire `QueryCard`/`RouteCard`, then delete the dead code.
- [ ] **Step 2:** `cd web && npm run typecheck` and `node --test web/src/*.test.ts web/src/routes/*.test.ts` → pass.
- [ ] **Step 3:** `just dev`, manual: scrub Forest to ×0.01 → one live refresh on release; type "x100" in Busy roads → ×100, "abc" reverts; keyboard arrows step; with loops shown, Clear all removes the areas from the map and refreshes once (Review Focus 5); score bars show values and details, no Adjust.
- [ ] **Step 4:** `jj commit -m "feat(web): dense settings card with scrub fields; read-only score bars (D106, D107)"`

### Task 8: "Use X km" and analytics

**Files:**
- Modify: `web/src/components/QueryCard.tsx` (error block), `web/index.html` (`<head>`)

**Interfaces:**
- Consumes: the server message "must-pass areas need at least N km" (`src/server.rs` `MustPassTooFar`).

- [ ] **Step 1:** In the error block, `const km = /need at least (\d+) km/.exec(m())?.[1]` (comment: matches `MustPassTooFar`'s message in server.rs); when set, a `link-btn` "Use {km} km" → `setDistanceKm(Number(km)); void findRoutes()`. Raise the custom distance input max if the parsed km exceeds 60 (cap at the server's 80).
- [ ] **Step 2:** Add `<script defer data-domain="ayoo.rkma.dev" src="https://plausible.rkma.dev/js/script.js"></script>` to `web/index.html` `<head>`.
- [ ] **Step 3:** `npm run typecheck`; `just dev`: 7 must-pass areas at 10 km show the button, clicking it finds the loop; the network tab shows `script.js` requested.
- [ ] **Step 4:** `jj commit -m "feat(web): use the must-pass minimum distance in one click; Plausible analytics (D110, D112)"`

### Task 9: Dense redesign (impeccable)

**Files:**
- Modify: `web/src/styles.css` and component markup as the redesign needs; `DESIGN.md` afterwards

- [ ] **Step 1:** Invoke the `impeccable` skill for a redesign of the whole app against `PRODUCT.md`, `DESIGN.md`, `.impeccable/design-full.md`, with the owner's direction: much less rounding (square or ≤ 2 px corners), much less padding, dense rows, more power-user friendly, less generic "AI" look. The settings card from Task 7 is the reference surface.
- [ ] **Step 2:** Check at 390 px and desktop widths: no horizontal scroll in the panel, scrub fields usable by touch (≥ 32 px tall hit area), sheet snaps still work.
- [ ] **Step 3:** Run the `impeccable-finish-reviewer` agent; fix its material findings.
- [ ] **Step 4:** `jj commit -m "style(web): dense, square redesign (D112)"`
- [ ] **Step 5:** Run the `impeccable-documenter` agent to rewrite `DESIGN.md` from the built UI; `jj commit -m "docs: DESIGN.md from the dense redesign"`.

### Task 10: Docs

**Files:**
- Modify: `docs/design.md` (cost model: signed weights and penalties; generation: must-pass order, must-only loop, spurs; frontend: settings card, no default start), `docs/brief.md` status line and Controls backlog row ("Weight sliders" → multipliers), `AGENTS.md` if a path it names changed (it names a missing `.impeccable/surfaces/...` brief: point it at `.impeccable/design-full.md`).

- [ ] **Step 1:** Update the docs; append a decision only if implementation settled something new (e.g. the summit grade Task 4 found).
- [ ] **Step 2:** `just check` → passes.
- [ ] **Step 3:** `jj commit -m "docs: design.md for power settings and must-pass loops"`

## Final verification

- `just check`, `node --test web/src/*.test.ts web/src/routes/*.test.ts`, `just smoke`.
- `cargo test --release --test churfirsten -- --ignored --nocapture` and `--test bichelsee` (default loops unchanged).
- Time generation on CH with Forest ×0.001 and with repeat ×0.01 (Risks in the spec); report numbers.
