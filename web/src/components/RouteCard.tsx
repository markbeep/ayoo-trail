import { createSignal, Show } from "solid-js";
import { Check, Download, Link, LoaderCircle, RefreshCw, Trash2, TriangleAlert, Undo2 } from "@lucide/solid";
import type { Route } from "../routes/types";
import {
  checkpoints, costView, editError, findRoutes, deleteCopy, hoverIndex, refreshing, setCostView, shareLink, stale, undoEdit,
} from "../state";
import { costBucket, termBucket } from "../map/controller";
import { narrow } from "../layout";
import { downloadGpx } from "../routes/gpx";
import { km, meters, score } from "../routes/format";
import { ElevationProfile } from "./ElevationProfile";
import { Preferences } from "./Preferences";

/** Plain words for each color of the cost ramp, best first. */
const PREFERENCE = ["preferred", "good", "fine", "avoided", "strongly avoided"];

export function RouteCard(props: { route: Route; label: { name: string; star: string } }) {
  const name = () => props.label.name + props.label.star;
  const [copied, setCopied] = createSignal(false);
  const share = () => void navigator.clipboard?.writeText(shareLink()).then(() => {
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  });
  const r = () => props.route;
  const bars = () => r().breakdown;
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
    <section class={["card route-card", { refreshing: refreshing() }]} aria-busy={`${refreshing()}`} aria-label={`${name()}, score ${score(r().score)}`}>
      <header class="rc-head">
        <h2>{props.label.name}<Show when={props.label.star}><sup>{props.label.star}</sup></Show></h2>
        <button class="icon-btn" aria-label={`Download ${name()} as GPX`} title="Download GPX"
          onClick={() => downloadGpx(r(), name(), `${name()} · ${km(r().distance_m)}`, checkpoints())}>
          <Download size={18} />
        </button>
        <button class="icon-btn" aria-label={copied() ? "Link copied" : `Copy a link to ${name()}`} title={copied() ? "Copied" : "Copy link"} onClick={share}>
          <Show when={copied()} fallback={<Link size={18} />}><Check size={18} /></Show>
        </button>
        <span class="score" title="Score out of 100">{score(r().score)}</span>
      </header>
      <p class="meta">
        <span>{km(r().distance_m)}</span><span>{meters(r().climb_m)} up</span><span>{meters(r().descent_m)} down</span>
      </p>
      <Show when={!r().distance_met}><p class="badge warn"><TriangleAlert size={14} />Closest match to your distance</p></Show>

      <ElevationProfile route={r()} />

      <Show when={r().copyOf} fallback={
        <p class="hint edit-hint">{narrow() ? "Drag a point on the map to move it, tap it to remove it." : "Drag a point to move it, drag the line to add one, click a point to remove it."}</p>
      }>
        <div class="edited">
          <span>Edited</span>
          <button class="link-btn" onClick={() => undoEdit(r().id)}><Undo2 size={14} />Undo</button>
          <button class="link-btn" onClick={() => deleteCopy(r().id)}><Trash2 size={14} />Delete</button>
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

      <Preferences
        bars={bars()}
        colorOf={(k) => { const t = termsHere(); return t ? termBucket(t[k]) : undefined; }}
        heading={<>Why it scores {score(r().score)}<Show when={termsHere()}><span class="why-here"> · colors show the point you're on</span></Show><Show when={refreshing()}><span class="updating"><LoaderCircle size={13} class="spin" />Updating</span></Show></>}
      />
    </section>
  );
}
