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

/** Cost weights (D51): each term is `factor^w`, w in 0–4, 0 ignores it, 1 is the default.
 * `uphill`/`downhill` run −1 gentle … 0 any … +1 steep (D53). */
export interface Weights {
  slope: number;
  uphill: number;
  downhill: number;
  surface: number;
  tech: number;
  official: number;
  forest: number;
  urban: number;
  /** 0–1, set by the generator per route (D52); 0 for anything the user asks for. */
  climb_pressure?: number;
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
  /** Climb pressure this route was generated with; drag edits re-route with it. */
  climb_pressure: number;
  /** Cost per metre at each coordinate (the step leaving it). */
  cost_per_m: number[];
  /** From coordinate `i` on, each term relative to its best possible value (1 = ideal), D61. */
  terms: ({ i: number } & Record<BreakdownKey, number>)[];
  graph_date: string | null;
}
