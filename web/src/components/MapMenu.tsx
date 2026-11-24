// Map context menu (D46): right-click, or long-press on phones.
import { createEffect, createSignal, onSettled, Show } from "solid-js";
import { CircleDot, CircleSlash, Copy, Flag, MapPin, Repeat2, TrainFront, Trash2 } from "@lucide/solid";
import type { LatLon } from "../routes/types";
import {
  addCheckpoint, addOrigin, addZone, checkpoints, origins, removeCheckpoint, removeOrigin, removeZone, renameCheckpoint, selected, setStart,
  toggleHard, zones,
} from "../state";
import { MAX_CHECKPOINTS } from "../url";
import { isMust, MAX_MUST, MAX_ZONES } from "../routes/zones";
import { MAX_ORIGINS } from "../routes/travel";

/** `zone` / `checkpoint` / `origin`: what was long-pressed or right-clicked, if anything (D90, D97, D98, ADR 0002). */
export interface MenuState { p: LatLon; x: number; y: number; zone?: number | undefined; checkpoint?: number | undefined; origin?: number | undefined }

export function MapMenu(props: { menu: MenuState; onClose: () => void }) {
  const [copied, setCopied] = createSignal(false);
  let el!: HTMLDivElement;
  let closing: ReturnType<typeof setTimeout> | undefined;
  // Reopened at a new spot (same component instance): drop the old menu's pending close.
  createEffect(() => props.menu, () => {
    clearTimeout(closing);
    setCopied(false);
    el.querySelector<HTMLElement>("input, button")?.focus();
  }, { defer: true });
  const zone = () => (props.menu.zone === undefined ? undefined : zones()[props.menu.zone]);
  const full = () => zones().length >= MAX_ZONES;
  // Making a prefer area hard adds a must-pass; the server takes at most MAX_MUST.
  const mustFull = () => zone()?.kind === "prefer" && !zone()?.hard && zones().filter(isMust).length >= MAX_MUST;
  const coords = () => `${props.menu.p.lat.toFixed(5)}, ${props.menu.p.lon.toFixed(5)}`;

  onSettled(() => {
    el.querySelector<HTMLElement>("input, button")?.focus();
    const outside = (e: PointerEvent) => { if (!el.contains(e.target as Node)) props.onClose(); };
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") props.onClose(); };
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("keydown", key);
    return () => {
      // A pending "Copied" close must not shut a menu reopened elsewhere.
      clearTimeout(closing);
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("keydown", key);
    };
  });

  // Keep the menu inside the map: open leftward/upward near the right/bottom edge.
  const style = () => ({
    left: `${props.menu.x}px`, top: `${props.menu.y}px`,
    transform: `translate(${props.menu.x > innerWidth - 240 ? "-100%" : "0"}, ${props.menu.y > innerHeight - 290 ? "-100%" : "0"})`,
  });

  const cp = () => (props.menu.checkpoint === undefined ? undefined : checkpoints()[props.menu.checkpoint]);
  /** The right-clicked travel origin, wrapped so index 0 is truthy for Show. */
  const origin = () => { const i = props.menu?.origin; return i !== undefined && origins()[i] ? { i } : undefined; };

  return (
    <div class="map-menu" role="menu" aria-label="Map actions" ref={el} style={style()}>
      <Show when={cp()}>
        {(c) => <>
          {/* Saved on every keystroke; Enter closes. */}
          <input aria-label="Checkpoint name" value={c().name} maxlength={40}
            onInput={(e) => renameCheckpoint(props.menu.checkpoint!, e.currentTarget.value)}
            onKeyDown={(e) => { if (e.key === "Enter") props.onClose(); }} />
          <button role="menuitem" onClick={() => { removeCheckpoint(props.menu.checkpoint!); props.onClose(); }}>
            <Trash2 size={16} />Remove checkpoint
          </button>
        </>}
      </Show>
      {/* `?.`: removing closes the menu, and these re-read after the menu is gone. */}
      <Show when={origin()}>
        {(o) => <button role="menuitem" onClick={() => { removeOrigin(o().i); props.onClose(); }}>
          <Trash2 size={16} />Remove travel origin {o().i + 1}
        </button>}
      </Show>
      <Show when={!cp() && props.menu?.origin === undefined}>
      <button role="menuitem" onClick={() => { setStart(props.menu.p); props.onClose(); }}>
        <MapPin size={16} />Start here
      </button>
      <Show
        when={zone()}
        fallback={<>
          <button role="menuitem" disabled={full()} onClick={() => { addZone(props.menu.p, "avoid"); props.onClose(); }}>
            <CircleSlash size={16} />Avoid area here
          </button>
          <button role="menuitem" disabled={full()} onClick={() => { addZone(props.menu.p, "prefer"); props.onClose(); }}>
            <CircleDot size={16} />Prefer area here
          </button>
          <button role="menuitem" disabled={origins().length >= MAX_ORIGINS} onClick={() => { addOrigin(props.menu.p); props.onClose(); }}>
            <TrainFront size={16} />Travel time from here
          </button>
          <Show when={selected()}>
            <button role="menuitem" disabled={checkpoints().length >= MAX_CHECKPOINTS}
              onClick={() => { addCheckpoint(props.menu.p); props.onClose(); }}>
              <Flag size={16} />Checkpoint here<small>On the selected loop</small>
            </button>
          </Show>
        </>}
      >
        {(z) => <>
          <button role="menuitem" disabled={mustFull()} onClick={() => { toggleHard(props.menu.zone!); props.onClose(); }}>
            <Repeat2 size={16} />
            <Show when={z().hard} fallback={<>Make hard<small>{z().kind === "prefer" ? "Must pass" : "Never cross"}</small></>}>
              Make soft<small>{z().kind === "prefer" ? "Prefer, may skip" : "Avoid, may cross"}</small>
            </Show>
          </button>
          <button role="menuitem" onClick={() => { removeZone(props.menu.zone!); props.onClose(); }}>
            <Trash2 size={16} />Remove {z().kind === "prefer" ? "prefer" : "avoid"} area
          </button>
        </>}
      </Show>
      <button role="menuitem" onClick={() => {
        void navigator.clipboard?.writeText(coords()).then(() => { setCopied(true); closing = setTimeout(props.onClose, 700); }, props.onClose);
      }}>
        <Copy size={16} /><Show when={copied()} fallback={<>Copy coordinates<small>{coords()}</small></>}>Copied</Show>
      </button>
      </Show>
    </div>
  );
}
