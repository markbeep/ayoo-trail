// Fine-grained settings (D51–D53): sliders that commit on release, so live regeneration (D55)
// runs once per gesture.
import { createSignal, For, Show } from "solid-js";
import {
  CLIMB_STEP_M, FACTORS, HILLINESS, climbMaxM, maxSac, setMaxSac, climbTargetM, distanceKm, positionOf, setClimb, setHill,
  setStrictness, setWeight, strength, strictness, weightAt, weights, type Climb, type Hilliness, type NumericWeight,
} from "../state";

const STEPS = 1000;

/** Range 0–1 with a filled track. Shows the drag live, commits on release. */
export function Slider(props: {
  value: number; label: string; text: (v: number) => string; onCommit: (v: number) => void;
  min?: number; notch?: number; ends?: [string, string];
  /** Labelled notches under the track, positions 0–1. */
  marks?: { at: number; label: string }[];
}) {
  const [drag, setDrag] = createSignal<number | null>(null);
  const min = () => props.min ?? 0;
  const v = () => drag() ?? props.value;
  const pct = () => ((v() - min()) / (1 - min())) * 100;
  return (
    <div class="slider">
      <div class="slider-track" style={{ "--p": `${pct()}%`, "--notch": `${(((props.notch ?? -1) - min()) / (1 - min())) * 100}%` }}>
        <Show when={props.notch !== undefined}><span class="slider-notch" aria-hidden="true" /></Show>
        <For each={props.marks}>{(m) => <span class="slider-notch" style={{ "--notch": `${m.at * 100}%` }} aria-hidden="true" />}</For>
        <input
          type="range" min={min() * STEPS} max={STEPS} step="1" value={Math.round(v() * STEPS)}
          aria-label={props.label} aria-valuetext={props.text(v())}
          onInput={(e) => setDrag(Number(e.currentTarget.value) / STEPS)}
          onChange={(e) => { const x = Number(e.currentTarget.value) / STEPS; setDrag(null); props.onCommit(x); }}
        />
      </div>
      <Show when={props.marks}>{(marks) => (
        <div class="slider-marks" aria-hidden="true">
          <For each={marks()}>{(m) => <span style={{ "--at": `${m.at * 100}%`, "--shift": m.at <= 0 ? "0" : m.at >= 1 ? "-100%" : "-50%" }}>{m.label}</span>}</For>
        </div>
      )}</Show>
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

const SAC_LABELS = ["T1", "T2", "T3", "T4", "T5", "T6"];
const SAC_NAMES = ["T1 hiking", "T2 mountain hiking", "T3 demanding mountain hiking", "T4 alpine hiking", "T5 demanding alpine hiking", "T6 difficult alpine hiking"];
const techWord = (t: number) => (t < -0.66 ? "Avoid" : t < -0.2 ? "Rather avoid" : t <= 0.2 ? "Don't mind" : t <= 0.66 ? "Rather seek" : "Seek out");

/** The Technical terrain adjust panel (D81): a hard grade limit, then avoid … seek below it, then how much it counts. */
export function TechPanel() {
  const at = (sac: number) => (sac - 1) / 5;
  const toSac = (p: number) => Math.round(p * 5) + 1;
  return (
    <>
      <div class="hill-row">
        <span class="hill-label">Hardest trail</span>
        <Slider
          label="Hardest trail" value={at(maxSac())}
          marks={SAC_LABELS.map((label, i) => ({ at: at(i + 1), label }))}
          text={(p) => SAC_NAMES[toSac(p) - 1]!}
          onCommit={(p) => setMaxSac(toSac(p))}
        />
      </div>
      <div class="hill-row">
        <span class="hill-label">Technical trails</span>
        <Slider
          label="Technical trails" value={(weights().technical + 1) / 2} notch={0.5}
          text={(t) => techWord(t * 2 - 1)}
          ends={["Avoid", "Seek out"]}
          onCommit={(t) => setHill("technical", Math.round((t * 2 - 1) * 20) / 20)}
        />
      </div>
      <div class="hill-row">
        <span class="hill-label">Counts</span>
        <WeightSlider weight="tech" />
      </div>
    </>
  );
}

/** How hard the generator chases the climb (D52). */
export function StrictnessSlider() {
  const max = () => climbTargetM() === null;
  const question = () => (max() ? "How hard should loops push for climb?" : `How closely should loops match ${climbTargetM()} m of climb?`);
  return (
    <div class="strictness">
      <p class="control-q">{question()}</p>
      <Slider
        label={question()} value={strictness()} min={0}
        text={(s) => max()
          ? (s < 0.15 ? "A little" : s < 0.6 ? "Somewhat" : s < 0.9 ? "Hard" : "As hard as possible")
          : (s < 0.15 ? "Roughly" : s < 0.6 ? "Fairly closely" : s < 0.9 ? "Closely" : "As exactly as possible")}
        ends={max() ? ["A little", "As hard as possible"] : ["Roughly", "Exactly"]}
        onCommit={(s) => setStrictness(Math.round(s * 100) / 100)}
      />
    </div>
  );
}

const PRESETS = Object.entries(HILLINESS) as [Hilliness, { label: string; mPerKm: number }][];
/** Thumb drops this close to a notch snap to it. */
const SNAP = 0.03;

/** One slider over 0…climbMax meters: Gentle, Hilly and Mountain are notches that follow the distance, Max is the end (D78). */
export function ClimbSlider() {
  const maxM = climbMaxM;
  // The top 4 % of the track is the Max stop.
  const posOf = (m: number) => Math.min(m / maxM(), 1) * 0.96;
  const marks = () => [
    ...PRESETS.map(([k, h]) => ({ k, at: posOf(h.mPerKm * distanceKm()), label: h.label })),
    { k: "max" as const, at: 1, label: "Max" },
  ];
  const toPos = () => { const t = climbTargetM(); return t === null ? 1 : posOf(t); };
  const fromPos = (p: number): Climb => {
    const near = marks().find((m) => Math.abs(m.at - p) <= SNAP);
    if (near) return near.k === "max" ? "max" : { preset: near.k };
    return { m: Math.round(((p / 0.96) * maxM()) / CLIMB_STEP_M) * CLIMB_STEP_M };
  };
  const word = (c: Climb) => c === "max" ? "As much as possible" : "preset" in c ? `${HILLINESS[c.preset].label}, ${HILLINESS[c.preset].mPerKm * distanceKm()} m` : `${c.m} m`;
  return (
    <Slider
      label="Climb" value={toPos()} marks={marks()}
      text={(p) => word(fromPos(p))}
      onCommit={(p) => setClimb(fromPos(p))}
    />
  );
}

/** The climb block in the query card. */
export function ClimbControl() {
  return (
    <>
      <div class="climb-slider"><ClimbSlider /></div>
      <p class="hint">
        {climbTargetM() === null ? "As much climb as the area has" : `About ${climbTargetM()} m of climb over ${distanceKm()} km`}
      </p>
      <StrictnessSlider />
    </>
  );
}
