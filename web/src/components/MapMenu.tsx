// Map context menu (D46): right-click, or long-press on phones.
import { createEffect, createSignal, onSettled, Show } from "solid-js";
import { Copy, MapPin } from "@lucide/solid";
import type { LatLon } from "../routes/types";
import { setStart } from "../state";

export interface MenuState { p: LatLon; x: number; y: number }

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
    transform: `translate(${props.menu.x > innerWidth - 240 ? "-100%" : "0"}, ${props.menu.y > innerHeight - 140 ? "-100%" : "0"})`,
  });

  return (
    <div class="map-menu" role="menu" aria-label="Map actions" ref={el} style={style()}>
      <button role="menuitem" onClick={() => { setStart(props.menu.p); props.onClose(); }}>
        <MapPin size={16} />Start here
      </button>
      <button role="menuitem" onClick={() => {
        void navigator.clipboard?.writeText(coords()).then(() => { setCopied(true); closing = setTimeout(props.onClose, 700); }, props.onClose);
      }}>
        <Copy size={16} /><Show when={copied()} fallback={<>Copy coordinates<small>{coords()}</small></>}>Copied</Show>
      </button>
    </div>
  );
}
