// A number field you drag left/right to change, like Photoshop's (D106). A click without a drag
// types a value; arrow keys step. It commits once per gesture, so a live refresh (D55) runs once.
import { createSignal, Show } from "solid-js";
import { parseNumber, scrub, type ScrubOpts } from "../routes/multipliers";

/** Arrow keys move like a 10 px drag. */
const KEY_PX = 10;

export function ScrubField(props: {
  value: number; label: string; format: (v: number) => string; scale: ScrubOpts["scale"]; step: number;
  min: number; max: number; parse?: (s: string) => number | null; onCommit: (v: number) => void; disabled?: boolean;
}) {
  const [live, setLive] = createSignal<number | null>(null);
  const [editing, setEditing] = createSignal(false);
  const opts = (fast: boolean): ScrubOpts => ({ scale: props.scale, step: props.step, min: props.min, max: props.max, fast });
  const v = () => live() ?? props.value;
  // Keep links and requests short: 3 significant digits on a log scale, whole steps linear.
  const tidy = (x: number) => props.scale === "log" ? Number(x.toPrecision(3)) : Number((Math.round(x / props.step) * props.step).toFixed(6));
  const commit = (x: number) => { setLive(null); const t = tidy(Math.min(props.max, Math.max(props.min, x))); if (t !== props.value) props.onCommit(t); };
  let down: { x: number; v: number; moved: boolean } | null = null;

  return (
    <Show when={editing()} fallback={
      <span
        class={["scrub", { disabled: props.disabled }]} role="spinbutton" tabindex={props.disabled ? -1 : 0}
        aria-label={props.label} aria-valuenow={v()} aria-valuemin={props.min} aria-valuemax={props.max}
        aria-valuetext={props.format(v())} aria-disabled={props.disabled ? "true" : undefined}
        onPointerDown={(e) => {
          if (props.disabled || e.button !== 0) return;
          down = { x: e.clientX, v: props.value, moved: false };
          e.currentTarget.setPointerCapture(e.pointerId);
        }}
        onPointerMove={(e) => {
          if (!down) return;
          const dx = e.clientX - down.x;
          if (!down.moved && Math.abs(dx) <= 3) return;
          down.moved = true;
          setLive(scrub(down.v, dx, opts(e.shiftKey)));
        }}
        onPointerUp={() => {
          if (!down) return;
          const d = down;
          down = null;
          if (d.moved) commit(live() ?? d.v);
          else setEditing(true);
        }}
        onPointerCancel={() => { down = null; setLive(null); }}
        onKeyDown={(e) => {
          if (props.disabled) return;
          const dir = e.key === "ArrowRight" || e.key === "ArrowUp" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowDown" ? -1 : 0;
          if (dir) { e.preventDefault(); commit(scrub(props.value, dir * KEY_PX, opts(e.shiftKey))); }
          else if (e.key === "Enter") { e.preventDefault(); setEditing(true); }
        }}
      >{props.format(v())}</span>
    }>
      <input
        class="scrub-input" aria-label={props.label} value={props.format(props.value)}
        ref={(el) => queueMicrotask(() => { el.focus(); el.select(); })}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
          else if (e.key === "Escape") { e.currentTarget.dataset.cancel = "1"; e.currentTarget.blur(); }
        }}
        onBlur={(e) => {
          const el = e.currentTarget;
          if (!el.dataset.cancel) {
            const x = (props.parse ?? parseNumber)(el.value);
            if (x !== null) commit(x); // invalid input reverts
          }
          setEditing(false);
        }}
      />
    </Show>
  );
}
