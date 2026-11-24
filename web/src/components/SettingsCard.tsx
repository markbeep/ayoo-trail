// Routing settings (D106, D107, D115): collapsed by default, then one collapsible group per topic,
// each with a plain sentence on what it does. Rows say in words what a value acts on.
import { createSignal, For, Show, type Element } from "solid-js";
import { ChevronDown } from "@lucide/solid";
import { getHealth } from "../api";
import type { Penalties } from "../routes/types";
import { MULT_MAX, MULT_MIN, multiplierOf, parseMultiplier, weightOf, word, type Term } from "../routes/multipliers";
import { DEFAULT_PENALTIES } from "../url";
import {
  CLIMB_STEP_M, DEFAULT_WEIGHTS, HILLINESS, clearAll, climb, climbMaxM, climbTargetM, maxSac, penalties, setClimb, setHill,
  setMaxSac, setPenalty, setStrictness, setWeight, strictness, weights, type Climb, type Hilliness, type NumericWeight,
} from "../state";
import { ScrubField } from "./ScrubField";

const [hasView, setHasView] = createSignal(false);
void getHealth().then((h) => setHasView(h.view)).catch(() => {});
const [open, setOpen] = createSignal(false);

const times = (m: number) => `×${Number(m.toPrecision(3))}`;

/** The six plain terms and what each acts on, in words (D106). */
const TERMS: { k: Term & NumericWeight; label: string; hint: string }[] = [
  { k: "surface", label: "Asphalt", hint: "Paved roads and paths" },
  { k: "official", label: "Marked trails", hint: "Signposted hiking trails" },
  { k: "forest", label: "Forest", hint: "Trails through woods" },
  { k: "urban", label: "Towns", hint: "Inside villages and towns" },
  { k: "traffic", label: "Busy roads", hint: "Roads with car traffic" },
  { k: "view", label: "Views", hint: "Spots with a wide view" },
];

function Row(props: { label: string; hint: string; children: Element; note?: string }) {
  return (
    <div class="set-row">
      <span class="set-text"><span class="set-label">{props.label}</span><span class="set-hint">{props.hint}</span></span>
      <span class="set-value">{props.children}<span class="set-note" data-w={props.note}>{props.note}</span></span>
    </div>
  );
}

