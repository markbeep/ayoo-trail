import { createEffect, createSignal, flush, onSettled, Show, untrack } from "solid-js";
import { Layers, Plus, Minus, X, LocateFixed, LoaderCircle } from "@lucide/solid";
import { MapView, mapController } from "./components/MapView";
import { DevRouteCard } from "./components/DevRouteCard";
import { DEV_ROUTE } from "./dev/routeDebug";
import { Preferences } from "./components/Preferences";
import { QueryCard } from "./components/QueryCard";
import { RouteCard } from "./components/RouteCard";
import { RouteTiles } from "./components/RouteTiles";
import { LayersCard } from "./components/LayersCard";
import { attachSheetDrag, setSnap, snap, snapHeight, SNAPS, type Snap } from "./components/Sheet";
import { narrow, TYPING } from "./layout";
import { openShared, pickingStart, routes, selected, status } from "./state";
import { loopLabel } from "./routes/copies";

function Brand() {
  return (
    <header class="brand">
      <svg viewBox="0 0 32 32" width="20" height="20" aria-hidden="true"><path d="M4 25c4-10 8-15 12-15s6 7 12 4" fill="none" stroke="currentColor" stroke-width="4" /><rect x="1" y="22" width="6" height="6" fill="#fbfbf8" /></svg>
      <span>ayoo</span>
    </header>
  );
}

/** Jump to the device location and mark it; errors show next to the button for a few seconds. */
function LocateButton() {
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const locate = () => {
    if (!navigator.geolocation) { setError("Location isn't available in this browser"); return; }
    setBusy(true);
    setError(null);
    navigator.geolocation.getCurrentPosition(
      (pos) => {
        setBusy(false);
        const p = { lat: pos.coords.latitude, lon: pos.coords.longitude };
        void mapController?.setHere(p);
        mapController?.flyTo(p, 14);
      },
      (e) => {
        setBusy(false);
        setError(e.code === e.PERMISSION_DENIED ? "Location permission was denied" : "Couldn't find your location");
        setTimeout(() => setError(null), 4000);
      },
      { enableHighAccuracy: true, timeout: 10_000, maximumAge: 60_000 },
    );
  };
  return (
    <div class="locate">
      <Show when={error()}><p class="locate-error" role="status">{error()}</p></Show>
      <button class="round-btn" aria-label="Show my location" aria-busy={`${busy()}`} onClick={locate}>
        <Show when={busy()} fallback={<LocateFixed size={20} />}><LoaderCircle size={20} class="spin" /></Show>
      </button>
    </div>
  );
}

