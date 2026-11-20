// App state as Solid signals. One module so components share a single source.

import { createMemo, createRoot, createSignal } from "solid-js";
import type { BaseMapId } from "./map/basemaps";
import { DEFAULT_BASE_MAP, DEFAULT_OVERLAYS } from "./map/basemaps";
import type { BreakdownKey, LatLon, Route, Weights } from "./routes/types";
import { ApiError, generate, routeVia } from "./api";
import { clampRadius, MAX_ZONES, ZONE_DEFAULT_M, type Zone } from "./routes/zones";

export type NumericWeight = Exclude<keyof Weights, "uphill" | "downhill" | "technical" | "climb_pressure">;

export type Hilliness = "gentle" | "hilly" | "mountain";
export const HILLINESS: Record<Hilliness, { label: string; mPerKm: number }> = {
  gentle: { label: "Gentle", mPerKm: 30 },
  hilly: { label: "Hilly", mPerKm: 60 },
  mountain: { label: "Mountain", mPerKm: 90 },
};
export const DISTANCE_PRESETS_KM = [10, 15, 21] as const;

export const DEFAULT_WEIGHTS: Weights = { slope: 1, uphill: 0, downhill: -1, surface: 1, tech: 1, technical: -1, official: 1, forest: 1, urban: 1, traffic: 1 };

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
};

/** Climb: a preset follows the distance, meters are fixed, "max" asks for as much as possible (D52). */
export type Climb = { preset: Hilliness } | { m: number } | "max";
export const CLIMB_STEP_M = 10;
export const climbMaxM = () => Math.max(1500, Math.ceil((150 * distanceKm()) / 100) * 100);

/** Wattwil: inside the demo graph (D34). */
export const DEFAULT_START: LatLon = { lat: 47.2985, lon: 9.095 };

const STORE_KEY = "trail-calc:v1";
interface Stored { start?: LatLon; startName?: string | null; baseMap?: BaseMapId; overlays?: string[]; zones?: Zone[] }
function load(): Stored {
  try { return JSON.parse(localStorage.getItem(STORE_KEY) ?? "{}") as Stored; } catch { return {}; }
}
function save(patch: Stored) {
  try { localStorage.setItem(STORE_KEY, JSON.stringify({ ...load(), ...patch })); } catch { /* storage unavailable */ }
}
const stored = load();

