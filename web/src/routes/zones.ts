// Avoid and prefer areas (D90, D97): circles the router penalizes, blocks, discounts or must
// pass. Pure helpers, no map or state.
import type { LatLon } from "./types";

/** `kind` and `hard` are optional so stored zones and old links stay soft avoids. */
export interface Zone { lat: number; lon: number; radius_m: number; kind?: "avoid" | "prefer"; hard?: boolean }

export type ZoneMode = "penalize" | "block" | "prefer" | "must";

/** The server's `ExclusionMode` for a zone. */
export const zoneMode = (z: Zone): ZoneMode =>
  z.kind === "prefer" ? (z.hard ? "must" : "prefer") : z.hard ? "block" : "penalize";

export const ZONE_DEFAULT_M = 300;
export const ZONE_MIN_M = 50;
export const ZONE_MAX_M = 5000;
/** The server rejects more (`Profile::validate`). */
export const MAX_ZONES = 32;
export const MAX_MUST = 5;
export const isMust = (z: Zone) => zoneMode(z) === "must";

const M_PER_DEG_LAT = 111_320;

export const clampRadius = (m: number) => Math.round(Math.min(ZONE_MAX_M, Math.max(ZONE_MIN_M, m)));

/** Radius for the edge handle dragged to `p`, in the same local meters the circle is drawn in. */
export function radiusTo(z: Zone, p: LatLon) {
  const dy = (p.lat - z.lat) * M_PER_DEG_LAT;
  const dx = (p.lon - z.lon) * M_PER_DEG_LAT * Math.cos((z.lat * Math.PI) / 180);
  return clampRadius(Math.hypot(dx, dy));
}

/** The resize handle: due east on the circle. */
export function edgePoint(z: Zone): [number, number] {
  return [z.lon + z.radius_m / (M_PER_DEG_LAT * Math.cos((z.lat * Math.PI) / 180)), z.lat];
}

/** Closed [lon, lat] ring approximating the circle. */
export function circle(z: Zone, n = 64): [number, number][] {
  const dLat = z.radius_m / M_PER_DEG_LAT;
  const dLon = dLat / Math.cos((z.lat * Math.PI) / 180);
  const ring: [number, number][] = [];
  for (let i = 0; i <= n; i++) {
    const a = (2 * Math.PI * (i % n)) / n;
    ring.push([z.lon + dLon * Math.cos(a), z.lat + dLat * Math.sin(a)]);
  }
  return ring;
}
