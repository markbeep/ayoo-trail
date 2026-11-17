import { createMemo } from "solid-js";
import type { Route } from "../routes/types";
import { haversine } from "../routes/geo";

/** Shared vertical scale so profiles of different loops compare honestly. */
const SCALE_M = 600;

export function ElevationProfile(props: { route: Route }) {
  const W = 360, H = 64;
  const path = createMemo(() => {
    const c = props.route.geometry.coordinates;
    if (c.length < 2) return { line: "", area: "", lo: 0, hi: 0 };
    let d = 0;
    const pts: [number, number][] = [[0, c[0]![2]]];
    for (let i = 1; i < c.length; i++) { d += haversine(c[i - 1]!, c[i]!); pts.push([d, c[i]![2]]); }
    const zs = pts.map((p) => p[1]);
    const lo = Math.min(...zs), hi = Math.max(...zs);
    const base = Math.max(0, (lo + hi) / 2 - SCALE_M / 2);
    const x = (m: number) => (m / d) * W;
    const y = (z: number) => H - Math.min(1, Math.max(0, (z - base) / SCALE_M)) * (H - 4);
    const line = pts.map(([m, z], i) => `${i ? "L" : "M"}${x(m).toFixed(1)} ${y(z).toFixed(1)}`).join("");
    return { line, area: `${line}L${W} ${H}L0 ${H}Z`, lo, hi };
  });
  return (
    <figure class="profile">
      <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" role="img"
        aria-label={`Elevation from ${Math.round(path().lo)} to ${Math.round(path().hi)} m`}>
        <path d={path().area} class="profile-area" />
        <path d={path().line} class="profile-line" vector-effect="non-scaling-stroke" />
      </svg>
      <figcaption><span>{Math.round(path().lo)} m</span><span>{Math.round(path().hi)} m</span></figcaption>
    </figure>
  );
}
