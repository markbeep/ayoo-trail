// Dev-only (`?dev=route`): click waypoints, get the engine's route between them.
import { DEFAULT_PENALTIES } from "../url";
import { createEffect, createRoot, createSignal } from "solid-js";
import { getHealth, routeVia } from "../api";
import type { LatLon, Weights } from "../routes/types";
import { setRoutes, setSelectedId, setStale, tlmOnly, weights } from "../state";

export const DEV_ROUTE = new URLSearchParams(location.search).get("dev") === "route";

export const [devWaypoints, setDevWaypoints] = createSignal<LatLon[]>([]);
export const [devError, setDevError] = createSignal<string | null>(null);
export const [devBusy, setDevBusy] = createSignal(false);
export const [devNoGraph, setDevNoGraph] = createSignal(false);

/** Startup check: resolves to the graph bbox, or null (and flags the card) if no graph is loaded. */
export async function devHealth() {
  const h = await getHealth().catch(() => null);
  setDevNoGraph(!h?.bbox);
  return h?.bbox ?? null;
}

export const addWaypoint = (p: LatLon) => setDevWaypoints((w) => [...w, p]);
export const undoWaypoint = () => setDevWaypoints((w) => w.slice(0, -1));
export const clearWaypoints = () => {
  setDevWaypoints([]);
  setDevError(null);
};

let seq = 0;
let shown = false; // a dev route is currently displayed
const clearRoute = () => { shown = false; setRoutes([]); setSelectedId(null); setStale(false); };
async function reroute(points: LatLon[], w: Weights) {
  const mine = ++seq;
  if (points.length < 2) {
    setDevBusy(false);
    if (shown) clearRoute();
    setDevError(null);
    return;
  }
  setDevBusy(true);
  try {
    const route = await routeVia(points, w, 3, null, [], DEFAULT_PENALTIES, tlmOnly());
    if (mine !== seq) return; // a newer request superseded this one
    setRoutes([route]);
    setSelectedId(route.id);
    setStale(false);
    shown = true;
    setDevError(null);
  } catch (e) {
    if (mine !== seq) return;
    setDevError(e instanceof Error ? e.message : String(e));
  }
  setDevBusy(false);
}

if (DEV_ROUTE) createRoot(() => createEffect(() => [devWaypoints(), weights()] as const, ([p, w]) => void reroute(p, w)));
