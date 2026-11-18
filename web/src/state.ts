// App state as Solid signals. One module so components share a single source.

import { createMemo, createRoot, createSignal } from "solid-js";
import type { BaseMapId } from "./map/basemaps";
import { DEFAULT_BASE_MAP } from "./map/basemaps";
import type { BreakdownKey, LatLon, Route, Weights } from "./routes/types";
import { DEFAULT_START, mockRoutes } from "./routes/mock";

export type Hilliness = "gentle" | "hilly" | "mountain";
export const HILLINESS: Record<Hilliness, { label: string; mPerKm: number }> = {
  gentle: { label: "Gentle", mPerKm: 30 },
  hilly: { label: "Hilly", mPerKm: 60 },
  mountain: { label: "Mountain", mPerKm: 90 },
};
export const DISTANCE_PRESETS_KM = [10, 15, 21] as const;

export const DEFAULT_WEIGHTS: Weights = { slope: 1, surface: 1, tech: 1, official: 1, forest: 1, urban: 1 };

/** How each score term maps to a weight, and how it's asked in plain language. */
export const FACTORS: Record<BreakdownKey, {
  label: string;
  question: string;
  weight: keyof Weights;
  choices: { label: string; value: number }[];
}> = {
  official: {
    label: "Marked trails", weight: "official",
    question: "How much should marked hiking trails count?",
    choices: [{ label: "A little", value: 0.5 }, { label: "Normal", value: 1 }, { label: "A lot", value: 2 }],
  },
  surface: {
    label: "Surface", weight: "surface",
    question: "How much should asphalt and paved paths be avoided?",
    choices: [{ label: "Fine", value: 0.5 }, { label: "Avoid", value: 1 }, { label: "Strongly avoid", value: 2 }],
  },
  slope: {
    label: "Gentle descents", weight: "slope",
    question: "How much should steep downhills count against a loop?",
    choices: [{ label: "Fine", value: 0.5 }, { label: "Avoid", value: 1 }, { label: "Strongly avoid", value: 2 }],
  },
  // Wording is a placeholder under open question Q23.
  tech: {
    label: "Technical terrain", weight: "tech",
    question: "How much should technical trails count against a loop?",
    choices: [{ label: "Fine", value: 0.5 }, { label: "Avoid", value: 1 }, { label: "Strongly avoid", value: 2 }],
  },
  forest: {
    label: "Forest", weight: "forest",
    question: "How much do you want to run through forest?",
    choices: [{ label: "No preference", value: 0 }, { label: "Some", value: 1 }, { label: "A lot", value: 2 }],
  },
  urban: {
    label: "Away from towns", weight: "urban",
    question: "How much should villages and towns be avoided?",
    choices: [{ label: "Fine", value: 0.5 }, { label: "Avoid", value: 1 }, { label: "Strongly avoid", value: 2 }],
  },
};

const STORE_KEY = "trail-route:v1";
interface Stored { start?: LatLon; baseMap?: BaseMapId }
function load(): Stored {
  try { return JSON.parse(localStorage.getItem(STORE_KEY) ?? "{}") as Stored; } catch { return {}; }
}
function save(patch: Stored) {
  try { localStorage.setItem(STORE_KEY, JSON.stringify({ ...load(), ...patch })); } catch { /* storage unavailable */ }
}
const stored = load();

export const [start, setStartRaw] = createSignal<LatLon>(stored.start ?? DEFAULT_START);
export const [distanceKm, setDistanceKm] = createSignal(15);
export const [hilliness, setHilliness] = createSignal<Hilliness>("hilly");
export const [weights, setWeights] = createSignal<Weights>({ ...DEFAULT_WEIGHTS });
export const [baseMap, setBaseMapRaw] = createSignal<BaseMapId>(stored.baseMap ?? DEFAULT_BASE_MAP);
export const [routes, setRoutes] = createSignal<Route[]>([]);
export const [selectedId, setSelectedId] = createSignal<string | null>(null);
export const [status, setStatus] = createSignal<"idle" | "loading" | "error">("idle");
export const [queryOpen, setQueryOpen] = createSignal(true);
export const [pickingStart, setPickingStart] = createSignal(false);
/** Settings changed since the routes were generated. */
export const [stale, setStale] = createSignal(false);

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
export function setWeight(k: keyof Weights, v: number) {
  setWeights((w) => ({ ...w, [k]: v }));
  if (routes().length) setStale(true);
}

/** Generate loops. Uses synthetic routes until `/api/generate` exists. */
export async function findRoutes() {
  setStatus("loading");
  setPickingStart(false);
  await new Promise((r) => setTimeout(r, 450));
  const rs = mockRoutes(start(), distanceKm() * 1000, HILLINESS[hilliness()].mPerKm * distanceKm());
  setRoutes(rs);
  setSelectedId(rs[0]?.id ?? null);
  setStale(false);
  setStatus("idle");
  setQueryOpen(false);
}
