// Fine-grained settings (D51–D53): sliders that commit on release, so live regeneration (D55)
// runs once per gesture.
import { createSignal, For, Show } from "solid-js";
import {
  CLIMB_STEP_M, FACTORS, HILLINESS, climb, climbMaxM, climbTargetM, distanceKm, positionOf, setClimb, setHill,
  setStrictness, setWeight, strength, strictness, weightAt, weights, type Hilliness, type NumericWeight,
} from "../state";

const STEPS = 1000;

/** Range 0–1 with a filled track. Shows the drag live, commits on release. */
export function Slider(props: {
  value: number; label: string; text: (v: number) => string; onCommit: (v: number) => void;
  min?: number; notch?: number; ends?: [string, string];
}) {
  const [drag, setDrag] = createSignal<number | null>(null);
  const min = () => props.min ?? 0;
  const v = () => drag() ?? props.value;
  const pct = () => ((v() - min()) / (1 - min())) * 100;
  return (
    <div class="slider">
      <div class="slider-track" style={{ "--p": `${pct()}%`, "--notch": `${(((props.notch ?? -1) - min()) / (1 - min())) * 100}%` }}>
        <Show when={props.notch !== undefined}><span class="slider-notch" aria-hidden="true" /></Show>
        <input
          type="range" min={min() * STEPS} max={STEPS} step="1" value={Math.round(v() * STEPS)}
          aria-label={props.label} aria-valuetext={props.text(v())}
          onInput={(e) => setDrag(Number(e.currentTarget.value) / STEPS)}
          onChange={(e) => { const x = Number(e.currentTarget.value) / STEPS; setDrag(null); props.onCommit(x); }}
        />
      </div>
      <Show when={props.ends}>{(ends) => (
        <div class="slider-ends" aria-hidden="true"><span>{ends()[0]}</span><span>{ends()[1]}</span></div>
      )}</Show>
    </div>
  );
}

/** Off … Above all for one cost term. */
export function WeightSlider(props: { weight: NumericWeight; }) {
  const label = () => Object.values(FACTORS).find((f) => f.weight === props.weight)?.label ?? props.weight;
  return (
    <Slider
      label={label()} value={positionOf(weights()[props.weight])} notch={0.5}
      text={(t) => strength(weightAt(t))}
      ends={["Off", "Above all"]}
      onCommit={(t) => setWeight(props.weight, t < 0.02 ? 0 : Math.round(weightAt(t) * 100) / 100)}
    />
  );
}

const hillWord = (h: number) => (h < -0.66 ? "Gentle" : h < -0.2 ? "Fairly gentle" : h <= 0.2 ? "Any" : h <= 0.66 ? "Fairly steep" : "Steep");

/** Gentle … Steep for uphills or downhills (D53); stored −1…1, shown 0…1. */
export function HillSlider(props: { dir: "uphill" | "downhill"; label: string; }) {
  return (
    <div class="hill-row">
      <span class="hill-label">{props.label}</span>
      <Slider
        label={props.label} value={(weights()[props.dir] + 1) / 2} notch={0.5}
        text={(t) => hillWord(t * 2 - 1)}
        ends={["Gentle", "Steep"]}
        onCommit={(t) => setHill(props.dir, Math.round((t * 2 - 1) * 20) / 20)}
      />
    </div>
  );
}

/** The Hills adjust panel: how much hills count, then their direction. */
export function HillsPanel() {
  return (
    <>
      <HillSlider dir="uphill" label="Uphills" />
      <HillSlider dir="downhill" label="Downhills" />
      <div class="hill-row">
        <span class="hill-label">Counts</span>
        <WeightSlider weight="slope" />
      </div>
    </>
  );
}

const PRESETS = Object.entries(HILLINESS) as [Hilliness, { label: string; mPerKm: number }][];
const presetOn = (k: Hilliness) => { const c = climb(); return c !== "max" && "preset" in c && c.preset === k; };

/** Gentle / Hilly / Mountain / Max quick picks; they set the ruler below. */
export function ClimbPresets() {
  return (
    <div class="seg" role="radiogroup" aria-label="Hilliness">
      <For each={PRESETS}>{([k, h]) => (
        <button role="radio" aria-checked={`${presetOn(k)}`} class={{ on: presetOn(k) }}
          onClick={() => setClimb({ preset: k })} title={`About ${h.mPerKm} m of climb per km`}>{h.label}</button>
      )}</For>
      <button role="radio" aria-checked={`${climb() === "max"}`} class={{ on: climb() === "max" }}
        onClick={() => setClimb("max")} title="As much climb as the area has">Max</button>
    </div>
  );
}

/** How hard the generator chases the climb (D52). */
export function StrictnessSlider() {
  return (
    <div class="hill-row">
      <span class="hill-label">Stick to it</span>
      <Slider
        label="How closely to match the climb" value={strictness()} min={0}
        text={(s) => (s < 0.15 ? "Loosely" : s < 0.6 ? "Fairly closely" : s < 0.9 ? "Closely" : "As closely as possible")}
        ends={["Loosely", "Closely"]}
        onCommit={(s) => setStrictness(Math.round(s * 100) / 100)}
      />
    </div>
  );
}

/** A slider over 0…climbMax metres whose last stop is Max. */
export function ClimbSlider() {
  const maxM = climbMaxM;
  // The top 4 % of the track is the Max stop.
  const toPos = () => { const t = climbTargetM(); return t === null ? 1 : Math.min(t / maxM(), 1) * 0.96; };
  const fromPos = (p: number): number | null => (p > 0.98 ? null : Math.round(((p / 0.96) * maxM()) / CLIMB_STEP_M) * CLIMB_STEP_M);
  return (
    <Slider
      label="Metres of climb" value={toPos()}
      text={(p) => { const m = fromPos(p); return m === null ? "As much as possible" : `${m} m`; }}
      ends={["0 m", "Max"]}
      onCommit={(p) => { const m = fromPos(p); setClimb(m === null ? "max" : { m }); }}
    />
  );
}

/** The climb block in the query card. */
export function ClimbControl() {
  return (
    <>
      <ClimbPresets />
      <div class="climb-slider"><ClimbSlider /></div>
      <p class="hint">
        {climbTargetM() === null ? "As much climb as the area has" : `About ${climbTargetM()} m of climb over ${distanceKm()} km`}
      </p>
      <StrictnessSlider />
    </>
  );
}
