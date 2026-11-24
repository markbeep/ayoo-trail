// Travel origins for the Travel time layer (ADR 0002). Pure helpers, no map or state.
import type { LatLon } from "./types";

/** A numbered point someone travels from, with its one-way, door-to-door time budget. */
export interface TravelOrigin { lat: number; lon: number; minutes: number }

/** The server's limits (`transit.rs`). */
export const MAX_ORIGINS = 8;
export const MIN_MINUTES = 10;
export const MAX_MINUTES = 240;
export const DEFAULT_MINUTES = 60;

export const clampMinutes = (m: number) =>
  Number.isFinite(m) ? Math.min(MAX_MINUTES, Math.max(MIN_MINUTES, Math.round(m))) : DEFAULT_MINUTES;

export const withOrigin = (list: readonly TravelOrigin[], p: LatLon): TravelOrigin[] =>
  list.length >= MAX_ORIGINS ? [...list] : [...list, { lat: p.lat, lon: p.lon, minutes: DEFAULT_MINUTES }];

/** Origins from localStorage, which may hold anything. */
export function loadOrigins(v: unknown): TravelOrigin[] {
  if (!Array.isArray(v)) return [];
  return v
    .filter((o): o is TravelOrigin => typeof o?.lat === "number" && typeof o?.lon === "number" && typeof o?.minutes === "number")
    .slice(0, MAX_ORIGINS)
    .map((o) => ({ lat: o.lat, lon: o.lon, minutes: clampMinutes(o.minutes) }));
}
