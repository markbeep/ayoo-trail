import { createEffect, on, onCleanup, onMount } from "solid-js";
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

  onMount(() => {
    const ctl = new MapController(el, baseMap(), {
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
        }
      },
    });
    mapController = ctl;
    ctl.flyTo(start(), 12);
    onCleanup(() => ctl.destroy());

    createEffect(on(baseMap, (id) => ctl.setBaseMap(id), { defer: true }));
    createEffect(() => void ctl.setStart(start()));
    createEffect(on(routes, (rs) => {
      void ctl.setRoutes(rs, selectedId());
      const id = selectedId();
      if (id) ctl.fitRoute(id, props.padding());
    }, { defer: true }));
    createEffect(on(selectedId, (id) => {
      void ctl.select(id);
      if (id) ctl.fitRoute(id, props.padding());
    }, { defer: true }));
    createEffect(() => el.classList.toggle("picking", pickingStart()));
  });

  return <div ref={el} class="map" role="region" aria-label="Map" />;
}
