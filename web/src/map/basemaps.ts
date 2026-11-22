// Map layer registry (D40–D42).
//
// Base maps are exclusive: exactly one is visible. Overlays stack on any base
// map and are toggled independently; each overlay is one map layer
// `overlay-<id>` that the controller shows or hides.

export interface RasterSource {
  tiles: string[];
  tileSize: number;
  maxzoom: number;
  attribution: string;
}

export interface BaseMapDef {
  id: string;
  label: string;
  /** Raster bases are swisstopo; `outdoor` is our own vector style (no raster). */
  raster?: RasterSource;
}

export interface OverlayDef {
  id: string;
  label: string;
  defaultOn: boolean;
}

const WMTS = "https://wmts.geo.admin.ch/1.0.0";
const SWISSTOPO = '<a href="https://www.swisstopo.admin.ch/" target="_blank" rel="noopener">© swisstopo</a>';

/** OpenFreeMap planet tiles, OpenMapTiles schema, z0–14 (D40). */
export const OPENMAPTILES = "https://tiles.openfreemap.org/planet";
export const GLYPHS = "https://tiles.openfreemap.org/fonts/{fontstack}/{range}.pbf";
export const OPENMAPTILES_ATTRIBUTION =
  '<a href="https://openfreemap.org" target="_blank" rel="noopener">OpenFreeMap</a> <a href="https://www.openmaptiles.org/" target="_blank" rel="noopener">© OpenMapTiles</a>';
/** Mapterhorn terrain (terrarium, 512 px), Europe at high resolution. */
export const TERRAIN = "https://tiles.mapterhorn.com/tilejson.json";

export const BASE_MAPS: readonly BaseMapDef[] = [
  { id: "outdoor", label: "Outdoor" },
  {
    id: "topo",
    label: "Swiss topo",
    raster: {
      tiles: [`${WMTS}/ch.swisstopo.pixelkarte-farbe/default/current/3857/{z}/{x}/{y}.jpeg`],
      tileSize: 256,
      maxzoom: 18,
      attribution: SWISSTOPO,
    },
  },
  {
    id: "satellite",
    label: "Satellite",
    raster: {
      tiles: [`${WMTS}/ch.swisstopo.swissimage/default/current/3857/{z}/{x}/{y}.jpeg`],
      tileSize: 256,
      maxzoom: 20,
      attribution: SWISSTOPO,
    },
  },
];

export const OVERLAYS: readonly OverlayDef[] = [
  { id: "relief", label: "Relief shading", defaultOn: true },
  { id: "trails", label: "Marked trails", defaultOn: false },
  { id: "forest", label: "Forest", defaultOn: false },
  { id: "view", label: "360° view", defaultOn: false },
];

/** Our own 360° view tiles (D100), rendered by trail-calc-server from view.bin. MapLibre needs an absolute URL. */
export const VIEW_SOURCE: RasterSource = {
  tiles: [`${globalThis.location?.origin ?? ""}/api/tiles/view/{z}/{x}/{y}`],
  tileSize: 256,
  maxzoom: 14,
  attribution: SWISSTOPO,
};

/** shortcut: Switzerland only (swisstopo Wanderwege); add Waymarked Trails per region once its tile terms are checked. */
export const TRAILS_SOURCE: RasterSource = {
  tiles: [`${WMTS}/ch.swisstopo.swisstlm3d-wanderwege/default/current/3857/{z}/{x}/{y}.png`],
  tileSize: 256,
  maxzoom: 18,
  attribution: SWISSTOPO,
};

/** Planned overlays, listed so the panel already has its future shape. Not selectable. */
export const LATER_OVERLAYS = [
  { id: "popular", label: "Popular trails" },
  { id: "steepness", label: "Path steepness" },
  { id: "sun", label: "Sun & shade" },
] as const;

export type BaseMapId = (typeof BASE_MAPS)[number]["id"];
export const DEFAULT_BASE_MAP: BaseMapId = "outdoor";
export const DEFAULT_OVERLAYS: readonly string[] = OVERLAYS.filter((o) => o.defaultOn).map((o) => o.id);
