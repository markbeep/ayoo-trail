// Our own calm outdoor base map over OpenMapTiles (D40). Colors come from the
// `--map-*` CSS tokens, so a palette change restyles map and UI together.

import type { LayerSpecification } from "maplibre-gl";

export interface MapPalette {
  land: string;
  wood: string;
  grass: string;
  rock: string;
  ice: string;
  water: string;
  road: string;
  roadCasing: string;
  path: string;
  label: string;
  halo: string;
}

export const OMT = "openmaptiles";
const FONT = ["Noto Sans Regular"];
const FONT_BOLD = ["Noto Sans Bold"];
const z = (stops: [number, number][]) => ["interpolate", ["linear"], ["zoom"], ...stops.flat()] as unknown as number;

/** Area fills, drawn under the swisstopo rasters and the hillshade. */
export function outdoorFills(c: MapPalette): LayerSpecification[] {
  return [
    { id: "o-land", type: "background", paint: { "background-color": c.land } },
    {
      id: "o-landcover", type: "fill", source: OMT, "source-layer": "landcover",
      paint: {
        "fill-color": ["match", ["get", "class"],
          "wood", c.wood, "grass", c.grass, "farmland", c.grass, "wetland", c.grass,
          "rock", c.rock, "sand", c.rock, "ice", c.ice, c.land],
      },
    },
    {
      id: "o-park", type: "fill", source: OMT, "source-layer": "park",
      paint: { "fill-color": c.grass, "fill-opacity": 0.5 },
    },
    { id: "o-water", type: "fill", source: OMT, "source-layer": "water", paint: { "fill-color": c.water } },
  ];
}

/** Lines above the hillshade: water, roads, paths, borders. */
export function outdoorLines(c: MapPalette): LayerSpecification[] {
  const roads = ["match", ["get", "class"], ["motorway", "trunk", "primary", "secondary", "tertiary", "minor", "service"], true, false];
  const width = z([[10, 0.6], [14, 3], [17, 9]]);
  return [
    {
      id: "o-waterway", type: "line", source: OMT, "source-layer": "waterway",
      paint: { "line-color": c.water, "line-width": z([[10, 0.6], [15, 2]]) },
    },
    {
      id: "o-road-casing", type: "line", source: OMT, "source-layer": "transportation", minzoom: 11,
      filter: roads as never, layout: { "line-join": "round", "line-cap": "round" },
      paint: { "line-color": c.roadCasing, "line-width": z([[11, 1.4], [14, 4.5], [17, 11]]) },
    },
    {
      id: "o-road", type: "line", source: OMT, "source-layer": "transportation",
      filter: roads as never, layout: { "line-join": "round", "line-cap": "round" },
      paint: { "line-color": c.road, "line-width": width },
    },
    {
      id: "o-path", type: "line", source: OMT, "source-layer": "transportation", minzoom: 12,
      filter: ["match", ["get", "class"], ["path", "track"], true, false],
      paint: {
        "line-color": c.path,
        "line-width": z([[12, 0.7], [15, 1.4], [17, 2.2]]),
        "line-dasharray": [3, 1.5],
      },
    },
    {
      id: "o-rail", type: "line", source: OMT, "source-layer": "transportation",
      filter: ["==", ["get", "class"], "rail"],
      paint: { "line-color": c.label, "line-width": z([[10, 0.6], [15, 1.5]]), "line-opacity": 0.45 },
    },
    {
      id: "o-border", type: "line", source: OMT, "source-layer": "boundary",
      filter: ["all", ["==", ["get", "admin_level"], 2], ["!=", ["get", "maritime"], 1]],
      paint: { "line-color": c.label, "line-width": 1.2, "line-opacity": 0.4, "line-dasharray": [4, 2] },
    },
  ];
}

/** Labels above the overlays, under the routes. */
export function outdoorLabels(c: MapPalette): LayerSpecification[] {
  const text = { "text-color": c.label, "text-halo-color": c.halo, "text-halo-width": 1.4 };
  return [
    {
      id: "o-peak", type: "symbol", source: OMT, "source-layer": "mountain_peak", minzoom: 11,
      filter: ["==", ["get", "class"], "peak"],
      layout: {
        "text-field": ["concat", "▲ ", ["get", "name"], ["case", ["has", "ele"], ["concat", "\n", ["to-string", ["get", "ele"]], " m"], ""]],
        "text-font": FONT, "text-size": 11, "text-anchor": "top",
      },
      paint: text,
    },
    {
      id: "o-place", type: "symbol", source: OMT, "source-layer": "place",
      filter: ["match", ["get", "class"], ["city", "town", "village", "hamlet"], true, false],
      layout: {
        "text-field": ["coalesce", ["get", "name:latin"], ["get", "name"]],
        "text-font": ["match", ["get", "class"], ["city", "town"], ["literal", FONT_BOLD], ["literal", FONT]] as never,
        "text-size": ["match", ["get", "class"], "city", 16, "town", 14, "village", 12, 11] as never,
      },
      paint: text,
    },
  ];
}
