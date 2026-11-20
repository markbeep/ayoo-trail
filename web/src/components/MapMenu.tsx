// Map context menu (D46): right-click, or long-press on phones.
import { createEffect, createSignal, onSettled, Show } from "solid-js";
import { CircleDot, CircleSlash, Copy, MapPin, Repeat2, Trash2 } from "@lucide/solid";
import type { LatLon } from "../routes/types";
import { addZone, removeZone, setStart, toggleHard, zones } from "../state";
import { isMust, MAX_MUST, MAX_ZONES } from "../routes/zones";

/** `zone`: the avoid or prefer area that was long-pressed or right-clicked, if any (D90, D97). */
export interface MenuState { p: LatLon; x: number; y: number; zone?: number | undefined }

export function MapMenu(props: { menu: MenuState; onClose: () => void }) {
  const [copied, setCopied] = createSignal(false);
  let el!: HTMLDivElement;
  let closing: ReturnType<typeof setTimeout> | undefined;
  // Reopened at a new spot (same component instance): drop the old menu's pending close.
  createEffect(() => props.menu, () => {
    clearTimeout(closing);
    setCopied(false);
    el.querySelector("button")?.focus();
  }, { defer: true });
  const zone = () => (props.menu.zone === undefined ? undefined : zones()[props.menu.zone]);
  const full = () => zones().length >= MAX_ZONES;
  // Making a prefer area hard adds a must-pass; the server takes at most MAX_MUST.
  const mustFull = () => zone()?.kind === "prefer" && !zone()?.hard && zones().filter(isMust).length >= MAX_MUST;
  const coords = () => `${props.menu.p.lat.toFixed(5)}, ${props.menu.p.lon.toFixed(5)}`;

  onSettled(() => {
    el.querySelector("button")?.focus();
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
    transform: `translate(${props.menu.x > innerWidth - 240 ? "-100%" : "0"}, ${props.menu.y > innerHeight - 190 ? "-100%" : "0"})`,
  });

  return (
    <div class="map-menu" role="menu" aria-label="Map actions" ref={el} style={style()}>
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
    </div>
  );
}
