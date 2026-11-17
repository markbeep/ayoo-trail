// Map layer registry.
//
// Base maps are exclusive: exactly one is visible. Overlays (snow, heatmaps,
// viewshed, ...) will stack between the base map and the routes; the registry
// has a slot for them so adding one does not reshape the map code. Whether a
// future layer is an overlay or a base map is still open (PRODUCT.md).

export interface RasterLayerDef {
  id: string;
  label: string;
  tiles: string[];
  tileSize: number;
  maxzoom: number;
  attribution: string;
}

export type BaseMapDef = RasterLayerDef;

export interface OverlayDef extends RasterLayerDef {
  /** 0–1, applied as raster-opacity. */
  opacity: number;
}

const WMTS = "https://wmts.geo.admin.ch/1.0.0";
const SWISSTOPO = '<a href="https://www.swisstopo.admin.ch/" target="_blank" rel="noopener">© swisstopo</a>';

export const BASE_MAPS: readonly BaseMapDef[] = [
  {
    id: "topo",
    label: "Topo",
    tiles: [`${WMTS}/ch.swisstopo.pixelkarte-farbe/default/current/3857/{z}/{x}/{y}.jpeg`],
    tileSize: 256,
    maxzoom: 18,
    attribution: SWISSTOPO,
  },
  {
    id: "satellite",
    label: "Satellite",
    tiles: [`${WMTS}/ch.swisstopo.swissimage/default/current/3857/{z}/{x}/{y}.jpeg`],
    tileSize: 256,
    maxzoom: 20,
    attribution: SWISSTOPO,
  },
];

export const OVERLAYS: readonly OverlayDef[] = [];

export type BaseMapId = (typeof BASE_MAPS)[number]["id"];
export const DEFAULT_BASE_MAP: BaseMapId = "topo";
