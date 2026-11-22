// App state as Solid signals. One module so components share a single source.

import { createMemo, createRoot, createSignal, flush } from "solid-js";
import type { BaseMapId } from "./map/basemaps";
import { DEFAULT_BASE_MAP, DEFAULT_OVERLAYS } from "./map/basemaps";
import type { BreakdownKey, Checkpoint, LatLon, Penalties, Route, Weights } from "./routes/types";
import { ApiError, generate, routeVia, type ClimbGoal } from "./api";
import { clampRadius, MAX_ZONES, ZONE_DEFAULT_M, type Zone } from "./routes/zones";
import { decodeShare, DEFAULT_PENALTIES, encodeShare, MAX_CHECKPOINTS } from "./url";
import { nearestOnRoute } from "./routes/gpx";
import { copyId, mergeCopies } from "./routes/copies";

export type NumericWeight = Exclude<keyof Weights, "uphill" | "downhill" | "technical" | "climb_pressure">;

export type Hilliness = "gentle" | "hilly" | "mountain";
export const HILLINESS: Record<Hilliness, { label: string; mPerKm: number }> = {
  gentle: { label: "Gentle", mPerKm: 30 },
  hilly: { label: "Hilly", mPerKm: 60 },
  mountain: { label: "Mountain", mPerKm: 90 },
};
export const DISTANCE_PRESETS_KM = [10, 15, 21] as const;

export const DEFAULT_WEIGHTS: Weights = { slope: 1, uphill: 0, downhill: -1, surface: 1, tech: 1, technical: -1, official: 1, forest: 1, urban: 1, traffic: 1, view: 0 };

/** Slider position 0–1 ↔ weight exponent 0–4, so the default (1) sits mid-track (D51). */
export const MAX_WEIGHT = 4;
export const weightAt = (t: number) => MAX_WEIGHT * t * t;
export const positionOf = (w: number) => Math.sqrt(w / MAX_WEIGHT);
/** Plain-language strength for a weight. */
export function strength(w: number) {
  if (w === 0) return "Off";
  if (w < 0.5) return "A little";
  if (w < 1.6) return "Normal";
  if (w < 3) return "A lot";
  return "Above all";
}

/** How each score term maps to a weight, and how it's asked in plain language. Wording under Q23.
 * Elevation has no weight: its Adjust panel holds the climb controls (D63). */
export const FACTORS: Record<BreakdownKey, { label: string; question: string; weight?: NumericWeight }> = {
  climb: { label: "Elevation", question: "How much climb do you want, and how closely should loops match it?" },
  official: { label: "Marked trails", weight: "official", question: "How much should marked hiking trails count?" },
  surface: { label: "Surface", weight: "surface", question: "How much should asphalt and paved paths be avoided?" },
  slope: { label: "Gradient", weight: "slope", question: "How steep do you like your climbs and descents? This is about steepness, not how much you climb." },
  tech: { label: "Technical terrain", weight: "tech", question: "How hard may trails get, and do you want technical ones?" },
  forest: { label: "Forest", weight: "forest", question: "How much do you want to run through forest?" },
  urban: { label: "Away from towns", weight: "urban", question: "How much should villages and towns be avoided?" },
  traffic: { label: "Quiet roads", weight: "traffic", question: "How much should busy roads be avoided?" },
  view: { label: "Views", weight: "view", question: "How much do you want wide views from ridges and peaks?" },
};

/** Climb: a preset follows the distance, meters are fixed, "max" asks for as much as possible (D52). */
export type Climb = { preset: Hilliness } | { m: number } | "max" | "off";
export const CLIMB_STEP_M = 10;
export const climbMaxM = () => Math.max(1500, Math.ceil((150 * distanceKm()) / 100) * 100);

const STORE_KEY = "trail-calc:v1";
interface Stored { start?: LatLon; startName?: string | null; baseMap?: BaseMapId; overlays?: string[]; zones?: Zone[]; checkpoints?: Checkpoint[] }
function load(): Stored {
  try { return JSON.parse(localStorage.getItem(STORE_KEY) ?? "{}") as Stored; } catch { return {}; }
}
function save(patch: Stored) {
  try { localStorage.setItem(STORE_KEY, JSON.stringify({ ...load(), ...patch })); } catch { /* storage unavailable */ }
}
const stored = load();

