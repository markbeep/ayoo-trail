// Imperative MapLibre wrapper. Solid components call into this; it owns the
// map instance, base-map switching, overlays and route drawing.

import * as maplibregl from "maplibre-gl";
import type { StyleSpecification, GeoJSONSource, LngLatLike, FilterSpecification, LayerSpecification } from "maplibre-gl";
// MapLibre 6 resolves its worker relative to its own module, which breaks once
// Vite bundles it; point it at the worker file explicitly.
import workerUrl from "maplibre-gl/dist/maplibre-gl-worker.mjs?url";
import {
  BASE_MAPS, GLYPHS, OPENMAPTILES, OPENMAPTILES_ATTRIBUTION, OVERLAYS, TERRAIN, TRAILS_SOURCE, type BaseMapId,
} from "./basemaps";
import { OMT, outdoorFills, outdoorLabels, outdoorLines, type MapPalette } from "./outdoorStyle";
import type { LatLon, Route } from "../routes/types";
import { bounds, cumulative, legAt, nearestIndex } from "../routes/geo";

export interface RouteColors {
  selected: string;
  candidate: string;
  casing: string;
  start: string;
}

export interface MapColors extends RouteColors {
  /** The user's-location dot. */
  here: string;
  palette: MapPalette;
  /** Hillshade shadow and the forest overlay tint. */
  shade: string;
  forest: string;
  /** Cost-per-meter ramp, cheapest first, for COST_STOPS (D54). */
  cost: string[];
}

/** Upper cost/m bound of each cost color but the last; 0.63 is a flat official forest trail (D32). */
export const COST_STOPS = [0.8, 1.1, 1.6, 2.4];
/** One term at one stretch relative to its best (1 = ideal): upper bound of each color but the last. */
export const TERM_STOPS = [1.05, 1.2, 1.5, 2.2];
export const termBucket = (ratio: number) => { const i = TERM_STOPS.findIndex((s) => ratio <= s); return i < 0 ? TERM_STOPS.length : i; };
/** Index into the cost ramp for a cost per meter. */
export const costBucket = (c: number) => { const i = COST_STOPS.findIndex((s) => c <= s); return i < 0 ? COST_STOPS.length : i; };

/** `step` color expression over line progress, one stop per change of cost bucket. */
function costGradient(r: Route, ramp: string[]): maplibregl.ExpressionSpecification | null {
  const coords = r.geometry.coordinates, cost = r.cost_per_m;
  if (!cost?.length || coords.length < 2) return null;
  const d = cumulative(coords), total = d[d.length - 1]!;
  if (!(total > 0)) return null;
  const bucket = (c: number) => ramp[Math.min(costBucket(c), ramp.length - 1)]!;
  const out: unknown[] = ["step", ["line-progress"], bucket(cost[0]!)];
  let prev = out[2], last = 0;
  for (let i = 1; i < coords.length - 1; i++) {
    const col = bucket(cost[i]!), p = d[i]! / total;
    if (col !== prev && p > last) { out.push(p, col); prev = col; last = p; }
  }
  return out as maplibregl.ExpressionSpecification;
}

maplibregl.setWorkerUrl(workerUrl);

const SWITZERLAND: [[number, number], [number, number]] = [[5.9, 45.8], [10.5, 47.85]];
const ROUTES = "routes";
const START = "start";
const WAYPOINTS = "waypoints";
const HOVER = "hover";
const HERE = "here";
const EMPTY = { type: "FeatureCollection" as const, features: [] };
const point = (p: LatLon | null) => ({
  type: "FeatureCollection" as const,
  features: p ? [{ type: "Feature" as const, properties: {}, geometry: { type: "Point" as const, coordinates: [p.lon, p.lat] } }] : [],
});

/** Hillshade strength per base: swisstopo topo already carries relief, imagery has real shadows. */
const RELIEF_STRENGTH: Record<string, number> = { outdoor: 0.45, topo: 0.2, satellite: 0.15 };
const OUTDOOR_PREFIX = "o-";

