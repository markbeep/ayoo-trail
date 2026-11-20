import { createEffect, createSignal, onSettled, Show, untrack } from "solid-js";
import { MapMenu, type MenuState } from "./MapMenu";
import { DEV_ROUTE, addWaypoint, devHealth, devWaypoints } from "../dev/routeDebug";
import { MapController } from "../map/controller";
import { nearestIndex } from "../routes/geo";
import type { LatLon } from "../routes/types";
import {
  hoverIndex, selected, setHoverIndex, baseMap, overlays, pickingStart, routes, selectedId, setPickingStart, setSelectedId, setStart, start,
  costView, editWaypoints, MAX_WAYPOINTS, refreshing, zones, moveZone, resizeZone,
} from "../state";

/** Reads a CSS custom property so map paint colors follow the design tokens. */
function token(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

export let mapController: MapController | undefined;

export function MapView(props: { padding: () => { left: number; right: number; top: number; bottom: number } }) {
  let el!: HTMLDivElement;
  let ctl: MapController | undefined;
  const [menu, setMenu] = createSignal<MenuState | null>(null);

  // 2.0 forbids creating effects inside onSettled, so they live here, deferred, and
  // onSettled applies their initial values once the controller exists.
  onSettled(() => {
    const c = new MapController(el, untrack(baseMap), untrack(overlays), {
      selected: token("--route-active"),
      candidate: token("--route-candidate"),
      casing: token("--route-casing"),
      start: token("--ink"),
      here: token("--here"),
      shade: token("--map-shade"),
      forest: token("--map-forest"),
      cost: [1, 2, 3, 4, 5].map((i) => token(`--cost-${i}`)),
      palette: {
        land: token("--map-land"), wood: token("--map-wood"), grass: token("--map-grass"),
        rock: token("--map-rock"), ice: token("--map-ice"), water: token("--map-water"),
        road: token("--map-road"), roadCasing: token("--map-road-casing"), path: token("--map-path"),
        label: token("--map-label"), halo: token("--map-halo"),
      },
    }, {
      onRouteClick: (id) => setSelectedId(id),
      onMapClick: (p) => {
        if (pickingStart()) {
          setStart(p);
          setPickingStart(false);
        } else if (DEV_ROUTE) addWaypoint(p);
      },
      onContextMenu: (p, at, zone) => setMenu({ p, x: at.x, y: at.y, zone }),
      onZoneMove: moveZone,
      onZoneResize: resizeZone,
      onMoveStart: () => setMenu(null),
      onWaypointMove: (i, p) => {
        const r = untrack(selected);
        if (r) void editWaypoints(r.id, r.waypoints.map((w, j) => (j === i ? p : w)));
      },
      onWaypointInsert: (leg, p) => {
        const r = untrack(selected);
        if (r && r.waypoints.length < MAX_WAYPOINTS) void editWaypoints(r.id, [...r.waypoints.slice(0, leg + 1), p, ...r.waypoints.slice(leg + 1)]);
      },
      onWaypointRemove: (i) => {
        const r = untrack(selected);
        // Keep the start, the finish and at least one point to turn at.
        if (r && r.waypoints.length > 3) void editWaypoints(r.id, r.waypoints.filter((_, j) => j !== i));
      },
      onRouteHover: (p) => {
        const r = untrack(selected);
        setHoverIndex(p && r ? nearestIndex(r.geometry.coordinates, p.lon, p.lat) : null);
      },
    });
    ctl = mapController = c;
    c.flyTo(untrack(start), 12);
    if (DEV_ROUTE) {
      void devHealth().then((b) => b && c.fitBounds(b, props.padding()));
      void c.setWaypoints(untrack(devWaypoints));
    }
    void c.setStart(untrack(start));
    void c.setZones(untrack(zones));
    el.classList.toggle("picking", untrack(pickingStart));
    return () => c.destroy();
  });

  if (DEV_ROUTE) createEffect(devWaypoints, (w) => void ctl?.setWaypoints(w), { defer: true });
  createEffect(baseMap, (id) => ctl?.setBaseMap(id), { defer: true });
  createEffect(overlays, (on, prev) => {
    for (const id of new Set([...on, ...(prev ?? [])])) ctl?.setOverlay(id, on.includes(id));
  }, { defer: true });
  createEffect(start, (p) => void ctl?.setStart(p), { defer: true });
  createEffect(zones, (z) => void ctl?.setZones(z), { defer: true });
  createEffect(routes, (rs) => {
    const id = untrack(selectedId);
    void ctl?.setRoutes(rs, id);
    // Edits and live refreshes keep the view where the user is working.
    if (id && !DEV_ROUTE && !untrack(refreshing)) ctl?.fitRoute(id, untrack(props.padding));
  }, { defer: true });
  createEffect(selectedId, (id) => {
    void ctl?.select(id);
    if (id && !DEV_ROUTE) ctl?.fitRoute(id, untrack(props.padding));
  }, { defer: true });
  createEffect(() => {
    const i = hoverIndex(), c = selected()?.geometry.coordinates[i ?? -1];
    return c ? { lon: c[0], lat: c[1] } : null;
  }, (p) => void ctl?.setHover(p), { defer: true });
  createEffect(selectedId, () => { setHoverIndex(null); }, { defer: true });
  if (!DEV_ROUTE) createEffect(() => selected()?.waypoints ?? ([] as LatLon[]), (w) => void ctl?.setHandles(w), { defer: true });
  createEffect(costView, (on) => void ctl?.setCostView(on), { defer: true });
  createEffect(pickingStart, (p) => { el.classList.toggle("picking", p); }, { defer: true });

  return (
    <>
      <div ref={el} class="map" role="region" aria-label="Map" />
      <Show when={menu()}>{(m) => <MapMenu menu={m()} onClose={() => setMenu(null)} />}</Show>
    </>
  );
}
