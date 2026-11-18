import { createEffect, onSettled, untrack } from "solid-js";
import { DEV_ROUTE, addWaypoint, devHealth, devWaypoints } from "../dev/routeDebug";
import { MapController } from "../map/controller";
import {
  baseMap, pickingStart, routes, selectedId, setPickingStart, setSelectedId, setStart, start,
} from "../state";

/** Reads a CSS custom property so map paint colours follow the design tokens. */
function token(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

export let mapController: MapController | undefined;

export function MapView(props: { padding: () => { left: number; right: number; top: number; bottom: number } }) {
  let el!: HTMLDivElement;
  let ctl: MapController | undefined;

  // 2.0 forbids creating effects inside onSettled, so they live here, deferred, and
  // onSettled applies their initial values once the controller exists.
  onSettled(() => {
    const c = new MapController(el, untrack(baseMap), {
      selected: token("--route-active"),
      candidate: token("--route-candidate"),
      casing: token("--route-casing"),
      start: token("--ink"),
    }, {
      onRouteClick: (id) => setSelectedId(id),
      onMapClick: (p) => {
        if (pickingStart()) {
          setStart(p);
          setPickingStart(false);
        } else if (DEV_ROUTE) addWaypoint(p);
      },
    });
    ctl = mapController = c;
    c.flyTo(untrack(start), 12);
    if (DEV_ROUTE) {
      void devHealth().then((b) => b && c.fitBounds(b, props.padding()));
      void c.setWaypoints(untrack(devWaypoints));
    }
    void c.setStart(untrack(start));
    el.classList.toggle("picking", untrack(pickingStart));
    return () => c.destroy();
  });

  if (DEV_ROUTE) createEffect(devWaypoints, (w) => void ctl?.setWaypoints(w), { defer: true });
  createEffect(baseMap, (id) => ctl?.setBaseMap(id), { defer: true });
  createEffect(start, (p) => void ctl?.setStart(p), { defer: true });
  createEffect(routes, (rs) => {
    const id = untrack(selectedId);
    void ctl?.setRoutes(rs, id);
    if (id && !DEV_ROUTE) ctl?.fitRoute(id, props.padding());
  }, { defer: true });
  createEffect(selectedId, (id) => {
    void ctl?.select(id);
    if (id && !DEV_ROUTE) ctl?.fitRoute(id, props.padding());
  }, { defer: true });
  createEffect(pickingStart, (p) => { el.classList.toggle("picking", p); }, { defer: true });

  return <div ref={el} class="map" role="region" aria-label="Map" />;
}