function buildStyle(base: BaseMapId, overlays: readonly string[], c: MapColors): StyleSpecification {
  const on = (v: boolean) => (v ? "visible" : "none") as "visible" | "none";
  const outdoor = base === "outdoor";
  const sources: StyleSpecification["sources"] = {
    [OMT]: { type: "vector", url: OPENMAPTILES, attribution: OPENMAPTILES_ATTRIBUTION },
    terrain: { type: "raster-dem", url: TERRAIN, tileSize: 512, encoding: "terrarium" },
    trails: { type: "raster", ...TRAILS_SOURCE },
  };
  for (const def of BASE_MAPS) if (def.raster) sources[def.id] = { type: "raster", ...def.raster };
  const withVis = (ls: LayerSpecification[], v: boolean) => ls.map((l) => ({ ...l, layout: { ...l.layout, visibility: on(v) } }) as LayerSpecification);
  const overlay = (id: string) => on(overlays.includes(id));

  // Order: outdoor fills, swisstopo rasters, relief, forest, outdoor lines, trails, labels; routes go on top at load.
  const layers: LayerSpecification[] = [
    ...withVis(outdoorFills(c.palette), outdoor),
    ...BASE_MAPS.filter((d) => d.raster).map((d) => ({
      id: `base-${d.id}`, type: "raster", source: d.id, layout: { visibility: on(d.id === base) },
    }) as LayerSpecification),
    {
      id: "overlay-relief", type: "hillshade", source: "terrain", layout: { visibility: overlay("relief") },
      paint: {
        "hillshade-exaggeration": RELIEF_STRENGTH[base] ?? 0.3,
        "hillshade-shadow-color": c.shade,
        "hillshade-highlight-color": "#ffffff",
        "hillshade-accent-color": c.shade,
      },
    },
    {
      id: "overlay-forest", type: "fill", source: OMT, "source-layer": "landcover",
      filter: ["==", ["get", "class"], "wood"], layout: { visibility: overlay("forest") },
      paint: { "fill-color": c.forest, "fill-opacity": 0.45 },
    },
    ...withVis(outdoorLines(c.palette), outdoor),
    { id: "overlay-trails", type: "raster", source: "trails", layout: { visibility: overlay("trails") } },
    ...withVis(outdoorLabels(c.palette), outdoor),
  ];
  // Every overlay in the registry must own exactly one layer here.
  console.assert(OVERLAYS.every((o) => layers.some((l) => l.id === `overlay-${o.id}`)), "overlay without a layer");
  return { version: 8, glyphs: GLYPHS, sources, layers };
}

export interface MapHandlers {
  onRouteClick?: (id: string) => void;
  onMapClick?: (p: LatLon) => void;
  /** Right-click or touch long-press: map position and screen point inside the container. */
  onContextMenu?: (p: LatLon, at: { x: number; y: number }) => void;
  /** The map started moving (closes transient UI such as the context menu). */
  onMoveStart?: () => void;
  /** Pointer over the selected route (null when it leaves). */
  onRouteHover?: (p: LatLon | null) => void;
  /** Drag edits on the selected route (D54): `i` indexes its waypoints, `leg` the leg after waypoint `leg`. */
  onWaypointMove?: (i: number, p: LatLon) => void;
  onWaypointInsert?: (leg: number, p: LatLon) => void;
  onWaypointRemove?: (i: number) => void;
}

export class MapController {
  readonly map: maplibregl.Map;
  private ready: Promise<void>;
  private routes: Route[] = [];
  private selected: string | null = null;
  private costOn = false;
  /** The selected route's waypoints; the first and last (the start) get no handle. */
  private wps: LatLon[] = [];
  private drag: { i: number; insert: boolean; at: LatLon | null; from: maplibregl.Point } | null = null;
  private justDragged = false;

  constructor(
    container: HTMLElement, base: BaseMapId, overlays: readonly string[], private colors: MapColors, handlers: MapHandlers = {},
  ) {
    this.map = new maplibregl.Map({
      container,
      style: buildStyle(base, overlays, colors),
      bounds: SWITZERLAND,
      maxBounds: [[4.5, 45.3], [11.9, 48.3]],
      attributionControl: false,
      dragRotate: false,
      pitchWithRotate: false,
      touchPitch: false,
    });
    this.map.touchZoomRotate.disableRotation();
    // Left unset, `compact` folds the credits into an ⓘ button on narrow maps (phones): it starts
    // open and collapses once the map is moved; desktop keeps the full line.
    this.map.addControl(new maplibregl.AttributionControl({
      customAttribution: '<a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">© OpenStreetMap contributors</a>',
    }), "bottom-right");
    this.map.addControl(new maplibregl.ScaleControl({ unit: "metric" }), "bottom-right");

    this.ready = new Promise((resolve) => this.map.on("load", () => {
      this.addRouteLayers();
      resolve();
    }));

    this.map.on("click", (e) => {
      if (this.justDragged) { this.justDragged = false; return; }
      const hit = this.map.queryRenderedFeatures(e.point, { layers: ["route-hit"] })[0];
      const id = hit?.properties?.id;
      if (typeof id === "string") handlers.onRouteClick?.(id);
      else handlers.onMapClick?.({ lat: e.lngLat.lat, lon: e.lngLat.lng });
    });
    // Listening also makes MapLibre suppress the browser menu on the canvas.
    this.map.on("contextmenu", (e) => handlers.onContextMenu?.({ lat: e.lngLat.lat, lon: e.lngLat.lng }, e.point));
    this.map.on("movestart", () => handlers.onMoveStart?.());
    let overSelected = false;
    this.map.on("mousemove", (e) => {
      if (!this.selected || !this.map.getLayer("route-hit")) return;
      const hit = this.map.queryRenderedFeatures(e.point, { layers: ["route-hit"] })
        .some((f) => f.properties?.id === this.selected);
      if (hit) handlers.onRouteHover?.({ lat: e.lngLat.lat, lon: e.lngLat.lng });
      else if (overSelected) handlers.onRouteHover?.(null);
      overSelected = hit;
    });
    this.map.on("mouseenter", "route-hit", () => (this.map.getCanvas().style.cursor = "pointer"));
    this.map.on("mouseleave", "route-hit", () => (this.map.getCanvas().style.cursor = ""));
    this.attachDrag(handlers);
  }