function Group(props: { title: string; sub: string; changed: number; children: Element }) {
  return (
    <details class="set-group">
      <summary>
        <span class="set-text"><span class="set-gtitle">{props.title}</span><span class="set-hint">{props.sub}</span></span>
        <Show when={props.changed}><span class="set-badge">{props.changed} changed</span></Show>
        <ChevronDown size={16} class="set-chev" aria-hidden="true" />
      </summary>
      {props.children}
    </details>
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

const signed = (v: number) => (v > 0 ? `+${v.toFixed(2)}` : v < 0 ? `−${(-v).toFixed(2)}` : v.toFixed(2));
const shape = (v: number, lo: string, hi: string) => (v < -0.2 ? lo : v > 0.2 ? hi : "any");

function Shape(props: { k: "uphill" | "downhill" | "technical"; label: string }) {
  return (
    <ScrubField
      label={props.label} value={weights()[props.k]} format={signed} scale="linear" step={0.01} min={-1} max={1}
      onCommit={(v) => setHill(props.k, v)}
    />
  );
}

const PENALTY_ROWS: { k: keyof Penalties; label: string; hint: string; log: boolean; min: number; max: number; step: number; fmt: (v: number) => string; parse?: (s: string) => number | null }[] = [
  { k: "repeat", label: "Reuse", hint: "Cost of running a trail twice", log: true, min: 0.01, max: 1000, step: 0, fmt: times, parse: parseMultiplier },
  { k: "avoid_factor", label: "Avoid areas", hint: "Cost inside your soft avoid areas", log: true, min: 1, max: 1000, step: 0, fmt: times, parse: parseMultiplier },
  { k: "prefer_factor", label: "Prefer areas", hint: "Cost inside your soft prefer areas", log: true, min: 0.001, max: 1, step: 0, fmt: times, parse: parseMultiplier },
  { k: "stem_m", label: "Start stem", hint: "Out-and-back from the start that's free", log: false, min: 0, max: 5000, step: 10, fmt: (v) => `${Math.round(v)} m` },
  {
    k: "max_overlap", label: "Overlap", hint: "How much two suggested loops may share", log: false, min: 0, max: 1, step: 0.005, fmt: (v) => `${Math.round(v * 100)} %`,
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

/** How many of these weights differ from their defaults, for the group's "n changed" badge. */
const moved = (ks: (keyof typeof DEFAULT_WEIGHTS)[]) => ks.filter((k) => weights()[k] !== DEFAULT_WEIGHTS[k]).length;

export function SettingsCard() {
  const changed = {
    ground: () => moved(TERMS.map((t) => t.k)),
    hills: () => moved(["uphill", "downhill", "slope"]),
    tech: () => moved(["technical", "tech"]) + (maxSac() !== 3 ? 1 : 0),
    climb: () => (sameClimb(climb(), { preset: "hilly" }) ? 0 : 1) + (strictness() !== 0.8 ? 1 : 0),
    shape: () => PENALTY_ROWS.filter((p) => penalties()[p.k] !== DEFAULT_PENALTIES[p.k]).length,
  };
  const total = () => Object.values(changed).reduce((n, f) => n + f(), 0);

  return (
    <section class={["card settings", { open: open() }]} aria-label="Settings">
      <header class="set-head">
        <button class="set-toggle" aria-expanded={`${open()}`} onClick={() => setOpen(!open())}>
          <span class="set-text">
            <h2>Settings</h2>
            <span class="set-hint">What loops prefer and avoid{total() ? ` · ${total()} changed` : ""}</span>
          </span>
          <ChevronDown size={18} class="set-chev" aria-hidden="true" />
        </button>
        <Show when={open()}>
          <button class="link-btn" onClick={clearAll} title="All costs off, no climb goal, no areas: only the distance counts">Clear all</button>
        </Show>
      </header>

      <Show when={open()}>
        <p class="set-intro">Drag a value sideways or tap it to type. <b>×1</b> means it doesn't matter, above ×1 avoids, below ×1 seeks out.</p>

        <Group title="Ground" sub="Which kinds of trail and surroundings to favour" changed={changed.ground()}>
          <For each={TERMS.filter((t) => t.k !== "view" || hasView())}>{(t) => (
            <Row label={t.label} hint={t.hint} note={word(t.k, weights()[t.k])}><Mult k={t.k} label={`${t.label}: ${t.hint}`} /></Row>
          )}</For>
        </Group>

        <Group title="Hills" sub="Gentle or steep slopes, and how much that counts" changed={changed.hills()}>
          <Row label="Uphill" hint="−1 gentle · +1 steep" note={shape(weights().uphill, "gentle", "steep")}><Shape k="uphill" label="Uphill: −1 gentle, +1 steep" /></Row>
          <Row label="Downhill" hint="−1 gentle · +1 steep" note={shape(weights().downhill, "gentle", "steep")}><Shape k="downhill" label="Downhill: −1 gentle, +1 steep" /></Row>
          <Row label="Strength" hint="How much the slope choice counts" note={weights().slope === 0 ? "off" : undefined}><Mult k="slope" label="Hills strength" min={1} /></Row>
        </Group>

        <Group title="Technical" sub="How hard the trails may get (SAC grades T1–T6)" changed={changed.tech()}>
          <div class="set-row wide">
            <span class="set-text"><span class="set-label">Hardest allowed</span><span class="set-hint">T1 easy hiking … T6 difficult alpine</span></span>
            <span class="seg" role="radiogroup" aria-label="Hardest trail allowed">
              <For each={[1, 2, 3, 4, 5, 6]}>{(s) => (
                <button role="radio" aria-checked={`${maxSac() === s}`} class={{ on: maxSac() === s }} onClick={() => setMaxSac(s)}>T{s}</button>
              )}</For>
            </span>
          </div>
          <Row label="Harder grades" hint="−1 avoid · +1 seek out" note={shape(weights().technical, "avoid", "seek")}><Shape k="technical" label="Harder grades: −1 avoid, +1 seek" /></Row>
          <Row label="Strength" hint="How much that counts on T4+ trails" note={weights().tech === 0 ? "off" : undefined}><Mult k="tech" label="Technical strength" min={1} /></Row>
        </Group>

        <Group title="Climb" sub="Total climb to aim for" changed={changed.climb()}>
          <div class="set-row wide">
            <span class="seg" role="radiogroup" aria-label="Climb goal">
              <For each={CLIMBS}>{(o) => (
                <button role="radio" aria-checked={`${sameClimb(climb(), o.c)}`} class={{ on: sameClimb(climb(), o.c) }} onClick={() => setClimb(o.c)}>{o.label}</button>
              )}</For>
            </span>
          </div>
          <Show when={climb() !== "off" && climb() !== "max"}>
            <Row label="Target" hint="Meters of climb over the loop">
              <ScrubField label="Climb target in meters" value={climbTargetM() ?? 0} format={(v) => `${Math.round(v)} m`}
                scale="linear" step={CLIMB_STEP_M} min={0} max={climbMaxM()} onCommit={(m) => setClimb({ m })} />
            </Row>
          </Show>
          <Show when={climb() !== "off"}>
            <Row label="Stick to it" hint="0 loose · 1 strict" note={climb() === "max" ? "push" : "match"}>
              <ScrubField label="How closely to follow the climb goal, 0 loose to 1 strict" value={strictness()} format={(v) => v.toFixed(2)}
                scale="linear" step={0.005} min={0} max={1} onCommit={setStrictness} />
            </Row>
          </Show>
        </Group>

        <Group title="Loop shape" sub="Reusing trails, your map areas, overlap between loops" changed={changed.shape()}>
          <For each={PENALTY_ROWS}>{(p) => (
            <Row label={p.label} hint={p.hint}>
              <ScrubField label={`${p.label}: ${p.hint}`} value={penalties()[p.k]} format={p.fmt} parse={p.parse}
                scale={p.log ? "log" : "linear"} step={p.step} min={p.min} max={p.max} onCommit={(v) => setPenalty(p.k, v)} />
            </Row>
          )}</For>
        </Group>
      </Show>
    </section>
  );
}
