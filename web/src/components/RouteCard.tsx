import { createSignal, For, Show } from "solid-js";
import { Download, LoaderCircle, RefreshCw, RotateCcw, TriangleAlert, Undo2 } from "@lucide/solid";
import type { BreakdownKey, Route } from "../routes/types";
import {
  FACTORS, costView, editError, findRoutes, hoverIndex, isEdited, refreshing, resetEdit, setCostView, stale, undoEdit,
} from "../state";
import { costBucket, termBucket } from "../map/controller";
import { narrow } from "../layout";
import { ClimbSlider, HillsPanel, StrictnessSlider, TechPanel, WeightSlider } from "./Controls";
import { downloadGpx } from "../routes/gpx";
import { km, meters, score } from "../routes/format";
import { ElevationProfile } from "./ElevationProfile";

/** Plain words for each color of the cost ramp, best first. */
const PREFERENCE = ["preferred", "good", "fine", "avoided", "strongly avoided"];

export function RouteCard(props: { route: Route; name: string }) {
  const [open, setOpen] = createSignal<BreakdownKey | null>(null);
  const r = () => props.route;
  // With Max there is no target to score against, but the row stays so its climb controls don't vanish (D78).
  const bars = () => r().breakdown.some((b) => b.key === "climb") ? r().breakdown
    : [{ key: "climb" as const, value: null, detail: `${meters(r().climb_m)} of climb, as much as possible` }, ...r().breakdown];
  const best = () => Math.min(...(r().cost_per_m ?? []).filter((c) => c > 0));
  const hoverAt = () => {
    const i = hoverIndex(), c = i === null ? undefined : r().cost_per_m?.[i];
    return c === undefined || !(c > 0) ? null : { cost: c, ratio: c / best() };
  };
  /** Each term at the hovered point (D61), or null when nothing is hovered. */
  const termsHere = () => {
    const i = hoverIndex(), ts = r().terms;
    if (!costView() || i === null || !ts?.length) return null;
    let lo = 0, hi = ts.length - 1; // last stretch starting at or before i
    while (lo < hi) { const m = (lo + hi + 1) >> 1; if (ts[m]!.i <= i) lo = m; else hi = m - 1; }
    return ts[lo]!;
  };
  return (
    <section class={["card route-card", { refreshing: refreshing() }]} aria-busy={`${refreshing()}`} aria-label={`${props.name}, score ${score(r().score)}`}>
      <header class="rc-head">
        <h2>{props.name}</h2>
        <button class="icon-btn" aria-label={`Download ${props.name} as GPX`} title="Download GPX"
          onClick={() => downloadGpx(r(), props.name, `${props.name} · ${km(r().distance_m)}`)}>
          <Download size={18} />
        </button>
        <span class="score" title="Score out of 100">{score(r().score)}</span>
      </header>
      <p class="meta">
        <span>{km(r().distance_m)}</span><span>{meters(r().climb_m)} up</span><span>{meters(r().descent_m)} down</span>
      </p>
      <Show when={!r().distance_met}><p class="badge warn"><TriangleAlert size={14} />Closest match to your distance</p></Show>

      <ElevationProfile route={r()} />

      <Show when={isEdited(r().id)} fallback={
        <p class="hint edit-hint">{narrow() ? "Drag a point on the map to move it, tap it to remove it." : "Drag a point to move it, drag the line to add one, click a point to remove it."}</p>
      }>
        <div class="edited">
          <span>Edited</span>
          <button class="link-btn" onClick={() => undoEdit(r().id)}><Undo2 size={14} />Undo</button>
          <button class="link-btn" onClick={() => resetEdit(r().id)}><RotateCcw size={14} />Reset</button>
        </div>
      </Show>
      <Show when={editError()}>{(m) => <p class="query-error" role="alert">{m()}</p>}</Show>

      <button role="switch" aria-checked={`${costView()}`} class={["layer overlay cost-switch", { active: costView() }]}
        onClick={() => setCostView(!costView())}>
        Color by preference<span class="switch" aria-hidden="true" />
      </button>
      <Show when={costView()}>
        <div class="cost-legend">
          <span class="cost-ramp" aria-hidden="true"><i /><i /><i /><i /><i /></span>
          <span class="cost-scale" aria-hidden="true"><span>Preferred</span><span>Avoided</span></span>
          <p class="hint">
            <Show when={hoverAt()} fallback={
              "Green is what your settings like, red what they avoid. The loop takes the most preferred way between its points, so red shows where it had to compromise. Hover the line for details."
            }>{(h) => (
              <>Here: <strong>{PREFERENCE[costBucket(h().cost)]}</strong>{h().ratio >= 1.05 ? `, ${h().ratio.toFixed(1)}× the cost of this loop's best stretch` : ", as good as this loop gets"}</>
            )}</Show>
          </p>
        </div>
      </Show>

      <Show when={stale()}>
        <div class="stale">
          <span>Settings changed</span>
          <button class="link-btn" onClick={() => void findRoutes()}><RefreshCw size={14} />Find loops again</button>
        </div>
      </Show>

      <h3 class="why">Why it scores {score(r().score)}<Show when={termsHere()}><span class="why-here"> · colors show the point you're on</span></Show><Show when={refreshing()}><span class="updating"><LoaderCircle size={13} class="spin" />Updating</span></Show></h3>
      <ul class="bars">
        <For each={bars()}>{(b) => {
          const f = FACTORS[b.key];
          const isOpen = () => open() === b.key;
          return (
            <li class={{ open: isOpen() }}>
              <button class="bar" aria-expanded={`${isOpen()}`} onClick={() => setOpen(isOpen() ? null : b.key)}>
                <span class="bar-label">{f.label}</span>
                <span class="track" aria-hidden="true"><span class="fill" style={{
                  width: `${Math.round((b.value ?? 0) * 100)}%`,
                  background: termsHere() && b.key !== "climb" ? `var(--cost-${termBucket(termsHere()![b.key]) + 1})` : undefined,
                }} /></span>
                <span class="bar-value"><Show when={b.value !== null} fallback={<span class="sr-only">, not scored. </span>}><span class="sr-only">, </span>{Math.round(b.value! * 100)}<span class="sr-only"> out of 100. </span></Show></span>
                <span class="link-btn">{isOpen() ? "Close" : "Adjust"}</span>
              </button>
              <Show when={isOpen()}>
                <div class="adjust">
                  <p><strong>{b.detail}.</strong> {f.question}</p>
                  <Show when={b.key === "climb"} fallback={
                    <Show when={b.key === "slope"} fallback={b.key === "tech" ? <TechPanel /> : <WeightSlider weight={f.weight!} />}>
                      <HillsPanel />
                    </Show>
                  }>
                    <div class="climb-slider"><ClimbSlider /></div>
                    <StrictnessSlider />
                  </Show>
                </div>
              </Show>
            </li>
          );
        }}</For>
      </ul>
    </section>
  );
}
