import { For, Show } from "solid-js";
import { X } from "@lucide/solid";
import { ScrubField } from "./ScrubField";
import { MAX_MINUTES, MIN_MINUTES } from "../routes/travel";
import { BASE_MAPS, LATER_OVERLAYS, OVERLAYS } from "../map/basemaps";
import { baseMap, origins, overlays, removeOrigin, setBaseMap, setOriginMinutes, toggleOverlay, travelError } from "../state";

const SWATCH: Record<string, string> = {
  outdoor: "linear-gradient(135deg,#f2efe9 0 40%,#dde4cf 40% 70%,#bfd4df 70%)",
  topo: "linear-gradient(135deg,#f1ecd9 0 45%,#d8c9a3 45% 52%,#e9e2cb 52%)",
  satellite: "linear-gradient(135deg,#55633f,#7b7a55 50%,#3d4a33)",
  relief: "linear-gradient(135deg,#f6f3ee 0 35%,#b9b0a6 60%,#7c726a)",
  trails: "repeating-linear-gradient(135deg,#f2efe9 0 7px,#e6b422 7px 9px,#f2efe9 9px 14px,#c8322a 14px 16px)",
  view: "linear-gradient(135deg,#28285a,#466eaa 35%,#fadc50 55%,#f58228 75%,#d72828)",
  schweizmobil: "repeating-linear-gradient(135deg,#f2efe9 0 8px,#1fae2d 8px 11px)",
  webcams: "radial-gradient(circle at 50% 50%,#1f2a33 0 30%,#ffffff 31% 42%,#dfe6ea 43%)",
  travel: "linear-gradient(135deg,rgba(31,78,140,.59) 0 50%,rgba(31,78,140,.27) 50%)",
  forest: "radial-gradient(circle at 30% 40%,#5f8a4c 0 30%,transparent 31%),radial-gradient(circle at 70% 65%,#5f8a4c 0 28%,#dde4cf 29%)",
};

export function LayersCard(props: { onPicked?: () => void }) {
  return (
    <section class="card layers" aria-label="Map layers">
      <h2 class="layers-title">Base map</h2>
      <div role="radiogroup" aria-label="Base map">
        <For each={BASE_MAPS}>{(b) => (
          <button role="radio" aria-checked={`${baseMap() === b.id}`} class={["layer", { on: baseMap() === b.id }]}
            onClick={() => { setBaseMap(b.id); props.onPicked?.(); }}>
            <span class="swatch" style={{ background: SWATCH[b.id] }} />{b.label}
          </button>
        )}</For>
      </div>

      <h2 class="layers-title overlays-title">Overlays</h2>
      <For each={OVERLAYS}>{(o) => (
        <button role="switch" aria-checked={`${overlays().includes(o.id)}`} class={["layer overlay", { active: overlays().includes(o.id) }]}
          onClick={() => toggleOverlay(o.id)}>
          <span class="swatch small" style={{ background: SWATCH[o.id] }} />{o.label}<span class="switch" aria-hidden="true" />
        </button>
      )}</For>
      <Show when={overlays().includes("travel")}><TravelOrigins /></Show>

      <p class="later-title">Coming later</p>
      <ul class="later-list">
        <For each={LATER_OVERLAYS}>{(l) => <li>{l.label}</li>}</For>
      </ul>
    </section>
  );
}

/** The travel origins under the Travel time switch (ADR 0002): number, coordinates, budget. */
function TravelOrigins() {
  return (
    <div class="travel" aria-label="Travel origins">
      <Show when={travelError()}>{(e) => <p class="travel-note warn">{e()}</p>}</Show>
      <Show when={origins().length} fallback={<p class="travel-note">Right-click or long-press the map: <b>Travel time from here</b>.</p>}>
        <p class="travel-note">Dark: every origin gets there. Light: only some do. Saturday, leaving 06:00–10:00.</p>
        <ol class="travel-list">
          <For each={origins()}>{(o, i) => (
            <li>
              <span class="travel-n">{i() + 1}</span>
              <span class="travel-at" title={`${o.lat.toFixed(5)}, ${o.lon.toFixed(5)}`}>{o.lat.toFixed(2)}, {o.lon.toFixed(2)}</span>
              <ScrubField value={o.minutes} label={`Time budget of travel origin ${i() + 1}`} format={(v) => `${v} min`}
                scale="linear" step={1} min={MIN_MINUTES} max={MAX_MINUTES} onCommit={(v) => setOriginMinutes(i(), v)} />
              <button class="travel-rm" aria-label={`Remove travel origin ${i() + 1}`} onClick={() => removeOrigin(i())}><X size={14} /></button>
            </li>
          )}</For>
        </ol>
      </Show>
    </div>
  );
}
