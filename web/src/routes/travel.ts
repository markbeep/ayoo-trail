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

/** A place an origin reaches: `[lon, lat, door-to-door seconds]`; the origin itself is 0 s (ADR 0005). */
export type Stop = [number, number, number];

/** Same rules as the server (`transit.rs`): 4.5 km/h, last walk at most 20 min. */
const WALK_MPS = 4.5 / 3.6;
const LAST_WALK_S = 20 * 60;
const EARTH_M = 6_371_008.8;
const CELL_M = 200;
/** RGBA for shared and partial reach: one hue, two strengths. */
export const SHARED = [31, 78, 140, 150] as const;
export const PARTIAL = [31, 78, 140, 70] as const;

const rad = (d: number) => (d * Math.PI) / 180;

/** Seconds from an origin to `p` through its best stop, or null when over its budget. */
export function timeTo(stops: readonly Stop[], minutes: number, p: LatLon): number | null {
  const budget = minutes * 60, k = Math.cos(rad(p.lat));
  let best = Infinity;
  for (const [lon, lat, secs] of stops) {
    // Equirectangular: exact enough within a 1.5 km walk.
    const walk = (EARTH_M * Math.hypot(rad(lon - p.lon) * k, rad(lat - p.lat))) / WALK_MPS;
    if (walk <= LAST_WALK_S && secs + walk < best) best = secs + walk;
  }
  return best <= budget ? Math.round(best) : null;
}

const mercX = (lon: number) => lon / 360 + 0.5;
const mercY = (lat: number) => { const s = Math.sin(rad(lat)); return 0.5 - Math.log((1 + s) / (1 - s)) / (4 * Math.PI); };
const unmercLat = (y: number) => (Math.atan(Math.sinh(Math.PI * (1 - 2 * y))) * 180) / Math.PI;
/** Mercator units per ground meter at a latitude. */
const scale = (lat: number) => 1 / (2 * Math.PI * EARTH_M * Math.cos(rad(lat)));

export interface ReachImage { width: number; height: number; data: Uint8ClampedArray<ArrayBuffer>; bounds: [number, number, number, number] }

/**
 * Shared and partial reach on ~200 m web-mercator cells: every stop is a walking disc of
 * min(budget left, 20 min). Bounds are `[west, south, east, north]`.
 */
export function reachImage(lists: readonly (readonly Stop[])[], minutes: readonly number[]): ReachImage {
  const discs = lists.map((stops, o) => stops.map(([lon, lat, secs]) => {
    const r = Math.min((minutes[o] ?? 0) * 60 - secs, LAST_WALK_S) * WALK_MPS * scale(lat);
    return [mercX(lon), mercY(lat), r] as const;
  }));
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const [x, y, r] of discs.flat()) { x0 = Math.min(x0, x - r); y0 = Math.min(y0, y - r); x1 = Math.max(x1, x + r); y1 = Math.max(y1, y + r); }
  const cell = CELL_M * scale(unmercLat((y0 + y1) / 2));
  const width = Math.max(1, Math.ceil((x1 - x0) / cell)), height = Math.max(1, Math.ceil((y1 - y0) / cell));
  const count = new Uint8Array(width * height), last = new Uint8Array(width * height).fill(255);
  discs.forEach((ds, o) => {
    for (const [x, y, r] of ds) {
      const i0 = Math.max(0, Math.floor((x - r - x0) / cell)), i1 = Math.min(width - 1, Math.floor((x + r - x0) / cell));
      const j0 = Math.max(0, Math.floor((y - r - y0) / cell)), j1 = Math.min(height - 1, Math.floor((y + r - y0) / cell));
      for (let j = j0; j <= j1; j++) {
        const dy = y0 + (j + 0.5) * cell - y;
        for (let i = i0; i <= i1; i++) {
          const dx = x0 + (i + 0.5) * cell - x, k = j * width + i;
          if (dx * dx + dy * dy <= r * r && last[k] !== o) { last[k] = o; count[k]!++; }
        }
      }
    }
  });
  const data = new Uint8ClampedArray(width * height * 4);
  count.forEach((c, k) => { if (c) data.set(c === lists.length ? SHARED : PARTIAL, k * 4); });
  const lonAt = (x: number) => (x - 0.5) * 360;
  return { width, height, data, bounds: [lonAt(x0), unmercLat(y0 + height * cell), lonAt(x0 + width * cell), unmercLat(y0)] };
}

/** Alpha of the image at `p` (0 outside). */
export function alphaAt(img: ReachImage, p: LatLon): number {
  const [w, , e, n] = img.bounds;
  const i = Math.floor(((mercX(p.lon) - mercX(w)) / (mercX(e) - mercX(w))) * img.width);
  const cell = (mercX(e) - mercX(w)) / img.width;
  const j = Math.floor((mercY(p.lat) - mercY(n)) / cell);
  return i < 0 || j < 0 || i >= img.width || j >= img.height ? 0 : img.data[(j * img.width + i) * 4 + 3]!;
}
