import type { LatLon, Route, Weights } from "./routes/types";

export interface Health {
  status: string;
  graph_date: string | null;
  bbox: { min_lat: number; min_lon: number; max_lat: number; max_lon: number } | null;
}

export class ApiError extends Error {}

async function call<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(path, init);
  const body = await res.json().catch(() => null);
  if (!res.ok) throw new ApiError(body?.error ?? res.statusText);
  return body as T;
}

export const getHealth = () => call<Health>("/api/health");

/** `climb_target_m` adds the Elevation bar and counts it in the score (D63). */
export const routeVia = (waypoints: LatLon[], weights: Weights, climb_target_m: number | null = null) =>
  call<Route>("/api/route", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ waypoints, profile: { weights }, climb_target_m }),
  });

/** `target_m: null` asks for as much climb as possible (D52). */
export interface ClimbGoal { target_m: number | null; strictness: number }

export const generate = (start: LatLon, distance_m: number, climb: ClimbGoal, weights: Weights) =>
  call<{ routes: Route[] }>("/api/generate", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ start, distance_m, climb, profile: { weights } }),
  }).then((r) => r.routes);
