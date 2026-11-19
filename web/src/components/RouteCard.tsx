import { createSignal, For, Show } from "solid-js";
import { Download, RefreshCw, TriangleAlert } from "@lucide/solid";
import type { BreakdownKey, Route } from "../routes/types";
import { FACTORS, HILL_CHOICES, findRoutes, setHill, setWeight, stale, weights } from "../state";
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

      <Show when={stale()}>
        <div class="stale">
          <span>Settings changed</span>
          <button class="link-btn" onClick={() => void findRoutes()}><RefreshCw size={14} />Find loops again</button>
        </div>
      </Show>

      <h3 class="why">Why it scores {score(r().score)}</h3>
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
                  <Show when={b.key === "slope"} fallback={
                    <div class="seg on-white" role="radiogroup" aria-label={f.label}>
                      <For each={f.choices}>{(c) => (
                        <button role="radio" aria-checked={`${weights()[f.weight] === c.value}`}
                          class={{ on: weights()[f.weight] === c.value }}
                          onClick={() => setWeight(f.weight, c.value)}>{c.label}</button>
                      )}</For>
                    </div>
                  }>
                    <For each={[["uphill", "Uphills"], ["downhill", "Downhills"]] as const}>{([k, label]) => (
                      <div class="hill-row">
                        <span class="hill-label">{label}</span>
                        <div class="seg on-white" role="radiogroup" aria-label={label}>
                          <For each={HILL_CHOICES}>{(c) => (
                            <button role="radio" aria-checked={`${weights()[k] === c.value}`}
                              class={{ on: weights()[k] === c.value }}
                              onClick={() => setHill(k, c.value)}>{c.label}</button>
                          )}</For>
                        </div>
                      </div>
                    )}</For>
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