/** No default start (D111): null until the user picks one; a stored start comes back. */
export const [start, setStartRaw] = createSignal<LatLon | null>(stored.start ?? null);
/** Name of a searched start (D57); null when it was picked on the map. */
export const [startName, setStartName] = createSignal<string | null>(stored.start ? (stored.startName ?? null) : null);
export const [distanceKm, setDistanceKm] = createSignal(15);
export const [climb, setClimbRaw] = createSignal<Climb>({ preset: "hilly" });
export const [strictness, setStrictnessRaw] = createSignal(0.8);
export const [weights, setWeights] = createSignal<Weights>({ ...DEFAULT_WEIGHTS });
/** Hardest SAC grade a loop may use, T1–T6 (D81). */
export const [maxSac, setMaxSacRaw] = createSignal(3);
/** Repeat, area, stem and overlap penalties (D109). */
export const [penalties, setPenaltiesRaw] = createSignal<Penalties>({ ...DEFAULT_PENALTIES });
/** Avoid areas (D90). */
export const [zones, setZonesRaw] = createSignal<readonly Zone[]>(Array.isArray(stored.zones) ? stored.zones.slice(0, MAX_ZONES) : []);
/** GPX checkpoints (D98): kept across loops, exported when near the loop. */
export const [checkpoints, setCheckpointsRaw] = createSignal<readonly Checkpoint[]>(
  Array.isArray(stored.checkpoints) ? stored.checkpoints.slice(0, MAX_CHECKPOINTS) : [],
);
export const [baseMap, setBaseMapRaw] = createSignal<BaseMapId>(stored.baseMap ?? DEFAULT_BASE_MAP);
export const [overlays, setOverlaysRaw] = createSignal<readonly string[]>(stored.overlays ?? DEFAULT_OVERLAYS);
export const [routes, setRoutes] = createSignal<Route[]>([]);
export const [selectedId, setSelectedId] = createSignal<string | null>(null);
export const [status, setStatus] = createSignal<"idle" | "loading" | "error">("idle");
/** A live refresh (D55) is in flight; unlike `status` it doesn't move the sheet. */
export const [refreshing, setRefreshing] = createSignal(false);
export const [errorMsg, setErrorMsg] = createSignal<string | null>(null);
export const [queryOpen, setQueryOpen] = createSignal(true);
export const [pickingStart, setPickingStart] = createSignal(false);
/** Settings changed since the routes were generated. */
export const [stale, setStale] = createSignal(false);

/** Index into the selected route's coordinates under the pointer (chart or map), or null. */
export const [hoverIndex, setHoverIndex] = createSignal<number | null>(null);

export const selected = createRoot(() => createMemo(() => routes().find((r) => r.id === selectedId()) ?? null));

export function setStart(p: LatLon, name: string | null = null) {
  setStartRaw(p);
  setStartName(name);
  save({ start: p, startName: name });
  if (routes().length) setStale(true);
}
export function setBaseMap(id: BaseMapId) {
  setBaseMapRaw(id);
  save({ baseMap: id });
}
export function toggleOverlay(id: string) {
  const next = overlays().includes(id) ? overlays().filter((o) => o !== id) : [...overlays(), id];
  setOverlaysRaw(next);
  save({ overlays: next });
}
/** The climb goal for `/api/generate`, or null without one ("off"). */
export function climbGoal(): ClimbGoal | null {
  return climb() === "off" ? null : { target_m: climbTargetM(), strictness: strictness() };
}
/** Target meters of climb, or null for "as much as possible" and "off". */
export function climbTargetM(): number | null {
  const c = climb();
  if (c === "max" || c === "off") return null;
  return "preset" in c ? HILLINESS[c.preset].mPerKm * distanceKm() : c.m;
}
export function climbLabel() {
  const c = climb();
  if (c === "max") return "Max climb";
  if (c === "off") return "Any climb";
  return "preset" in c ? HILLINESS[c.preset].label : `${c.m} m up`;
}

