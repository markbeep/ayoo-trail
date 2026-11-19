// App state as Solid signals. One module so components share a single source.

import { createMemo, createRoot, createSignal } from "solid-js";
import type { BaseMapId } from "./map/basemaps";
import { DEFAULT_BASE_MAP, DEFAULT_OVERLAYS } from "./map/basemaps";
import type { BreakdownKey, LatLon, Route, Weights } from "./routes/types";
import { ApiError, generate } from "./api";

export type NumericWeight = Exclude<keyof Weights, "uphill" | "downhill" | "climb_pressure">;

export type Hilliness = "gentle" | "hilly" | "mountain";
export const HILLINESS: Record<Hilliness, { label: string; mPerKm: number }> = {
  gentle: { label: "Gentle", mPerKm: 30 },
  hilly: { label: "Hilly", mPerKm: 60 },
  mountain: { label: "Mountain", mPerKm: 90 },
};
export const DISTANCE_PRESETS_KM = [10, 15, 21] as const;

export const DEFAULT_WEIGHTS: Weights = { slope: 1, uphill: 0, downhill: -1, surface: 1, tech: 1, official: 1, forest: 1, urban: 1 };

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

/** How each score term maps to a weight, and how it's asked in plain language. Wording under Q23. */
export const FACTORS: Record<BreakdownKey, { label: string; question: string; weight: NumericWeight }> = {
  official: { label: "Marked trails", weight: "official", question: "How much should marked hiking trails count?" },
  surface: { label: "Surface", weight: "surface", question: "How much should asphalt and paved paths be avoided?" },
  slope: { label: "Hills", weight: "slope", question: "What kind of climbs and descents do you like?" },
  tech: { label: "Technical terrain", weight: "tech", question: "How much should technical trails count against a loop?" },
  forest: { label: "Forest", weight: "forest", question: "How much do you want to run through forest?" },
  urban: { label: "Away from towns", weight: "urban", question: "How much should villages and towns be avoided?" },
};

/** Climb: a preset follows the distance, metres are fixed, "max" asks for as much as possible (D52). */
export type Climb = { preset: Hilliness } | { m: number } | "max";
export const CLIMB_STEP_M = 10;
export const climbMaxM = () => Math.max(1500, Math.ceil((150 * distanceKm()) / 100) * 100);

/** Wattwil: inside the demo graph (D34). */
export const DEFAULT_START: LatLon = { lat: 47.2985, lon: 9.095 };

const STORE_KEY = "navilau:v1";
interface Stored { start?: LatLon; baseMap?: BaseMapId; overlays?: string[] }
function load(): Stored {
  try { return JSON.parse(localStorage.getItem(STORE_KEY) ?? "{}") as Stored; } catch { return {}; }
}
function save(patch: Stored) {
  try { localStorage.setItem(STORE_KEY, JSON.stringify({ ...load(), ...patch })); } catch { /* storage unavailable */ }
}
const stored = load();

export const [start, setStartRaw] = createSignal<LatLon>(stored.start ?? DEFAULT_START);
export const [distanceKm, setDistanceKm] = createSignal(15);
export const [climb, setClimbRaw] = createSignal<Climb>({ preset: "hilly" });
export const [strictness, setStrictnessRaw] = createSignal(0.5);
export const [weights, setWeights] = createSignal<Weights>({ ...DEFAULT_WEIGHTS });
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

export function setStart(p: LatLon) {
  setStartRaw(p);
  save({ start: p });
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
/** Target metres of climb, or null for "as much as possible". */
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
  liveTimer = setTimeout(() => void findRoutes({ keepOpen: true }), 250);
}
export function setHill(k: "uphill" | "downhill", v: number) {
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

let seq = 0;
/** Generate loops with `POST /api/generate`. A live refresh keeps the query card as it is and the same loop slot selected. */
export async function findRoutes(opts: { keepOpen?: boolean } = {}) {
  const mine = ++seq;
  const slot = Math.max(0, routes().findIndex((r) => r.id === selectedId()));
  if (opts.keepOpen) setRefreshing(true);
  else setStatus("loading");
  setErrorMsg(null);
  setPickingStart(false);
  try {
    const rs = await generate(start(), distanceKm() * 1000, { target_m: climbTargetM(), strictness: strictness() }, weights());
    if (mine !== seq) return; // a newer request superseded this one
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
