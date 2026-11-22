import { For, Show } from "solid-js";
import { MapPin, Search, LoaderCircle } from "@lucide/solid";
import {
  DISTANCE_PRESETS_KM, climbLabel, start, distanceKm, editQuery, findRoutes, queryOpen, setDistanceKm, status, errorMsg,
} from "../state";
import { StartSearch, startLabel } from "./StartSearch";

export function QueryCard() {
  return (
    <section class="card query" aria-label="Route search">
      <Show when={queryOpen()} fallback={
        <div class="chips-row">
          <button class="chip" onClick={editQuery}><MapPin size={15} /><span class="chip-text">{startLabel()}</span></button>
          <button class="chip" onClick={editQuery}>{distanceKm()} km</button>
          <button class="chip" onClick={editQuery}>{climbLabel()}</button>
          <button class="link-btn push" onClick={editQuery}>Change</button>
        </div>
      }>
        {/* One ruled grid: start across, then distance and Find loops; two rows because the panel is 400 px wide. */}
        <div class="qgrid">
          <div class="qcell qstart"><span class="qlabel">Start</span><StartSearch /></div>
          <div class="qcell qkm">
            <span class="qlabel">Distance, km</span>
            <div class="seg" role="radiogroup" aria-label="Distance in km">
              <For each={DISTANCE_PRESETS_KM}>{(d) => (
                <button role="radio" aria-checked={`${distanceKm() === d}`} class={{ on: distanceKm() === d }} onClick={() => setDistanceKm(d)}>{d}</button>
              )}</For>
              <label class={["seg-input", { on: !DISTANCE_PRESETS_KM.includes(distanceKm() as 10) }]}>
                <input
                  type="number" min="3" max="80" step="1" inputmode="numeric" aria-label="Custom distance in km"
                  placeholder="Other"
                  value={DISTANCE_PRESETS_KM.includes(distanceKm() as 10) ? "" : distanceKm()}
                  onInput={(e) => { const v = Number(e.currentTarget.value); if (v >= 3 && v <= 80) setDistanceKm(v); }}
                />
              </label>
            </div>
          </div>
          <button class="primary qgo" onClick={() => void findRoutes()} disabled={status() === "loading" || !start()} aria-busy={`${status() === "loading"}`}>
            <Show when={status() === "loading"} fallback={<><Search size={16} />Find loops</>}>
              <LoaderCircle size={16} class="spin" />Finding…
            </Show>
          </button>
        </div>
        <Show when={errorMsg()}>{(m) => {
          // Matches `MustPassTooFar`'s message in src/server.rs (D110).
          const need = () => Number(/need at least (\d+) km/.exec(m())?.[1] ?? NaN);
          return (
            <p class="query-error" role="alert">{m()}
              <Show when={need() <= 80}>
                {" "}<button class="link-btn" onClick={() => { setDistanceKm(need()); void findRoutes(); }}>Use {need()} km</button>
              </Show>
            </p>
          );
        }}</Show>
      </Show>
    </section>
  );
}