// Setting changes regenerate shortly after the user lets go, while routes are shown (D55).
let liveTimer: ReturnType<typeof setTimeout> | undefined;
function settingsChanged() {
  if (!routes().length) return;
  clearTimeout(liveTimer);
  setRefreshing(true); // fade the old numbers right away, not only once the request starts
  liveTimer = setTimeout(() => void findRoutes({ keepOpen: true }), 250);
}
export function setMaxSac(v: number) {
  setMaxSacRaw(v);
  settingsChanged();
}
export function setHill(k: "uphill" | "downhill" | "technical", v: number) {
  setWeights((w) => ({ ...w, [k]: v }));
  settingsChanged();
}
export function setWeight(k: NumericWeight, v: number) {
  setWeights((w) => ({ ...w, [k]: v }));
  settingsChanged();
}
export function setClimb(c: Climb) {
  setClimbRaw(c);
  settingsChanged();
}
export function setPenalty(k: keyof Penalties, v: number) {
  setPenaltiesRaw((p) => ({ ...p, [k]: v }));
  settingsChanged();
}
/** Clear all (D108): only the distance drives the route. */
export function clearAll() {
  setWeights({ slope: 0, uphill: 0, downhill: 0, surface: 0, tech: 0, technical: 0, official: 0, forest: 0, urban: 0, traffic: 0, view: 0 });
  setClimbRaw("off");
  setMaxSacRaw(3);
  setPenaltiesRaw({ ...DEFAULT_PENALTIES });
  setZonesRaw([]);
  save({ zones: [] });
  settingsChanged();
}
export function setStrictness(v: number) {
  setStrictnessRaw(v);
  settingsChanged();
}
export function setZones(z: readonly Zone[]) {
  setZonesRaw(z);
  save({ zones: [...z] });
  settingsChanged();
}
export function addZone(p: LatLon, kind: "avoid" | "prefer" = "avoid") {
  if (zones().length < MAX_ZONES) setZones([...zones(), { ...p, radius_m: ZONE_DEFAULT_M, kind }]);
}
export const toggleHard = (i: number) => setZones(zones().map((z, j) => (j === i ? { ...z, hard: !z.hard } : z)));
export const moveZone = (i: number, p: LatLon) => setZones(zones().map((z, j) => (j === i ? { ...z, ...p } : z)));
export const resizeZone = (i: number, m: number) => setZones(zones().map((z, j) => (j === i ? { ...z, radius_m: clampRadius(m) } : z)));
export const removeZone = (i: number) => setZones(zones().filter((_, j) => j !== i));

/** Checkpoints don't change the routing, so no regenerate. */
function setCheckpoints(c: readonly Checkpoint[]) {
  setCheckpointsRaw(c);
  save({ checkpoints: [...c] });
}
/** A checkpoint on the selected loop's track point nearest `p`. */
export function addCheckpoint(p: LatLon) {
  const r = selected();
  if (!r || checkpoints().length >= MAX_CHECKPOINTS) return;
  const [lon, lat] = r.geometry.coordinates[nearestOnRoute(r.geometry.coordinates, p).i]!;
  setCheckpoints([...checkpoints(), { lat, lon, name: `CP${checkpoints().length + 1}` }]);
}
export const renameCheckpoint = (i: number, name: string) =>
  setCheckpoints(checkpoints().map((c, j) => (j === i ? { ...c, name: name.slice(0, 40) } : c)));
export const removeCheckpoint = (i: number) => setCheckpoints(checkpoints().filter((_, j) => j !== i));

/** Edited copies (D101): earlier versions per copy id for Undo. */
export const [history, setHistory] = createSignal<Record<string, Route[]>>({});
export const [editError, setEditError] = createSignal<string | null>(null);
/** Color the selected route by cost per meter (D54). */
export const [costView, setCostView] = createSignal(false);
/** `/api/route` takes at most 20 waypoints. */
export const MAX_WAYPOINTS = 20;

const withinDistance = (m: number) => Math.abs(m - distanceKm() * 1000) / (distanceKm() * 1000) <= 0.1;
/** Re-route a loop through its waypoints under the current weights and its own climb pressure. */
async function reroute(r: Route, wps: LatLon[], id: string): Promise<Route> {
  const next = await routeVia(wps, { ...weights(), climb_pressure: r.climb_pressure }, maxSac(), climbTargetM(), zones(), penalties());
  return { ...next, id, copyOf: r.copyOf, climb_pressure: r.climb_pressure, distance_met: withinDistance(next.distance_m) };
}
const replaceRoute = (id: string, r: Route) => setRoutes((rs) => rs.map((x) => (x.id === id ? r : x)));

let editSeq = 0;
/** Edit a loop's waypoints (D101): a generated loop gets a new copy, a copy changes in place with Undo. */
export async function editWaypoints(id: string, wps: LatLon[]) {
  const r = routes().find((x) => x.id === id);
  if (!r || wps.length > MAX_WAYPOINTS) return;
  const mine = ++editSeq;
  setRefreshing(true);
  try {
    if (r.copyOf) {
      const next = await reroute(r, wps, id);
      if (mine !== editSeq) return;
      setHistory((h) => ({ ...h, [id]: [...(h[id] ?? []), r] }));
      replaceRoute(id, next);
    } else {
      const cid = copyId(id);
      const next = { ...(await reroute(r, wps, cid)), copyOf: id };
      if (mine !== editSeq) return;
      setRoutes((rs) => {
        // after the original and its earlier copies
        const i = rs.length - [...rs].reverse().findIndex((x) => x.id === id || x.copyOf === id);
        return [...rs.slice(0, i), next, ...rs.slice(i)];
      });
      setSelectedId(cid);
    }
    setEditError(null);
  } catch (e) {
    if (mine !== editSeq) return;
    setEditError(e instanceof ApiError ? `Couldn't route there: ${e.message}` : "Couldn't reach the route server.");
  }
  setRefreshing(false);
}
/** Undo the copy's last edit; undoing its first edit deletes the copy. */
export function undoEdit(id: string) {
  const h = history()[id];
  if (!h?.length) return deleteCopy(id);
  replaceRoute(id, h[h.length - 1]!);
  setHistory((x) => ({ ...x, [id]: h.slice(0, -1) }));
  setEditError(null);
}
/** Remove an edited copy and go back to the loop it was made from. */
export function deleteCopy(id: string) {
  const r = routes().find((x) => x.id === id);
  if (!r?.copyOf) return;
  setRoutes((rs) => rs.filter((x) => x.id !== id));
  setHistory(({ [id]: _, ...rest }) => rest);
  if (selectedId() === id) setSelectedId(routes().some((x) => x.id === r.copyOf) ? r.copyOf : (routes()[0]?.id ?? null));
  setEditError(null);
}

