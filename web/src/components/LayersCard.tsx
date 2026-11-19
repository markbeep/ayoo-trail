import { For } from "solid-js";
import { BASE_MAPS, LATER_OVERLAYS, OVERLAYS } from "../map/basemaps";
import { baseMap, overlays, setBaseMap, toggleOverlay } from "../state";

const SWATCH: Record<string, string> = {
  outdoor: "linear-gradient(135deg,#f2efe9 0 40%,#dde4cf 40% 70%,#bfd4df 70%)",
  topo: "linear-gradient(135deg,#f1ecd9 0 45%,#d8c9a3 45% 52%,#e9e2cb 52%)",
  satellite: "linear-gradient(135deg,#55633f,#7b7a55 50%,#3d4a33)",
  relief: "linear-gradient(135deg,#f6f3ee 0 35%,#b9b0a6 60%,#7c726a)",
  trails: "repeating-linear-gradient(135deg,#f2efe9 0 7px,#e6b422 7px 9px,#f2efe9 9px 14px,#c8322a 14px 16px)",
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

      <p class="later-title">Coming later</p>
      <ul class="later-list">
        <For each={LATER_OVERLAYS}>{(l) => <li>{l.label}</li>}</For>
      </ul>
    </section>
  );
}
