// Every routing setting in one dense table (D106, D107): typed multipliers you scrub or type,
// the hill and technical shapes, the climb goal and the loop-shaping penalties.
import { createSignal, For, Show, type Element } from "solid-js";
import { getHealth } from "../api";
import type { Penalties } from "../routes/types";
import { KEY, MULT_MAX, MULT_MIN, multiplierOf, parseMultiplier, weightOf, word, type Term } from "../routes/multipliers";
import {
  CLIMB_STEP_M, HILLINESS, clearAll, climb, climbMaxM, climbTargetM, maxSac, penalties, setClimb, setHill, setMaxSac,
  setPenalty, setStrictness, setWeight, strictness, weights, type Climb, type Hilliness, type NumericWeight,
} from "../state";
import { ScrubField } from "./ScrubField";

const [hasView, setHasView] = createSignal(false);
void getHealth().then((h) => setHasView(h.view)).catch(() => {});

const times = (m: number) => `×${Number(m.toPrecision(3))}`;

/** The six plain terms: what each acts on (D106). */
const TERMS: { k: Term & NumericWeight; label: string; on: string }[] = [
  { k: "surface", label: "Surface", on: "asphalt" },
  { k: "official", label: "Marked trails", on: "marked" },
  { k: "forest", label: "Forest", on: "full forest" },
  { k: "urban", label: "Towns", on: "in town" },
  { k: "traffic", label: "Busy roads", on: "busy road" },
  { k: "view", label: "Views", on: "wide view" },
];

function Row(props: { label: string; on?: string; children: Element; note?: string }) {
  return (
    <div class="set-row">
      <span class="set-label">{props.label}</span>
      <span class="set-on">{props.on}</span>
      <span class="set-field">{props.children}</span>
      <span class="set-note">{props.note}</span>
    </div>
  );
}

function Mult(props: { k: Term & NumericWeight; label: string; min?: number }) {
  return (
    <ScrubField
      label={props.label} value={multiplierOf(props.k, weights()[props.k])} format={times} parse={parseMultiplier}
      scale="log" step={0} min={props.min ?? MULT_MIN} max={MULT_MAX}
      onCommit={(m) => setWeight(props.k, weightOf(props.k, m))}
    />
  );
}

const signed = (v: number) => (v > 0 ? `+${v.toFixed(2)}` : v.toFixed(2));
const shape = (v: number, lo: string, hi: string) => (v < -0.2 ? lo : v > 0.2 ? hi : "any");

function Shape(props: { k: "uphill" | "downhill" | "technical"; label: string }) {
  return (
    <ScrubField
      label={props.label} value={weights()[props.k]} format={signed} scale="linear" step={0.01} min={-1} max={1}
      onCommit={(v) => setHill(props.k, v)}
    />
  );
}

const PENALTY_ROWS: { k: keyof Penalties; label: string; on: string; log: boolean; min: number; max: number; step: number; fmt: (v: number) => string; parse?: (s: string) => number | null }[] = [
  { k: "repeat", label: "Repeat", on: "used trail", log: true, min: 0.01, max: 1000, step: 0, fmt: times, parse: parseMultiplier },
  { k: "avoid_factor", label: "Avoid areas", on: "soft avoid", log: true, min: 1, max: 1000, step: 0, fmt: times, parse: parseMultiplier },
  { k: "prefer_factor", label: "Prefer areas", on: "soft prefer", log: true, min: 0.001, max: 1, step: 0, fmt: times, parse: parseMultiplier },
  { k: "stem_m", label: "Start stem", on: "free reuse", log: false, min: 0, max: 5000, step: 10, fmt: (v) => `${Math.round(v)} m` },
  {
    k: "max_overlap", label: "Overlap", on: "between loops", log: false, min: 0, max: 1, step: 0.005, fmt: (v) => `${Math.round(v * 100)} %`,
    parse: (s) => { const n = Number(s.replace(/[%\s]/g, "")); return s.trim() !== "" && Number.isFinite(n) ? n / 100 : null; },
  },
];

