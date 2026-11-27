// Preferences under the score bars (ADR 0008, D27's "explain, then adjust"): one row per bar, named
// as the bar; tapping a row opens its controls inline. Without a route the same rows show without bars.
import { createSignal, For, Show, type Element } from "solid-js";
import { ChevronDown } from "@lucide/solid";
import { getHealth } from "../api";
import type { BreakdownItem, BreakdownKey, Penalties, TermKey } from "../routes/types";
import { MULT_MAX, MULT_MIN, multiplierOf, parseMultiplier, weightOf, word } from "../routes/multipliers";
import {
  FACTORS, clearAll, maxSac, penalties, resetDefaults, setHill, setMaxSac, setPenalty, setTlmOnly, setWeight, tlmOnly, weights,
  type NumericWeight,
} from "../state";
import { ScrubField } from "./ScrubField";

const [hasView, setHasView] = createSignal(false);
void getHealth().then((h) => setHasView(h.view)).catch(() => {});
/** The one open row, kept across route changes so a tweak-and-refresh keeps its panel open. */
const [openKey, setOpenKey] = createSignal<TermKey | null>(null);

const times = (m: number) => `×${Number(m.toPrecision(3))}`;
const signed = (v: number) => (v > 0 ? `+${v.toFixed(2)}` : v < 0 ? `−${(-v).toFixed(2)}` : v.toFixed(2));

const ORDER: TermKey[] = ["official", "surface", "slope", "tech", "forest", "urban", "traffic", "view"];
/** What each row's control acts on: the × rows' control label, the shape rows' hint line. */
const ACTS_ON: Record<TermKey, string> = {
  official: "× on signposted hiking trails",
  surface: "× on asphalt",
  slope: "Steepness: −1 gentle, +1 steep",
  tech: "Trails T4 and harder: −1 avoid, +1 seek",
  forest: "× on trails through woods",
  urban: "× inside villages and towns",
  traffic: "× on roads with car traffic",
  view: "× at spots with a wide view",
};

function Mult(props: { k: TermKey & NumericWeight; label: string; min?: number }) {
  return (
    <ScrubField
      label={props.label} value={multiplierOf(props.k, weights()[props.k])} format={times} parse={parseMultiplier}
      scale="log" step={0} min={props.min ?? MULT_MIN} max={MULT_MAX}
      onCommit={(m) => setWeight(props.k, weightOf(props.k, m))}
    />
  );
}

function Shape(props: { k: "uphill" | "downhill" | "technical"; label: string }) {
  return (
    <ScrubField
      label={props.label} value={weights()[props.k]} format={signed} scale="linear" step={0.01} min={-1} max={1}
      onCommit={(v) => setHill(props.k, v)}
    />
  );
}

/** One control line in an open panel: name left, control right. */
function Ctl(props: { label: string; children: Element }) {
  return <div class="pref-ctl"><span>{props.label}</span>{props.children}</div>;
}

function Panel(props: { k: TermKey }) {
  const w = () => weights();
  switch (props.k) {
    case "official":
      return (
        <>
          <Ctl label={ACTS_ON.official}><Mult k="official" label={ACTS_ON.official} /></Ctl>
          <Ctl label="Hardest grade">
            <span class="seg sac" role="radiogroup" aria-label="Hardest trail grade allowed">
              <For each={[1, 2, 3, 4, 5, 6]}>{(s) => (
                <button role="radio" aria-checked={`${maxSac() === s}`} class={{ on: maxSac() === s }} onClick={() => setMaxSac(s)}>T{s}</button>
              )}</For>
            </span>
          </Ctl>
          <button role="switch" aria-checked={`${tlmOnly()}`} class={["pref-switch", { active: tlmOnly() }]} onClick={() => setTlmOnly(!tlmOnly())}>
            Only ways on swisstopo's map<span class="switch" aria-hidden="true" />
          </button>
        </>
      );
    case "slope":
      return (
        <>
          <Ctl label="Uphill"><Shape k="uphill" label="Uphill: −1 gentle, +1 steep" /></Ctl>
          <Ctl label="Downhill"><Shape k="downhill" label="Downhill: −1 gentle, +1 steep" /></Ctl>
          <Ctl label={w().slope === 0 ? "Strength · off" : "Strength"}><Mult k="slope" label="Gradient strength" min={1} /></Ctl>
        </>
      );
    case "tech":
      return (
        <>
          <Ctl label="Harder grades"><Shape k="technical" label="Harder grades: −1 avoid, +1 seek" /></Ctl>
          <Ctl label={w().tech === 0 ? "Strength · off" : "Strength"}><Mult k="tech" label="Technical strength" min={1} /></Ctl>
        </>
      );
    default:
      return <Ctl label={ACTS_ON[props.k]}><Mult k={props.k} label={ACTS_ON[props.k]} /></Ctl>;
  }
}

