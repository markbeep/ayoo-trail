import type { LatLon, Penalties, Route, Weights } from "./routes/types";
import { zoneMode, type Zone } from "./routes/zones";
import type { Stop, TravelOrigin } from "./routes/travel";

export interface Health {
  status: string;
  graph_date: string | null;
  bbox: { min_lat: number; min_lon: number; max_lat: number; max_lon: number } | null;
  /** The server has 360° view scores (D100). */
  view: boolean;
}

export class ApiError extends Error {}

async function call<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(path, init);
  const body = await res.json().catch(() => null);
  if (!res.ok) throw new ApiError(body?.error ?? res.statusText);
  return body as T;
}

export const getHealth = () => call<Health>("/api/health");

/** Zones as server exclusions (D90, D97). */
const exclude = (zones: readonly Zone[]) => zones.map((z) => ({ lat: z.lat, lon: z.lon, radius_m: z.radius_m, mode: zoneMode(z) }));

/** `climb_target_m` adds the Elevation bar and counts it in the score (D63). */
export const routeVia = (
  waypoints: LatLon[], weights: Weights, max_sac: number, climb_target_m: number | null, zones: readonly Zone[], penalties: Penalties,
) =>
  call<Route>("/api/route", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ waypoints, profile: { weights, max_sac, exclude: exclude(zones), ...penalties }, climb_target_m }),
  });

/** `target_m: null` asks for as much climb as possible (D52). */
export interface ClimbGoal { target_m: number | null; strictness: number }

export const generate = (
  start: LatLon, distance_m: number, climb: ClimbGoal | null, weights: Weights, max_sac: number, zones: readonly Zone[], penalties: Penalties,
) =>
  call<{ routes: Route[] }>("/api/generate", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ start, distance_m, climb, profile: { weights, max_sac, exclude: exclude(zones), ...penalties } }),
  }).then((r) => r.routes);

/** Per travel origin, the places it reaches within its budget (ADR 0005). */
export const travel = (origins: readonly TravelOrigin[]) =>
  call<{ origins: Stop[][] }>("/api/travel", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ origins }),
  });
