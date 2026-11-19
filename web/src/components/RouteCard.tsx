import { createSignal, For, Show } from "solid-js";
import { Download, LoaderCircle, RefreshCw, RotateCcw, TriangleAlert, Undo2 } from "@lucide/solid";
import type { BreakdownKey, Route } from "../routes/types";
import {
  FACTORS, costView, editError, findRoutes, hoverIndex, isEdited, refreshing, resetEdit, setCostView, stale, undoEdit,
} from "../state";
import { COST_STOPS } from "../map/controller";
import { narrow } from "../layout";
import { HillsPanel, WeightSlider } from "./Controls";
import { downloadGpx } from "../routes/gpx";
import { km, meters, score } from "../routes/format";
import { ElevationProfile } from "./ElevationProfile";

export function RouteCard(props: { route: Route; name: string }) {
  const [open, setOpen] = createSignal<BreakdownKey | null>(null);
  const r = () => props.route;
  return (
    <section class="card route-card" aria-label={`${props.name}, score ${score(r().score)}`}>
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
        Colour by cost<span class="switch" aria-hidden="true" />
      </button>
      <Show when={costView()}>
        <div class="cost-legend" aria-label="Cost per metre, cheap to expensive">
          <span class="cost-ramp" aria-hidden="true"><i /><i /><i /><i /><i /></span>
          <span class="cost-scale"><span>Cheap</span><span>{COST_STOPS[0]}</span><span>{COST_STOPS[1]}</span><span>{COST_STOPS[2]}</span><span>{COST_STOPS[3]}</span><span>Costly</span></span>
          <p class="hint">
            <Show when={hoverIndex() !== null && r().cost_per_m?.[hoverIndex()!] !== undefined}
              fallback="Cost per metre under your settings. A flat marked forest trail is 0.63.">
              Here: <strong>{r().cost_per_m[hoverIndex()!]!.toFixed(2)}</strong> per metre
            </Show>
          </p>
        </div>
      </Show>

      <Show when={stale()}>
        <div class="stale">
          <span>Settings changed</span>
          <button class="link-btn" onClick={() => void findRoutes()}><RefreshCw size={14} />Find loops again</button>
        </div>
      </Show>

      <h3 class="why">Why it scores {score(r().score)}<Show when={refreshing()}><span class="updating"><LoaderCircle size={13} class="spin" />Updating</span></Show></h3>
      <ul class="bars">
        <For each={r().breakdown}>{(b) => {
          const f = FACTORS[b.key];
          const isOpen = () => open() === b.key;
          return (
            <li class={{ open: isOpen() }}>
              <button class="bar" aria-expanded={`${isOpen()}`} onClick={() => setOpen(isOpen() ? null : b.key)}>
                <span class="bar-label">{f.label}</span>
                <span class="track" aria-hidden="true"><span class="fill" style={{ width: `${Math.round(b.value * 100)}%` }} /></span>
                <span class="bar-value"><span class="sr-only">, </span>{Math.round(b.value * 100)}<span class="sr-only"> out of 100. </span></span>
                <span class="link-btn">{isOpen() ? "Close" : "Adjust"}</span>
              </button>
              <Show when={isOpen()}>
                <div class="adjust">
                  <p><strong>{b.detail}.</strong> {f.question}</p>
                  <Show when={b.key === "slope"} fallback={<WeightSlider weight={f.weight} />}>
                    <HillsPanel />
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
