// Route payload, mirroring `docs/design.md` §10. The server will return this
// shape from `POST /api/generate` and `POST /api/route`.

export interface LatLon {
  lat: number;
  lon: number;
}

/** GeoJSON LineString whose positions are [lon, lat, elevation_m]. */
export interface RouteGeometry {
  type: "LineString";
  coordinates: [number, number, number][];
}

/** Design §8 cost weights; each 0–3, 1 is neutral. */
export interface Weights {
  slope: number;
  surface: number;
  tech: number;
  official: number;
  forest: number;
  urban: number;
}

export type BreakdownKey = "official" | "surface" | "slope" | "tech" | "forest" | "urban";

/** One term of the score, normalised so 1 is ideal. */
export interface BreakdownItem {
  key: BreakdownKey;
  value: number;
  /** Plain-language fact about this route, e.g. "0.8 km of steep downhill". */
  detail: string;
}

export interface Route {
  id: string;
  geometry: RouteGeometry;
  waypoints: LatLon[];
  distance_m: number;
  climb_m: number;
  descent_m: number;
  /** 0–100, see design §8. */
  score: number;
  /** Share of distance on the official hiking network, 0–1. */
  official_share: number;
  breakdown: BreakdownItem[];
  distance_met: boolean;
  graph_date: string | null;
}
