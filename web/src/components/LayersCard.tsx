import { For } from "solid-js";
import { BASE_MAPS } from "../map/basemaps";
import { baseMap, setBaseMap } from "../state";

/** Planned layers, shown so the list already has its future shape. Not selectable. */
const LATER = [
  { id: "snow", label: "Snow depth", swatch: "linear-gradient(135deg,#f4f8fc,#bcd3ea)" },
  { id: "heat", label: "Popular trails", swatch: "linear-gradient(135deg,#f6dcc0,#e0814f)" },
];

const SWATCH: Record<string, string> = {
  topo: "linear-gradient(135deg,#f1ecd9 0 45%,#d8c9a3 45% 52%,#e9e2cb 52%)",
  satellite: "linear-gradient(135deg,#55633f,#7b7a55 50%,#3d4a33)",
};

export function LayersCard(props: { onPicked?: () => void }) {
  return (
    <section class="card layers" aria-label="Map layers">
      <h2 class="layers-title">Map layers</h2>
      <div role="radiogroup" aria-label="Base map">
        <For each={BASE_MAPS}>{(b) => (
          <button role="radio" class="layer" aria-checked={baseMap() === b.id} classList={{ on: baseMap() === b.id }}
            onClick={() => { setBaseMap(b.id); props.onPicked?.(); }}>
            <span class="swatch" style={{ background: SWATCH[b.id] }} />{b.label}
          </button>
        )}</For>
      </div>
      <For each={LATER}>{(l) => (
        <div class="layer later" aria-disabled="true">
          <span class="swatch" style={{ background: l.swatch }} />{l.label}<small>Later</small>
        </div>
      )}</For>
    </section>
  );
}
