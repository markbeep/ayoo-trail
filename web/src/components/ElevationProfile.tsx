import { createMemo, Show } from "solid-js";
import type { Route } from "../routes/types";
import { cumulative } from "../routes/geo";
import { km } from "../routes/format";
import { hoverIndex, routes, setHoverIndex } from "../state";

/** Smallest vertical span, so a flat loop doesn't get its 20 m bumps blown up into mountains. */
const MIN_SPAN_M = 150;
const W = 360, H = 88;

export function ElevationProfile(props: { route: Route }) {
  // One vertical scale for all current loops (the largest range), so their profiles compare honestly.
  const span = createMemo(() => Math.max(MIN_SPAN_M, ...routes().map((r) => {
    const zs = r.geometry.coordinates.map((c) => c[2]);
    return Math.max(...zs) - Math.min(...zs);
  })));
  const data = createMemo(() => {
    const c = props.route.geometry.coordinates;
    const d = cumulative(c);
    const zs = c.map((p) => p[2]);
    const lo = Math.min(...zs), hi = Math.max(...zs), total = d[d.length - 1] || 1;
    const x = (m: number) => (m / total) * W;
    const y = (z: number) => H - 2 - ((z - lo) / span()) * (H - 6);
    const line = c.length < 2 ? "" : zs.map((z, i) => `${i ? "L" : "M"}${x(d[i]!).toFixed(1)} ${y(z).toFixed(1)}`).join("");
    return { d, zs, lo, hi, total, x, y, line, area: line && `${line}L${W} ${H}L0 ${H}Z` };
  });

  let plot!: HTMLDivElement;
  const pick = (e: PointerEvent) => {
    const { d, total } = data();
    const r = plot.getBoundingClientRect();
    const m = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)) * total;
    // Binary search for the last position at or before m.
    let lo = 0, hi = d.length - 1;
    while (lo < hi) { const mid = (lo + hi + 1) >> 1; if (d[mid]! <= m) lo = mid; else hi = mid - 1; }
    setHoverIndex(lo);
  };
  const at = () => {
    const i = hoverIndex();
    const { d, zs, x, y } = data();
    if (i === null || i >= zs.length) return null;
    return { left: (x(d[i]!) / W) * 100, top: (y(zs[i]!) / H) * 100, z: zs[i]!, m: d[i]! };
  };

  return (
    <figure class="profile">
      <div class="profile-plot" ref={plot} onPointerMove={pick} onPointerDown={pick} onPointerLeave={() => setHoverIndex(null)}>
        <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" role="img"
          aria-label={`Elevation from ${Math.round(data().lo)} to ${Math.round(data().hi)} m over ${km(data().total)}`}>
          <path d={data().area} class="profile-area" />
          <path d={data().line} class="profile-line" vector-effect="non-scaling-stroke" />
        </svg>
        <Show when={at()}>{(p) => (
          <>
            <span class="profile-cursor" style={{ left: `${p().left}%` }} />
            <span class="profile-dot" style={{ left: `${p().left}%`, top: `${p().top}%` }} />
          </>
        )}</Show>
      </div>
      <figcaption aria-live="polite">
        <Show when={at()} fallback={<span>{Math.round(data().lo)}–{Math.round(data().hi)} m</span>}>{(p) => (
          <span class="profile-readout"><strong>{Math.round(p().z)} m</strong> at {km(p().m)}</span>
        )}</Show>
        <span>{km(data().total)}</span>
      </figcaption>
    </figure>
  );
}
