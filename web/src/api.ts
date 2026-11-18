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

export const routeVia = (waypoints: LatLon[], weights: Weights) =>
  call<Route>("/api/route", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ waypoints, profile: { weights } }),
  });
