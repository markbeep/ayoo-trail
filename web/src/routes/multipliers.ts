// Settings as multipliers (D106): the API keeps exponents (`factor^w`, D51); the UI shows the
// factor a weight gives the thing it acts on. Pure, so `node --test` can run it.

export type Term = "surface" | "official" | "forest" | "urban" | "traffic" | "view" | "slope" | "tech";

/** The table factor each term's multiplier is read on, copied from `src/cost.rs`: asphalt,
 * marked trail, full forest, town, busy road, wide view; slope is the gentle row's steepest
 * descent band, tech T4+ in the avoid row. */
export const KEY: Record<Term, number> = {
  surface: 2.0, official: 0.7, forest: 0.9, urban: 2.0, traffic: 2.0, view: 0.7, slope: 2.5, tech: 1.4,
};
/** Hills and technical keep their own shape controls; their multiplier is strength only. */
const STRENGTH_ONLY = new Set<Term>(["slope", "tech"]);
export const MULT_MIN = 0.001;
export const MULT_MAX = 1000;

export const multiplierOf = (t: Term, w: number) => KEY[t] ** w;

export function weightOf(t: Term, m: number): number {
  const c = Math.min(MULT_MAX, Math.max(STRENGTH_ONLY.has(t) ? 1 : MULT_MIN, m));
  return Math.round((Math.log(c) / Math.log(KEY[t])) * 100) / 100 || 0;
}

/** "2", "×2", "x0.01", "1e2" → the number; null for anything that isn't a positive finite. */
export function parseMultiplier(s: string): number | null {
  const v = s.trim().replace(/^[×x*]\s*/i, "");
  const n = v === "" ? NaN : Number(v);
  return Number.isFinite(n) && n > 0 ? n : null;
}

export interface ScrubOpts { scale: "log" | "linear"; step: number; min: number; max: number; fast?: boolean }
/** The value after dragging `dxPx` pixels: ×1.01 per px on a log scale, `step` per px linear; Shift ×10. */
export function scrub(v: number, dxPx: number, o: ScrubOpts): number {
  const k = dxPx * (o.fast ? 10 : 1);
  const next = o.scale === "log" ? v * 1.01 ** k : v + k * o.step;
  return Math.min(o.max, Math.max(o.min, next));
}

export function word(t: Term, w: number): "avoid" | "off" | "prefer" {
  if (Math.abs(w) < 0.005) return "off";
  return multiplierOf(t, w) > 1 ? "avoid" : "prefer";
}

/** A typed number with its unit or sign glyph ("500 m", "−1.00"); null when there are no digits. */
export function parseNumber(s: string): number | null {
  const v = s.replace(/−/g, "-").replace(/[^\d.eE+-]/g, "");
  const n = /\d/.test(v) ? Number(v) : NaN;
  return Number.isFinite(n) ? n : null;
}