/** The row's value: the × it is set to, coloured by its effect; strength-only rows read "off" at 0. */
function Value(props: { k: TermKey }) {
  const w = () => weights()[props.k];
  const strength = () => props.k === "slope" || props.k === "tech";
  const effect = () => (strength() ? (w() === 0 ? "off" : undefined) : word(props.k, w()));
  return <span class="pref-x" data-w={effect()}>{effect() === "off" ? "off" : times(multiplierOf(props.k, w()))}</span>;
}

const PENALTY_ROWS: { k: keyof Penalties; label: string; log: boolean; min: number; max: number; step: number; fmt: (v: number) => string; parse?: (s: string) => number | null }[] = [
  { k: "repeat", label: "Running a trail twice", log: true, min: 0.01, max: 1000, step: 0, fmt: times, parse: parseMultiplier },
  { k: "avoid_factor", label: "Inside avoid areas", log: true, min: 1, max: 1000, step: 0, fmt: times, parse: parseMultiplier },
  { k: "prefer_factor", label: "Inside prefer areas", log: true, min: 0.001, max: 1, step: 0, fmt: times, parse: parseMultiplier },
  { k: "stem_m", label: "Free out-and-back at the start", log: false, min: 0, max: 5000, step: 10, fmt: (v) => `${Math.round(v)} m` },
  {
    k: "max_overlap", label: "Overlap between loops", log: false, min: 0, max: 1, step: 0.005, fmt: (v) => `${Math.round(v * 100)} %`,
    parse: (s) => { const n = Number(s.replace(/[%\s]/g, "")); return s.trim() !== "" && Number.isFinite(n) ? n / 100 : null; },
  },
];

export function Preferences(props: {
  /** The route's bars; without them the rows show only their values. */
  bars?: BreakdownItem[];
  /** Ramp step (0–4) per term at the hovered point, to colour the fills (D61). */
  colorOf?: (k: TermKey) => number | undefined;
  heading?: Element;
}) {
  const bar = (k: BreakdownKey) => props.bars?.find((b) => b.key === k);
  const rows = () => ORDER.filter((k) => k !== "view" || hasView() || bar(k));
  return (
    <section class="prefs" aria-label="Preferences">
      <header class="prefs-head">
        <h3>{props.heading ?? "Preferences"}</h3>
        <button class="link-btn" onClick={resetDefaults} title="Every preference back to its default; map areas stay">Defaults</button>
        <button class="link-btn" onClick={clearAll} title="All preferences off, no climb goal, no areas: only the distance counts">Clear all</button>
      </header>
      <Show when={bar("climb")}>{(b) => (
        <div class="pref-row static" title={b().detail}>
          <span class="pref-name">{FACTORS.climb.label}</span>
          <Track value={b().value} />
          <Score value={b().value} />
          <span /><span />
        </div>
      )}</Show>
      <For each={rows()}>{(k) => {
        const open = () => openKey() === k;
        const b = () => bar(k);
        const id = `pref-${k}`;
        return (
          <div class={["pref", { open: open() }]}>
            <button class="pref-row" aria-expanded={`${open()}`} aria-controls={id} onClick={() => setOpenKey(open() ? null : k)} title={b()?.detail}>
              <span class="pref-name">{FACTORS[k].label}</span>
              <Show when={props.bars} fallback={<><span /><span /></>}>
                <Track value={b()?.value ?? null} step={props.colorOf?.(k)} />
                <Score value={b()?.value ?? null} />
              </Show>
              <Value k={k} />
              <ChevronDown size={14} class="pref-chev" aria-hidden="true" />
            </button>
            <Show when={open()}>
              <div class="pref-panel" id={id}>
                {/* This loop's fact; the shape rows say what their −1…+1 means. Below ×1 prefers, above avoids (the colour says which). */}
                <Show when={b()?.detail || k === "slope" || k === "tech"}>
                  <p class="pref-hint">{[b()?.detail, k === "slope" || k === "tech" ? ACTS_ON[k] : ""].filter(Boolean).join(". ")}</p>
                </Show>
                <Panel k={k} />
              </div>
            </Show>
          </div>
        );
      }}</For>
      <details class="pref-more">
        <summary>Advanced<ChevronDown size={14} class="pref-chev" aria-hidden="true" /></summary>
        <For each={PENALTY_ROWS}>{(p) => (
          <Ctl label={p.label}>
            <ScrubField label={p.label} value={penalties()[p.k]} format={p.fmt} parse={p.parse}
              scale={p.log ? "log" : "linear"} step={p.step} min={p.min} max={p.max} onCommit={(v) => setPenalty(p.k, v)} />
          </Ctl>
        )}</For>
      </details>
    </section>
  );
}

function Track(props: { value: number | null; step?: number }) {
  return (
    <span class="track" aria-hidden="true"><span class="fill" style={{
      width: `${Math.round((props.value ?? 0) * 100)}%`,
      background: props.step === undefined ? undefined : `var(--cost-${props.step + 1})`,
    }} /></span>
  );
}

function Score(props: { value: number | null }) {
  return (
    <span class="bar-value">
      <Show when={props.value !== null} fallback={<span class="sr-only">not scored</span>}>
        {Math.round(props.value! * 100)}<span class="sr-only"> out of 100</span>
      </Show>
    </span>
  );
}