  /** Drag a handle to move it, drag the selected line to add a point, click a handle to remove it. */
  private attachDrag(h: MapHandlers) {
    const canvas = () => this.map.getCanvas();
    const begin = (e: maplibregl.MapMouseEvent | maplibregl.MapTouchEvent, i: number, insert: boolean) => {
      e.preventDefault(); // keeps the map from panning
      this.drag = { i, insert, at: null, from: e.point };
      canvas().style.cursor = "grabbing";
    };
    const handleAt = (e: maplibregl.MapMouseEvent | maplibregl.MapTouchEvent) =>
      this.map.queryRenderedFeatures(e.point, { layers: ["waypoints"] })[0];
    const onHandle = (e: maplibregl.MapMouseEvent | maplibregl.MapTouchEvent) => {
      const f = handleAt(e);
      if (f && this.drag === null && this.wps.length) begin(e, Number(f.properties?.i), false);
    };
    this.map.on("mousedown", "waypoints", onHandle);
    this.map.on("touchstart", "waypoints", (e) => { if (e.points.length === 1) onHandle(e); });
    // shortcut: adding points by dragging the line is mouse-only, so touch panning over the route still pans; add a long-press if phones need it.
    this.map.on("mousedown", "route-hit", (e) => {
      const r = this.routes.find((x) => x.id === this.selected);
      const onSel = e.features?.some((f) => f.properties?.id === this.selected);
      if (!r || !onSel || handleAt(e) || this.wps.length < 2 || e.originalEvent.button !== 0) return;
      const k = nearestIndex(r.geometry.coordinates, e.lngLat.lng, e.lngLat.lat);
      begin(e, legAt(r.geometry.coordinates, this.wps, k), true);
    });
    const move = (e: maplibregl.MapMouseEvent | maplibregl.MapTouchEvent) => {
      // A few pixels of jitter is still a click (removes a handle).
      if (!this.drag || (!this.drag.at && this.drag.from.dist(e.point) < 5)) return;
      this.drag.at = { lat: e.lngLat.lat, lon: e.lngLat.lng };
      const shown = [...this.wps];
      if (this.drag.insert) shown.splice(this.drag.i + 1, 0, this.drag.at);
      else shown[this.drag.i] = this.drag.at;
      this.drawHandles(shown);
    };
    this.map.on("mousemove", move);
    this.map.on("touchmove", move);
    const end = () => {
      const d = this.drag;
      if (!d) return;
      this.drag = null;
      this.justDragged = true;
      canvas().style.cursor = "";
      if (d.at && d.insert) h.onWaypointInsert?.(d.i, d.at);
      else if (d.at) h.onWaypointMove?.(d.i, d.at);
      else if (!d.insert) h.onWaypointRemove?.(d.i);
      else this.justDragged = false; // a plain click on the line selects as before
      this.drawHandles(this.wps);
    };
    this.map.on("mouseup", end);
    this.map.on("touchend", end);
    this.map.on("mouseenter", "waypoints", () => (canvas().style.cursor = "grab"));
    this.map.on("mouseleave", "waypoints", () => { if (!this.drag) canvas().style.cursor = ""; });
  }

