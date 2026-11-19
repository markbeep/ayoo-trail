import { For, Show } from "solid-js";
import { MapPin, Crosshair, Search, LoaderCircle } from "@lucide/solid";
import {
  DISTANCE_PRESETS_KM, climbLabel, distanceKm, findRoutes, pickingStart, queryOpen,
  setDistanceKm, setPickingStart, setQueryOpen, start, status, DEFAULT_START, errorMsg,
} from "../state";
import { ClimbControl } from "./Controls";
import { coord } from "../routes/format";

function startLabel() {
  const s = start();
  return s.lat === DEFAULT_START.lat && s.lon === DEFAULT_START.lon ? "Wattwil" : coord(s);
}

export function QueryCard() {
  return (
    <section class="card query" aria-label="Route search">
      <Show when={queryOpen()} fallback={
        <div class="chips-row">
          <button class="chip" onClick={() => setQueryOpen(true)}><MapPin size={15} /><span class="chip-text">{startLabel()}</span></button>
          <button class="chip" onClick={() => setQueryOpen(true)}>{distanceKm()} km</button>
          <button class="chip" onClick={() => setQueryOpen(true)}>{climbLabel()}</button>
          <button class="link-btn push" onClick={() => setQueryOpen(true)}>Change</button>
        </div>
      }>
        <h2 class="q-title">Where do you start?</h2>
        <button
          class={["field start-field", { active: pickingStart() }]}
          onClick={() => setPickingStart(!pickingStart())}
          aria-pressed={`${pickingStart()}`}
        >
          <MapPin size={18} />
          <span class="grow">{pickingStart() ? "Tap the map to set your start" : startLabel()}</span>
          <Crosshair size={18} class="muted-icon" />
        </button>

        <h2 class="q-title">How far?</h2>
        <div class="seg" role="radiogroup" aria-label="Distance">
          <For each={DISTANCE_PRESETS_KM}>{(d) => (
            <button role="radio" aria-checked={`${distanceKm() === d}`} class={{ on: distanceKm() === d }} onClick={() => setDistanceKm(d)}>{d} km</button>
          )}</For>
          <label class={["seg-input", { on: !DISTANCE_PRESETS_KM.includes(distanceKm() as 10) }]}>
            <input
              type="number" min="3" max="60" step="1" inputmode="numeric" aria-label="Custom distance in km"
              placeholder="Other"
              value={DISTANCE_PRESETS_KM.includes(distanceKm() as 10) ? "" : distanceKm()}
              onInput={(e) => { const v = Number(e.currentTarget.value); if (v >= 3 && v <= 60) setDistanceKm(v); }}
            />
          </label>
        </div>

        <h2 class="q-title">How hilly?</h2>
        <ClimbControl />

        <button class="primary" onClick={() => void findRoutes()} disabled={status() === "loading"}>
          <Show when={status() === "loading"} fallback={<><Search size={18} />Find loops</>}>
            <LoaderCircle size={18} class="spin" />Finding loops…
          </Show>
        </button>
        <Show when={errorMsg()}>{(m) => <p class="query-error" role="alert">{m()}</p>}</Show>
      </Show>
    </section>
  );
}
