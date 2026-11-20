// Share links (D91): the query, the selected loop and its dragged points in the URL hash.
// Pure: no state imports, so `node --test` can run it.
import type { LatLon, Weights } from "./routes/types";
import type { Zone } from "./routes/zones";

export type ShareClimb = { preset: "gentle" | "hilly" | "mountain" } | { m: number } | "max";

export interface Share {
  start: LatLon;
  startName: string | null;
  km: number;
  climb: ShareClimb;
  strictness: number;
  weights: Weights;
  maxSac: number;
  zones: Zone[];
  /** Index of the selected loop (ids are `loop-<i>`, best first, D79). */
  loop: number;
  /** The loop's waypoints when it was edited, else null. */
  wps: LatLon[] | null;
}

const r5 = (x: number) => Math.round(x * 1e5) / 1e5;
const pt = (p: LatLon) => [r5(p.lat), r5(p.lon)];

export function encodeShare(s: Share): string {
  const v = {
    s: pt(s.start), n: s.startName, d: s.km, c: s.climb, t: s.strictness, w: s.weights, x: s.maxSac,
    z: s.zones.map((z) => [...pt(z), Math.round(z.radius_m)]), l: s.loop, p: s.wps?.map(pt) ?? null,
  };
  return `s=${encodeURIComponent(JSON.stringify(v))}`;
}

const TERMS = ["slope", "surface", "tech", "official", "forest", "urban", "traffic"] as const;
const HILLS = ["uphill", "downhill", "technical"] as const;
const PRESETS = ["gentle", "hilly", "mountain"];

const num = (v: unknown, lo: number, hi: number): number => {
  if (typeof v !== "number" || !Number.isFinite(v) || v < lo || v > hi) throw new Error("range");
  return v;
};
const latLon = (v: unknown): LatLon => {
  if (!Array.isArray(v)) throw new Error("point");
  return { lat: num(v[0], -90, 90), lon: num(v[1], -180, 180) };
};
const list = (v: unknown, max: number): unknown[] => {
  if (!Array.isArray(v) || v.length > max) throw new Error("list");
  return v;
};

/** The share in a URL hash, or null when there is none or it doesn't validate. */
export function decodeShare(hash: string): Share | null {
  const raw = new URLSearchParams(hash.replace(/^#/, "")).get("s");
  if (!raw) return null;
  try {
    const v = JSON.parse(raw) as Record<string, unknown>;
    const w = (v.w ?? {}) as Record<string, unknown>;
    const weights = {} as Weights;
    for (const k of TERMS) weights[k] = num(w[k] ?? 1, 0, 4);
    for (const k of HILLS) weights[k] = num(w[k], -1, 1);
    const c = v.c;
    let climb: ShareClimb;
    if (c === "max") climb = "max";
    else if (c && typeof c === "object" && "preset" in c && PRESETS.includes(String(c.preset))) climb = { preset: c.preset as "hilly" };
    else if (c && typeof c === "object" && "m" in c) climb = { m: num(c.m, 0, 20_000) };
    else throw new Error("climb");
    return {
      start: latLon(v.s),
      startName: typeof v.n === "string" ? v.n.slice(0, 200) : null,
      km: num(v.d, 3, 60),
      climb,
      strictness: num(v.t, 0, 1),
      weights,
      maxSac: Math.round(num(v.x, 1, 6)),
      zones: list(v.z ?? [], 32).map((z) => {
        const a = list(z, 3);
        return { ...latLon(a), radius_m: num(a[2], 50, 5000) };
      }),
      loop: Math.round(num(v.l ?? 0, 0, 50)),
      wps: v.p == null ? null : list(v.p, 20).map(latLon),
    };
  } catch {
    return null;
  }
}
