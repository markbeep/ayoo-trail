// Share links (D91): the query, the selected loop and its dragged points in the URL query
// (ADR 0006; `#` before it, still read).
// Pure: no state imports, so `node --test` can run it.
import type { Checkpoint, LatLon, Penalties, Weights } from "./routes/types";
import type { Zone } from "./routes/zones";

export type ShareClimb = { preset: "gentle" | "hilly" | "mountain" } | { m: number } | "max" | "off";

export interface Share {
  start: LatLon;
  km: number;
  climb: ShareClimb;
  strictness: number;
  weights: Weights;
  maxSac: number;
  zones: Zone[];
  checkpoints: Checkpoint[];
  /** Index of the selected loop (ids are `loop-<i>`, best first, D79). */
  loop: number;
  /** The loop's waypoints when it was edited, else null. */
  wps: LatLon[] | null;
  penalties: Penalties;
  /** Only ways on swisstopo's map (ADR 0007); `v=0` in the link turns it off. */
  tlmOnly: boolean;
}

/** The server's defaults (`Profile::default`, D109). */
export const DEFAULT_PENALTIES: Penalties = { repeat: 5, avoid_factor: 5, prefer_factor: 0.5, stem_m: 500, max_overlap: 0.5 };
/** Order in `n=` and each value's range, as `Profile::validate` checks them. */
const PENALTIES: [keyof Penalties, number, number][] = [
  ["repeat", 0.01, 1000], ["avoid_factor", 1, 1000], ["prefer_factor", 0.001, 1], ["stem_m", 0, 5000], ["max_overlap", 0, 1],
];

const r5 = (x: number) => Math.round(x * 1e5) / 1e5;
const pt = (p: LatLon) => `${r5(p.lat)},${r5(p.lon)}`;

/** Weight order in `w=`; new terms go at the end so older links still parse. */
const WEIGHTS = ["slope", "uphill", "downhill", "surface", "tech", "technical", "official", "forest", "urban", "traffic", "view"] as const;
const HILLS = new Set(["uphill", "downhill", "technical"]);
/** Strength-only terms: 0–100; the rest −100–100 (D106). */
const STRENGTH = new Set(["slope", "tech"]);
const PRESETS = ["gentle", "hilly", "mountain"];
/** Zone flag in `z=` (D97): absent = soft avoid, as in links before it. */
const ZONE_FLAGS: Record<string, Pick<Zone, "kind" | "hard">> = {
  A: { kind: "avoid", hard: true }, p: { kind: "prefer", hard: false }, P: { kind: "prefer", hard: true },
};
const zoneFlag = (z: Zone) => (z.kind === "prefer" ? (z.hard ? ",P" : ",p") : z.hard ? ",A" : "");
export const MAX_CHECKPOINTS = 50;
// URLSearchParams decodes one level on read; the second keeps `,` and `;` in names apart.
const nameOut = (n: string) => encodeURIComponent(encodeURIComponent(n));

/** Compact `k=v&…` hash: plain numbers and commas, no JSON, so the link stays short and readable. */
export function encodeShare(s: Share): string {
  const c = s.climb === "max" || s.climb === "off" ? s.climb : "preset" in s.climb ? s.climb.preset : String(Math.round(s.climb.m));
  const parts = [`s=${pt(s.start)}`, `d=${s.km}`, `c=${c}`, `t=${s.strictness}`, `x=${s.maxSac}`,
    `w=${WEIGHTS.map((k) => Math.round(s.weights[k] * 100) / 100).join(",")}`];
  if (s.loop) parts.push(`l=${s.loop}`);
  if (!s.tlmOnly) parts.push("v=0");
  if (PENALTIES.some(([k]) => s.penalties[k] !== DEFAULT_PENALTIES[k])) parts.push(`n=${PENALTIES.map(([k]) => s.penalties[k]).join(",")}`);
  if (s.zones.length) parts.push(`z=${s.zones.map((z) => `${pt(z)},${Math.round(z.radius_m)}${zoneFlag(z)}`).join(";")}`);
  if (s.checkpoints.length) parts.push(`k=${s.checkpoints.map((c) => `${pt(c)},${nameOut(c.name)}`).join(";")}`);
  if (s.wps) parts.push(`p=${s.wps.map(pt).join(";")}`);
  // %3B, not `;`: the deployed proxy splits a raw `;` like `&` and cuts `p=` and `z=` short (ADR 0006).
  return parts.join("&").replaceAll(";", "%3B");
}

const num = (v: unknown, lo: number, hi: number): number => {
  const n = typeof v === "string" && v.trim() !== "" ? Number(v) : NaN;
  if (!Number.isFinite(n) || n < lo || n > hi) throw new Error("range");
  return n;
};
const nums = (v: string) => v.split(",");
const latLon = (a: string[]): LatLon => ({ lat: num(a[0], -90, 90), lon: num(a[1], -180, 180) });
const list = (v: string | null, max: number): string[][] => {
  const items = v ? v.split(";").map(nums) : [];
  if (items.length > max) throw new Error("list");
  return items;
};

/** The share in a URL query or hash, or null when there is none or it doesn't validate. */
export function decodeShare(hash: string): Share | null {
  const q = new URLSearchParams(hash.replace(/^[#?]/, ""));
  const start = q.get("s");
  if (!start) return null;
  try {
    const w = nums(q.get("w") ?? "");
    const weights = {} as Weights;
    WEIGHTS.forEach((k, i) => {
      const hill = HILLS.has(k);
      weights[k] = w[i] === undefined || w[i] === "" ? (k === "uphill" || k === "view" ? 0 : hill ? -1 : 1) : num(w[i], hill ? -1 : STRENGTH.has(k) ? 0 : -100, hill ? 1 : 100);
    });
    const c = q.get("c") ?? "";
    const climb: ShareClimb = c === "max" || c === "off" ? c : PRESETS.includes(c) ? { preset: c as "hilly" } : { m: num(c, 0, 20_000) };
    return {
      start: latLon(nums(start)),
      km: num(q.get("d"), 3, 80),
      climb,
      strictness: num(q.get("t"), 0, 1),
      weights,
      maxSac: Math.round(num(q.get("x"), 1, 6)),
      zones: list(q.get("z"), 32).map((a) => {
        if (a[3] !== undefined && !ZONE_FLAGS[a[3]]) throw new Error("zone flag");
        return { ...latLon(a), radius_m: num(a[2], 50, 5000), ...(a[3] ? ZONE_FLAGS[a[3]] : {}) };
      }),
      checkpoints: list(q.get("k"), MAX_CHECKPOINTS).map((a) => ({ ...latLon(a), name: decodeURIComponent(a[2] ?? "") })),
      loop: Math.round(num(q.get("l") ?? "0", 0, 50)),
      wps: q.has("p") ? list(q.get("p"), 20).map(latLon) : null,
      penalties: q.has("n")
        ? Object.fromEntries(PENALTIES.map(([k, lo, hi], i) => [k, num(nums(q.get("n")!)[i], lo, hi)])) as unknown as Penalties
        : { ...DEFAULT_PENALTIES },
      tlmOnly: q.get("v") !== "0",
    };
  } catch {
    return null;
  }
}