export function App() {
  const [layersOpen, setLayersOpen] = createSignal(!untrack(narrow));
  let panel!: HTMLElement;
  let handle!: HTMLDivElement;
  let scroller!: HTMLDivElement;

  // Keep fitted routes clear of the panel (left on desktop, the sheet on phone).
  const padding = () => {
    if (narrow()) return { left: 28, right: 28, top: 80, bottom: snapHeight("half") + 24 };
    const r = panel.getBoundingClientRect();
    return { left: r.right + 40, right: 300, top: 60, bottom: 60 };
  };

  onSettled(() => {
    // Deferred: openShared flush()es, which 2.0 forbids inside onSettled. hashchange covers a link pasted into an open tab.
    const open = () => queueMicrotask(() => void openShared());
    open();
    window.addEventListener("hashchange", open);
    attachSheetDrag(handle, panel);
    // Typing on a phone (D105): open the sheet fully so the field sits near the top of the screen,
    // above the keyboard, and put it back where it was once typing ends. Android shrinks the page
    // for the keyboard (index.html interactive-widget); iOS doesn't, but a field near the top stays
    // visible without Safari scrolling the page.
    let before: Snap | null = null;
    const onFocus = (e: FocusEvent) => {
      if (!untrack(narrow) || !(e.target as Element).matches(TYPING)) return;
      before ??= untrack(snap);
      // Instantly, not animated: Safari picks its scroll as the keyboard opens, and a field still
      // low mid-animation makes it scroll the page, throwing the field off the top.
      panel.classList.add("dragging", "typing");
      setSnap("full");
      flush();
      void panel.offsetHeight;
      panel.classList.remove("dragging");
      // A field deep in the settings would still sit under the keyboard: scroll it to the top of
      // the sheet (.typing adds room below so even the last row can get there).
      const top = (e.target as Element).getBoundingClientRect().top - scroller.getBoundingClientRect().top;
      if (top > 56) scroller.scrollTop += top - 40;
    };
    const onBlur = () => setTimeout(() => {
      if (before === null || document.activeElement?.matches(TYPING)) return;
      panel.classList.remove("typing");
      setSnap(before);
      before = null;
    }, 0);
    panel.addEventListener("focusin", onFocus);
    panel.addEventListener("focusout", onBlur);
    return () => {
      window.removeEventListener("hashchange", open);
      panel.removeEventListener("focusin", onFocus);
      panel.removeEventListener("focusout", onBlur);
    };
  });
  // Sheet choreography: results open it halfway, picking a start lowers it.
  createEffect(status, (s, prev) => {
    if (prev === "loading" && s === "idle" && untrack(routes).length) {
      setSnap("half");
      scroller.scrollTo({ top: 0 });
    }
  }, { defer: true });
  createEffect(pickingStart, (p) => { if (p) setSnap("peek"); }, { defer: true });
  createEffect(narrow, (n) => { setLayersOpen(!n); }, { defer: true });

  return (
    <div class={["app", { narrow: narrow() }]}>
      <MapView padding={padding} />

      <Show when={narrow()}><div class="brand-float"><Brand /></div></Show>

      <aside class="panel" ref={panel} aria-label="Plan a loop" data-snap={snap()}>
        <div class="sheet-handle" ref={handle} role="button" tabindex="0"
          aria-label={snap() === "closed" || snap() === "peek" ? "Expand panel" : "Collapse panel"}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setSnap(SNAPS[(SNAPS.indexOf(snap()) + 1) % SNAPS.length]!); }
            else if (e.key === "ArrowUp") { e.preventDefault(); setSnap(SNAPS[Math.min(SNAPS.indexOf(snap()) + 1, SNAPS.length - 1)]!); }
            else if (e.key === "ArrowDown") { e.preventDefault(); setSnap(SNAPS[Math.max(SNAPS.indexOf(snap()) - 1, 0)]!); }
          }}>
          <span />
        </div>
        <div class="panel-scroll" ref={scroller}>
          <Show when={!narrow()}><Brand /></Show>
          <Show when={DEV_ROUTE}><DevRouteCard /></Show>
          <QueryCard />
          <Show when={routes().length} fallback={
            <p class="empty">Pick a start and a distance, then find loops.</p>
          }><RouteTiles routes={routes()} /></Show>
          <Show when={selected()} fallback={<section class="card"><Preferences /></section>}>{(r) => <RouteCard route={r()} label={loopLabel(routes(), r())} />}</Show>
        </div>
      </aside>

      <div class="map-tools">
        <Show when={layersOpen()} fallback={
          <button class="round-btn" aria-label="Map layers" aria-expanded="false" onClick={() => setLayersOpen(true)}><Layers size={20} /></button>
        }>
          <div class="layers-wrap">
            <Show when={narrow()}>
              <button class="round-btn close" aria-label="Close map layers" onClick={() => setLayersOpen(false)}><X size={18} /></button>
            </Show>
            <LayersCard onPicked={() => narrow() && setLayersOpen(false)} />
          </div>
        </Show>
        <LocateButton />
        <Show when={!narrow()}>
          <div class="zoom">
            <button class="round-btn" aria-label="Zoom in" onClick={() => mapController?.zoomIn()}><Plus size={20} /></button>
            <button class="round-btn" aria-label="Zoom out" onClick={() => mapController?.zoomOut()}><Minus size={20} /></button>
          </div>
        </Show>
      </div>
    </div>
  );
}