  private addRouteLayers() {
    const c = this.colors;
    this.map.addSource(ROUTES, { type: "geojson", lineMetrics: true, data: { type: "FeatureCollection", features: [] } });
    this.map.addSource(START, { type: "geojson", data: { type: "FeatureCollection", features: [] } });
    this.map.addSource(WAYPOINTS, { type: "geojson", data: { type: "FeatureCollection", features: [] } });
    this.map.addSource(HOVER, { type: "geojson", data: EMPTY });
    this.map.addSource(HERE, { type: "geojson", data: EMPTY });
    const isSel: FilterSpecification = ["==", ["get", "selected"], true];
    const notSel: FilterSpecification = ["!=", ["get", "selected"], true];
    const join = { "line-join": "round", "line-cap": "round" } as const;
    // Line form carries state: candidates thin and muted, selected heavy and in the accent.
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
    // Debug view: the selected line colored by cost per meter (D54).
    this.map.addLayer({
      id: "route-cost", type: "line", source: ROUTES, layout: { ...join, visibility: "none" },
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
    this.map.addLayer({
      id: "waypoints", type: "circle", source: WAYPOINTS,
      paint: { "circle-radius": 6, "circle-color": c.selected, "circle-stroke-color": c.casing, "circle-stroke-width": 2.5 },
    });
    this.map.addLayer({
      id: "here", type: "circle", source: HERE,
      paint: { "circle-radius": 7, "circle-color": c.here, "circle-stroke-color": c.casing, "circle-stroke-width": 3 },
    });
    this.map.addLayer({
      id: "hover", type: "circle", source: HOVER,
      paint: { "circle-radius": 6.5, "circle-color": c.casing, "circle-stroke-color": c.selected, "circle-stroke-width": 3.5 },
    });
  }

  setBaseMap(id: BaseMapId) {
    void this.ready.then(() => {
      for (const def of BASE_MAPS) {
        if (def.raster) this.map.setLayoutProperty(`base-${def.id}`, "visibility", def.id === id ? "visible" : "none");
      }
      for (const l of this.map.getStyle().layers) {
        if (l.id.startsWith(OUTDOOR_PREFIX)) this.map.setLayoutProperty(l.id, "visibility", id === "outdoor" ? "visible" : "none");
      }
      this.map.setPaintProperty("overlay-relief", "hillshade-exaggeration", RELIEF_STRENGTH[id] ?? 0.3);
    });
  }

  setOverlay(id: string, on: boolean) {
    void this.ready.then(() => this.map.setLayoutProperty(`overlay-${id}`, "visibility", on ? "visible" : "none"));
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

  /** Dev route mode: every point is drawn (and draggable). */
  async setWaypoints(points: LatLon[]) {
    await this.ready;
    this.wps = [];
    this.drawPoints(points.map((p, i) => [p, i]));
  }

  /** The selected loop's waypoints; the start and finish get no handle. */
  async setHandles(points: LatLon[]) {
    await this.ready;
    this.wps = points;
    this.drawHandles(points);
  }

  private drawHandles(points: LatLon[]) {
    this.drawPoints(points.map((p, i) => [p, i] as [LatLon, number]).slice(1, -1));
  }

  private drawPoints(points: [LatLon, number][]) {
    (this.map.getSource(WAYPOINTS) as GeoJSONSource).setData({
      type: "FeatureCollection",
      features: points.map(([p, i]) => ({
        type: "Feature", properties: { i }, geometry: { type: "Point", coordinates: [p.lon, p.lat] },
      })),
    });
  }

  async setCostView(on: boolean) {
    this.costOn = on;
    await this.ready;
    this.redraw();
  }

  fitBounds(b: { min_lat: number; min_lon: number; max_lat: number; max_lon: number }, padding: maplibregl.PaddingOptions) {
    this.map.fitBounds([[b.min_lon, b.min_lat], [b.max_lon, b.max_lat]], { padding, duration: 600 });
  }

  fitRoute(id: string, padding: maplibregl.PaddingOptions) {
    const r = this.routes.find((x) => x.id === id);
    if (!r) return;
    this.map.fitBounds(bounds(r.geometry.coordinates), { padding, duration: 600, maxZoom: 15 });
  }

  flyTo(p: LatLon, zoom = 13) {
    this.map.flyTo({ center: [p.lon, p.lat] as LngLatLike, zoom, duration: 800 });
  }

  /** Marker on the selected route where the elevation chart is hovered. */
  async setHover(p: LatLon | null) {
    await this.ready;
    (this.map.getSource(HOVER) as GeoJSONSource).setData(point(p));
  }

  /** The user's location dot. */
  async setHere(p: LatLon | null) {
    await this.ready;
    (this.map.getSource(HERE) as GeoJSONSource).setData(point(p));
  }

  zoomIn() { this.map.zoomIn(); }
  zoomOut() { this.map.zoomOut(); }

  private redraw() {
    const sel = this.routes.find((r) => r.id === this.selected);
    const grad = this.costOn && sel ? costGradient(sel, this.colors.cost) : null;
    this.map.setLayoutProperty("route-cost", "visibility", grad ? "visible" : "none");
    if (grad) this.map.setPaintProperty("route-cost", "line-gradient", grad);
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
