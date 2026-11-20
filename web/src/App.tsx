import { createEffect, createSignal, onSettled, Show, untrack } from "solid-js";
import { Layers, Plus, Minus, X, LocateFixed, LoaderCircle } from "@lucide/solid";
import { MapView, mapController } from "./components/MapView";
import { DevRouteCard } from "./components/DevRouteCard";
import { DEV_ROUTE } from "./dev/routeDebug";
import { QueryCard } from "./components/QueryCard";
import { RouteCard } from "./components/RouteCard";
import { RouteTiles } from "./components/RouteTiles";
import { LayersCard } from "./components/LayersCard";
import { attachSheetDrag, setSnap, snap, snapHeight, SNAPS } from "./components/Sheet";
import { narrow } from "./layout";
import { openShared, pickingStart, routes, selected, selectedId, status } from "./state";
import { loopName } from "./routes/format";

function Brand() {
  return (
    <header class="brand">
      <svg viewBox="0 0 32 32" width="30" height="30" aria-hidden="true"><rect width="32" height="32" rx="9" fill="currentColor" /><path d="M7 22c3-7 6-11 9-11s4 5 9 3" fill="none" stroke="#a7adf0" stroke-width="3" stroke-linecap="round" /><circle cx="7" cy="22" r="3" fill="#fff" /></svg>
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
    void openShared();
    return attachSheetDrag(handle, panel);
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

  const selectedIndex = () => routes().findIndex((r) => r.id === selectedId());

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
          <Show when={selected()}>{(r) => (
            <>
              <Show when={narrow()}><RouteTiles routes={routes()} /></Show>
              <RouteCard route={r()} name={loopName(selectedIndex())} />
              <Show when={!narrow()}><RouteTiles routes={routes()} /></Show>
            </>
          )}</Show>
          <Show when={!routes().length}>
            <p class="empty">Pick a start, a distance and how hilly you want it. You'll get a few loops to compare.</p>
          </Show>
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
