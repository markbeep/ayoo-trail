// Imperative MapLibre wrapper. Solid components call into this; it owns the
// map instance, base-map switching, overlays and route drawing.

import * as maplibregl from "maplibre-gl";
import type { StyleSpecification, GeoJSONSource, LngLatLike, FilterSpecification } from "maplibre-gl";
// MapLibre 6 resolves its worker relative to its own module, which breaks once
// Vite bundles it; point it at the worker file explicitly.
import workerUrl from "maplibre-gl/dist/maplibre-gl-worker.mjs?url";
import { BASE_MAPS, OVERLAYS, type BaseMapId } from "./basemaps";
import type { LatLon, Route } from "../routes/types";
import { bounds } from "../routes/geo";

export interface RouteColors {
  selected: string;
  candidate: string;
  casing: string;
  start: string;
}

maplibregl.setWorkerUrl(workerUrl);

const SWITZERLAND: [[number, number], [number, number]] = [[5.9, 45.8], [10.5, 47.85]];
const ROUTES = "routes";
const START = "start";

function buildStyle(base: BaseMapId): StyleSpecification {
  const sources: StyleSpecification["sources"] = {};
  const layers: StyleSpecification["layers"] = [];
  for (const def of [...BASE_MAPS, ...OVERLAYS]) {
    sources[def.id] = {
      type: "raster",
      tiles: def.tiles,
      tileSize: def.tileSize,
      maxzoom: def.maxzoom,
      attribution: def.attribution,
    };
  }
  for (const def of BASE_MAPS) {
    layers.push({
      id: `base-${def.id}`,
      type: "raster",
      source: def.id,
      layout: { visibility: def.id === base ? "visible" : "none" },
    });
  }
  for (const def of OVERLAYS) {
    layers.push({
      id: `overlay-${def.id}`,
      type: "raster",
      source: def.id,
      paint: { "raster-opacity": def.opacity },
      layout: { visibility: "none" },
    });
  }
  return { version: 8, sources, layers };
}

export interface MapHandlers {
  onRouteClick?: (id: string) => void;
  onMapClick?: (p: LatLon) => void;
}

export class MapController {
  readonly map: maplibregl.Map;
  private ready: Promise<void>;
  private routes: Route[] = [];
  private selected: string | null = null;

  constructor(container: HTMLElement, base: BaseMapId, private colors: RouteColors, handlers: MapHandlers = {}) {
    this.map = new maplibregl.Map({
      container,
      style: buildStyle(base),
      bounds: SWITZERLAND,
      maxBounds: [[4.5, 45.3], [11.9, 48.3]],
      attributionControl: false,
      dragRotate: false,
      pitchWithRotate: false,
      touchPitch: false,
    });
    this.map.touchZoomRotate.disableRotation();
    this.map.addControl(new maplibregl.AttributionControl({
      compact: false,
      customAttribution: '<a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">© OpenStreetMap contributors</a>',
    }), "bottom-right");
    this.map.addControl(new maplibregl.ScaleControl({ unit: "metric" }), "bottom-right");

    this.ready = new Promise((resolve) => this.map.on("load", () => {
      this.addRouteLayers();
      resolve();
    }));

    this.map.on("click", (e) => {
      const hit = this.map.queryRenderedFeatures(e.point, { layers: ["route-hit"] })[0];
      const id = hit?.properties?.id;
      if (typeof id === "string") handlers.onRouteClick?.(id);
      else handlers.onMapClick?.({ lat: e.lngLat.lat, lon: e.lngLat.lng });
    });
    this.map.on("mouseenter", "route-hit", () => (this.map.getCanvas().style.cursor = "pointer"));
    this.map.on("mouseleave", "route-hit", () => (this.map.getCanvas().style.cursor = ""));
  }