const CLIMBS: { label: string; c: Climb }[] = [
  { label: "Off", c: "off" },
  ...(Object.keys(HILLINESS) as Hilliness[]).map((k) => ({ label: HILLINESS[k].label, c: { preset: k } as Climb })),
  { label: "Max", c: "max" },
];
const sameClimb = (a: Climb, b: Climb) =>
  typeof a === "string" || typeof b === "string" ? a === b : "preset" in a && "preset" in b && a.preset === b.preset;

export function SettingsCard() {
  return (
    <section class="card settings" aria-label="Settings">
      <header class="set-head">
        <h2>Settings</h2>
        <button class="link-btn" onClick={clearAll} title="All costs off, no climb goal, no areas: only the distance counts">Clear all</button>
      </header>

      <h3 class="set-group">Preferences <span>below ×1 prefers, above avoids</span></h3>
      <For each={TERMS.filter((t) => t.k !== "view" || hasView())}>{(t) => (
        <Row label={t.label} on={t.on} note={word(t.k, weights()[t.k])}><Mult k={t.k} label={`${t.label}, multiplier on ${t.on}`} /></Row>
      )}</For>

      <h3 class="set-group">Hills <span>−1 gentle … +1 steep</span></h3>
      <Row label="Uphill" note={shape(weights().uphill, "gentle", "steep")}><Shape k="uphill" label="Uphill shape" /></Row>
      <Row label="Downhill" note={shape(weights().downhill, "gentle", "steep")}><Shape k="downhill" label="Downhill shape" /></Row>
      <Row label="Strength" on={`×${KEY.slope} band`} note={weights().slope === 0 ? "off" : undefined}><Mult k="slope" label="Hills strength" min={1} /></Row>

      <h3 class="set-group">Technical <span>−1 avoid … +1 seek</span></h3>
      <Row label="Hardest">
        <span class="seg tight" role="radiogroup" aria-label="Hardest trail">
          <For each={[1, 2, 3, 4, 5, 6]}>{(s) => (
            <button role="radio" aria-checked={`${maxSac() === s}`} class={{ on: maxSac() === s }} onClick={() => setMaxSac(s)}>T{s}</button>
          )}</For>
        </span>
      </Row>
      <Row label="Harder grades" note={shape(weights().technical, "avoid", "seek")}><Shape k="technical" label="Technical trails" /></Row>
      <Row label="Strength" on="T4+" note={weights().tech === 0 ? "off" : undefined}><Mult k="tech" label="Technical strength" min={1} /></Row>

      <h3 class="set-group">Climb</h3>
      <div class="set-row wide">
        <span class="seg tight" role="radiogroup" aria-label="Climb goal">
          <For each={CLIMBS}>{(o) => (
            <button role="radio" aria-checked={`${sameClimb(climb(), o.c)}`} class={{ on: sameClimb(climb(), o.c) }} onClick={() => setClimb(o.c)}>{o.label}</button>
          )}</For>
        </span>
      </div>
      <Show when={climb() !== "off" && climb() !== "max"}>
        <Row label="Target">
          <ScrubField label="Climb target in meters" value={climbTargetM() ?? 0} format={(v) => `${Math.round(v)} m`}
            scale="linear" step={CLIMB_STEP_M} min={0} max={climbMaxM()} onCommit={(m) => setClimb({ m })} />
        </Row>
      </Show>
      <Show when={climb() !== "off"}>
        <Row label="Stick to it" note={climb() === "max" ? "push" : "match"}>
          <ScrubField label="How closely to follow the climb goal" value={strictness()} format={(v) => v.toFixed(2)}
            scale="linear" step={0.005} min={0} max={1} onCommit={setStrictness} />
        </Row>
      </Show>

      <h3 class="set-group">Loop shape</h3>
      <For each={PENALTY_ROWS}>{(p) => (
        <Row label={p.label} on={p.on}>
          <ScrubField label={`${p.label}, ${p.on}`} value={penalties()[p.k]} format={p.fmt} parse={p.parse}
            scale={p.log ? "log" : "linear"} step={p.step} min={p.min} max={p.max} onCommit={(v) => setPenalty(p.k, v)} />
        </Row>
      )}</For>
    </section>
  );
}