let seq = 0;
/** "Change" (or a query chip): drop the loops and any pending refresh, so nothing regenerates until "Find loops". */
export function editQuery() {
  seq++; // a request already in flight is ignored when it returns
  clearTimeout(liveTimer);
  setRoutes([]);
  setSelectedId(null);
  setHistory({});
  setEditError(null);
  setStale(false);
  setRefreshing(false);
  setStatus("idle");
  setQueryOpen(true);
}
/** Generate loops with `POST /api/generate`. A live refresh keeps the query card as it is and the same loop slot selected. */
export async function findRoutes(opts: { keepOpen?: boolean } = {}) {
  const from = start();
  if (!from) return;
  const mine = ++seq;
  const slot = Math.max(0, routes().findIndex((r) => r.id === selectedId()));
  if (opts.keepOpen) setRefreshing(true);
  else setStatus("loading");
  setErrorMsg(null);
  setPickingStart(false);
  try {
    const rs = await generate(from, distanceKm() * 1000, climbGoal(), weights(), maxSac(), zones(), penalties());
    if (mine !== seq) return; // a newer request superseded this one
    let all = rs;
    if (opts.keepOpen) {
      // Generated loops are re-picked; edited copies keep their points and re-route under the new settings (D101).
      const copies = await Promise.all(routes().filter((x) => x.copyOf).map((c) => reroute(c, c.waypoints, c.id).catch(() => c)));
      if (mine !== seq) return;
      all = mergeCopies(rs, copies);
    } else setHistory({});
    setRoutes(all);
    const keep = opts.keepOpen && all.some((r) => r.id === selectedId());
    setSelectedId(keep ? selectedId() : (all[opts.keepOpen ? Math.min(slot, all.length - 1) : 0]?.id ?? null));
    setStale(false);
    setStatus("idle");
    if (!opts.keepOpen) setQueryOpen(false);
  } catch (e) {
    if (mine !== seq) return;
    setErrorMsg(e instanceof ApiError ? e.message : "Couldn't reach the route server.");
    setStatus("error");
  }
  setRefreshing(false);
}

/** A link that reopens this query, the selected loop and its edits (D91). */
export function shareLink(): string {
  const r = selected();
  // A copy is shared as its original's slot plus the copy's points; opening the link recreates the copy.
  const i = Math.max(0, routes().findIndex((x) => x.id === (r?.copyOf ?? r?.id)));
  const hash = encodeShare({
    start: start()!, km: distanceKm(), climb: climb(), strictness: strictness(),
    weights: weights(), maxSac: maxSac(), zones: [...zones()], checkpoints: [...checkpoints()], loop: i,
    wps: r?.copyOf ? r.waypoints : null, penalties: penalties(),
  });
  return `${location.origin}${location.pathname}#${hash}`;
}

/** Open a share link from the URL hash, if there is one: apply it, generate, select and re-edit the loop. */
export async function openShared() {
  const s = decodeShare(location.hash);
  if (!s) return;
  window.history.replaceState(null, "", location.pathname + location.search);
  setStart(s.start); // no name in the link: the label shows the coordinates
  setDistanceKm(s.km);
  setClimbRaw(s.climb);
  setStrictnessRaw(s.strictness);
  setWeights({ ...DEFAULT_WEIGHTS, ...s.weights });
  setMaxSacRaw(s.maxSac);
  setPenaltiesRaw(s.penalties);
  setZonesRaw(s.zones);
  setCheckpointsRaw(s.checkpoints);
  flush(); // Solid 2.0 batches writes; findRoutes must read the shared values, not the stored ones
  await findRoutes();
  const id = routes()[s.loop]?.id;
  if (!id) return;
  setSelectedId(id);
  flush();
  if (s.wps) await editWaypoints(id, s.wps);
}
