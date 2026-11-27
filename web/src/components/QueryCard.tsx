import { For, Show } from "solid-js";
import { MapPin, Search, LoaderCircle } from "@lucide/solid";
import {
  CLIMB_STEP_M, DISTANCE_PRESETS_KM, HILLINESS, climb, climbLabel, climbMaxM, climbTargetM, start, distanceKm, editQuery, findRoutes,
  queryOpen, setClimb, setDistanceKm, setStrictness, status, strictness, errorMsg, type Climb, type Hilliness,
} from "../state";
import { ScrubField } from "./ScrubField";
import { StartSearch, startLabel } from "./StartSearch";

const CLIMBS: { label: string; c: Climb }[] = [
  { label: "Off", c: "off" },
  ...(Object.keys(HILLINESS) as Hilliness[]).map((k) => ({ label: HILLINESS[k].label, c: { preset: k } as Climb })),
  { label: "Max", c: "max" },
];
const sameClimb = (a: Climb, b: Climb) =>
  typeof a === "string" || typeof b === "string" ? a === b : "preset" in a && "preset" in b && a.preset === b.preset;
/** Custom distances keep one decimal (16.5 km, ADR 0008). */
const km1 = (v: number) => Math.round(v * 10) / 10;

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
                  type="number" min="3" max="80" step="0.1" inputmode="decimal" aria-label="Custom distance in km"
                  placeholder="Other"
                  value={DISTANCE_PRESETS_KM.includes(distanceKm() as 10) ? "" : distanceKm()}
                  onInput={(e) => { const v = km1(Number(e.currentTarget.value)); if (v >= 3 && v <= 80) setDistanceKm(v); }}
                />
              </label>
            </div>
          </div>
          <button class="primary qgo" onClick={() => void findRoutes()} disabled={status() === "loading" || !start()} aria-busy={`${status() === "loading"}`}>
            <Show when={status() === "loading"} fallback={<><Search size={16} />Find loops</>}>
              <LoaderCircle size={16} class="spin" />Finding…
            </Show>
          </button>
          {/* The climb goal is part of the ask, like the distance (ADR 0008). */}
          <div class="qcell qclimb">
            <span class="qlabel">Climb</span>
            <div class="seg" role="radiogroup" aria-label="Climb goal">
              <For each={CLIMBS}>{(o) => (
                <button role="radio" aria-checked={`${sameClimb(climb(), o.c)}`} class={{ on: sameClimb(climb(), o.c) }} onClick={() => setClimb(o.c)}>{o.label}</button>
              )}</For>
            </div>
          </div>
          <Show when={climb() !== "off"}>
            <div class="qcell qclimb-x">
              <Show when={climb() !== "max"}>
                <label class="qpair"><span class="qlabel">Target, m</span>
                  <ScrubField label="Climb target in meters" value={climbTargetM() ?? 0} format={(v) => `${Math.round(v)} m`}
                    scale="linear" step={CLIMB_STEP_M} min={0} max={climbMaxM()} onCommit={(m) => setClimb({ m })} />
                </label>
              </Show>
              <label class="qpair"><span class="qlabel">Stick to it, 0–1</span>
                <ScrubField label="How closely to follow the climb goal, 0 loose to 1 strict" value={strictness()} format={(v) => v.toFixed(2)}
                  scale="linear" step={0.005} min={0} max={1} onCommit={setStrictness} />
              </label>
            </div>
          </Show>
        </div>
        <Show when={errorMsg()}>{(m) => {
          // Matches `MustPassTooFar`'s message in src/server.rs (D110).
          const need = () => Number(/need at least (\d+(?:\.\d)?) km/.exec(m())?.[1] ?? NaN);
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