  private addRouteLayers() {
    const c = this.colors;
    this.map.addSource(ROUTES, { type: "geojson", data: { type: "FeatureCollection", features: [] } });
    this.map.addSource(START, { type: "geojson", data: { type: "FeatureCollection", features: [] } });
    const isSel: FilterSpecification = ["==", ["get", "selected"], true];
    const notSel: FilterSpecification = ["!=", ["get", "selected"], true];
    const join = { "line-join": "round", "line-cap": "round" } as const;
    // Line form carries state: candidates thin and muted, selected heavy and purple.
    // (No dashes: swisstopo draws railways dashed.)
    this.map.addLayer({
      id: "route-candidate-casing", type: "line", source: ROUTES, layout: join,
      filter: notSel,
      paint: { "line-color": c.casing, "line-width": ["interpolate", ["linear"], ["zoom"], 10, 5, 15, 8], "line-opacity": 0.85 },
    });
    this.map.addLayer({
      id: "route-candidate", type: "line", source: ROUTES, layout: join,
      filter: notSel,
      paint: {
        "line-color": c.candidate,
        "line-width": ["interpolate", ["linear"], ["zoom"], 10, 2.5, 15, 4],
        "line-opacity": 0.85,
      },
    });
    this.map.addLayer({
      id: "route-casing", type: "line", source: ROUTES, layout: join,
      filter: isSel,
      paint: { "line-color": c.casing, "line-width": ["interpolate", ["linear"], ["zoom"], 10, 7, 15, 11] },
    });
    this.map.addLayer({
      id: "route-selected", type: "line", source: ROUTES, layout: join,
      filter: isSel,
      paint: { "line-color": c.selected, "line-width": ["interpolate", ["linear"], ["zoom"], 10, 4, 15, 6.5] },
    });
    // Wide invisible line so thin candidates are easy to tap.
    this.map.addLayer({
      id: "route-hit", type: "line", source: ROUTES, layout: join,
      paint: { "line-color": "#000", "line-opacity": 0, "line-width": 22 },
    });
    this.map.addLayer({
      id: "start-point", type: "circle", source: START,
      paint: { "circle-radius": 7, "circle-color": c.start, "circle-stroke-color": c.casing, "circle-stroke-width": 3 },
    });
  }

  setBaseMap(id: BaseMapId) {
    const apply = () => {
      for (const def of BASE_MAPS) {
        this.map.setLayoutProperty(`base-${def.id}`, "visibility", def.id === id ? "visible" : "none");
      }
    };
    if (this.map.isStyleLoaded()) apply();
    else void this.ready.then(apply);
  }

  async setRoutes(routes: Route[], selected: string | null) {
    this.routes = routes;
    this.selected = selected;
    await this.ready;
    this.redraw();
  }

  async select(id: string | null) {
    this.selected = id;
    await this.ready;
    this.redraw();
  }

  async setStart(p: LatLon | null) {
    await this.ready;
    (this.map.getSource(START) as GeoJSONSource).setData({
      type: "FeatureCollection",
      features: p ? [{ type: "Feature", properties: {}, geometry: { type: "Point", coordinates: [p.lon, p.lat] } }] : [],
    });
  }

  fitRoute(id: string, padding: maplibregl.PaddingOptions) {
    const r = this.routes.find((x) => x.id === id);
    if (!r) return;
    this.map.fitBounds(bounds(r.geometry.coordinates), { padding, duration: 600, maxZoom: 15 });
  }

  flyTo(p: LatLon, zoom = 13) {
    this.map.flyTo({ center: [p.lon, p.lat] as LngLatLike, zoom, duration: 800 });
  }

  zoomIn() { this.map.zoomIn(); }
  zoomOut() { this.map.zoomOut(); }

  private redraw() {
    // Selected route last so it sits on top within the shared source.
    const ordered = [...this.routes].sort((a, b) => Number(a.id === this.selected) - Number(b.id === this.selected));
    (this.map.getSource(ROUTES) as GeoJSONSource).setData({
      type: "FeatureCollection",
      features: ordered.map((r) => ({
        type: "Feature",
        properties: { id: r.id, selected: r.id === this.selected },
        geometry: { type: "LineString", coordinates: r.geometry.coordinates.map(([x, y]) => [x, y]) },
      })),
    });
  }

  destroy() { this.map.remove(); }
}