export const [start, setStartRaw] = createSignal<LatLon>(stored.start ?? DEFAULT_START);
/** Name of a searched start (D57); null when it was picked on the map. */
export const [startName, setStartName] = createSignal<string | null>(stored.start ? (stored.startName ?? null) : "Wattwil");
export const [distanceKm, setDistanceKm] = createSignal(15);
export const [climb, setClimbRaw] = createSignal<Climb>({ preset: "hilly" });
export const [strictness, setStrictnessRaw] = createSignal(0.8);
export const [weights, setWeights] = createSignal<Weights>({ ...DEFAULT_WEIGHTS });
/** Hardest SAC grade a loop may use, T1–T6 (D81). */
export const [maxSac, setMaxSacRaw] = createSignal(3);
/** Avoid areas (D90). */
export const [zones, setZonesRaw] = createSignal<readonly Zone[]>(Array.isArray(stored.zones) ? stored.zones.slice(0, MAX_ZONES) : []);
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
/** Target meters of climb, or null for "as much as possible". */
export function climbTargetM(): number | null {
  const c = climb();
  if (c === "max") return null;
  return "preset" in c ? HILLINESS[c.preset].mPerKm * distanceKm() : c.m;
}
export function climbLabel() {
  const c = climb();
  if (c === "max") return "Max climb";
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
export function setStrictness(v: number) {
  setStrictnessRaw(v);
  settingsChanged();
}
export function setZones(z: readonly Zone[]) {
  setZonesRaw(z);
  save({ zones: [...z] });
  settingsChanged();
}
export function addZone(p: LatLon) {
  if (zones().length < MAX_ZONES) setZones([...zones(), { ...p, radius_m: ZONE_DEFAULT_M }]);
}
export const moveZone = (i: number, p: LatLon) => setZones(zones().map((z, j) => (j === i ? { ...z, ...p } : z)));
export const resizeZone = (i: number, m: number) => setZones(zones().map((z, j) => (j === i ? { ...z, radius_m: clampRadius(m) } : z)));
export const removeZone = (i: number) => setZones(zones().filter((_, j) => j !== i));

/** Edited loops (D54): earlier versions per loop id for Undo; the first is the loop as generated. */
export const [history, setHistory] = createSignal<Record<string, Route[]>>({});
export const isEdited = (id: string) => (history()[id]?.length ?? 0) > 0;
export const [editError, setEditError] = createSignal<string | null>(null);
/** Color the selected route by cost per meter (D54). */
export const [costView, setCostView] = createSignal(false);
/** `/api/route` takes at most 20 waypoints. */
export const MAX_WAYPOINTS = 20;

const withinDistance = (m: number) => Math.abs(m - distanceKm() * 1000) / (distanceKm() * 1000) <= 0.1;
/** Re-route a loop through its waypoints under the current weights and its own climb pressure. */
async function reroute(r: Route, wps: LatLon[], id: string): Promise<Route> {
  const next = await routeVia(wps, { ...weights(), climb_pressure: r.climb_pressure }, maxSac(), climbTargetM(), zones());
  return { ...next, id, climb_pressure: r.climb_pressure, distance_met: withinDistance(next.distance_m) };
}
const replaceRoute = (id: string, r: Route) => setRoutes((rs) => rs.map((x) => (x.id === id ? r : x)));

let editSeq = 0;
/** Replace a loop's waypoints and re-route it; the old version goes on its undo stack. */
export async function editWaypoints(id: string, wps: LatLon[]) {
  const r = routes().find((x) => x.id === id);
  if (!r || wps.length > MAX_WAYPOINTS) return;
  const mine = ++editSeq;
  setRefreshing(true);
  try {
    const next = await reroute(r, wps, id);
    if (mine !== editSeq) return;
    setHistory((h) => ({ ...h, [id]: [...(h[id] ?? []), r] }));
    replaceRoute(id, next);
    setEditError(null);
  } catch (e) {
    if (mine !== editSeq) return;
    setEditError(e instanceof ApiError ? `Couldn't route there: ${e.message}` : "Couldn't reach the route server.");
  }
  setRefreshing(false);
}
export function undoEdit(id: string) {
  const h = history()[id];
  if (!h?.length) return;
  replaceRoute(id, h[h.length - 1]!);
  setHistory((x) => ({ ...x, [id]: h.slice(0, -1) }));
  setEditError(null);
}
export function resetEdit(id: string) {
  const first = history()[id]?.[0];
  if (!first) return;
  replaceRoute(id, first);
  setHistory((x) => ({ ...x, [id]: [] }));
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
  const mine = ++seq;
  const slot = Math.max(0, routes().findIndex((r) => r.id === selectedId()));
  if (opts.keepOpen) setRefreshing(true);
  else setStatus("loading");
  setErrorMsg(null);
  setPickingStart(false);
  try {
    const rs = await generate(start(), distanceKm() * 1000, { target_m: climbTargetM(), strictness: strictness() }, weights(), maxSac(), zones());
    if (mine !== seq) return; // a newer request superseded this one
    if (opts.keepOpen) {
      // Edited loops keep their points and re-route under the new settings; Reset goes to the fresh loop.
      const h = history();
      const kept = await Promise.all(rs.map(async (fresh) => {
        const old = routes().find((x) => x.id === fresh.id);
        if (!old || !h[fresh.id]?.length) return fresh;
        return reroute(fresh, old.waypoints, fresh.id).catch(() => fresh);
      }));
      if (mine !== seq) return;
      setHistory((x) => Object.fromEntries(Object.entries(x).map(([id, v]) => {
        const fresh = rs.find((r) => r.id === id);
        return [id, fresh && v.length ? [fresh] : []];
      })));
      rs.splice(0, rs.length, ...kept);
    } else setHistory({});
    setRoutes(rs);
    setSelectedId(rs[opts.keepOpen ? Math.min(slot, rs.length - 1) : 0]?.id ?? null);
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
