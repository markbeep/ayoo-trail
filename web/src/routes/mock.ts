// Synthetic demo routes. They do not follow trails: they are wobbly loops
// generated around a start point so the map, list and profile can be built
// before the routing engine exists. Every route is flagged `synthetic`.

import type { LatLon, Route } from "./types";
import { climbAndDescent, lineLength } from "./geo";

interface Shape {
  heading: number; // degrees, direction of the loop's centre from the start
  radiusKm: number;
  wobble: number[]; // harmonic amplitudes
  relief: number; // elevation amplitude in m
  baseElev: number;
  official: number;
  score: number;
  surface: number;
  steepDownKm: number;
  forest: number;
  remote: number;
}

const SHAPES: Shape[] = [
  { heading: 200, radiusKm: 2.3, wobble: [0.18, 0.07, 0.05], relief: 230, baseElev: 640, official: 0.84, score: 88, surface: 0.76, steepDownKm: 0.8, forest: 0.64, remote: 0.9 },
  { heading: 320, radiusKm: 2.2, wobble: [0.1, 0.12, 0.04], relief: 190, baseElev: 620, official: 0.71, score: 81, surface: 0.82, steepDownKm: 0.3, forest: 0.48, remote: 0.74 },
  { heading: 95, radiusKm: 2.6, wobble: [0.22, 0.05, 0.08], relief: 260, baseElev: 600, official: 0.62, score: 74, surface: 0.58, steepDownKm: 1.4, forest: 0.71, remote: 0.66 },
];

function loop(start: LatLon, s: Shape, targetM: number, climbM: number): [number, number, number][] {
  const mPerDegLat = 111_320;
  const mPerDegLon = 111_320 * Math.cos((start.lat * Math.PI) / 180);
  const h = (s.heading * Math.PI) / 180;
  // Scale the radius so the loop length lands near the target.
  const r = (s.radiusKm * 1000 * targetM) / 15_000;
  const cx = start.lon + (Math.sin(h) * r) / mPerDegLon;
  const cy = start.lat + (Math.cos(h) * r) / mPerDegLat;
  const phase0 = Math.atan2(start.lat - cy, (start.lon - cx) * (mPerDegLon / mPerDegLat));
  const n = 240;
  const out: [number, number, number][] = [];
  for (let i = 0; i <= n; i++) {
    const t = (i / n) * Math.PI * 2;
    let k = 1;
    s.wobble.forEach((a, j) => (k += a * Math.sin((j + 2) * t)));
    const a = phase0 + t;
    const rr = r * (i === 0 || i === n ? 1 : k);
    const lon = cx + (Math.cos(a) * rr) / mPerDegLon;
    const lat = cy + (Math.sin(a) * rr) / mPerDegLat;
    // Two big climbs plus rolling terrain; amplitude scaled so total gain lands near the target.
    const amp = (climbM * (s.relief / 230)) / 6.2;
    const z = s.baseElev + amp * (1 - Math.cos(2 * t)) + amp * 0.3 * Math.sin(9 * t);
    out.push([lon, lat, Math.round(z)]);
  }
  return out;
}

export function mockRoutes(start: LatLon, targetM = 15_000, climbM = 900): Route[] {
  return SHAPES.map((s, i) => {
    const coordinates = loop(start, s, targetM, climbM);
    const [climb, descent] = climbAndDescent(coordinates);
    const distance = lineLength(coordinates);
    return {
      id: `demo-${i + 1}`,
      geometry: { type: "LineString", coordinates },
      waypoints: [start, ...[0.33, 0.66].map((f) => {
        const c = coordinates[Math.round(f * (coordinates.length - 1))]!;
        return { lon: c[0], lat: c[1] };
      }), start],
      distance_m: distance,
      climb_m: climb,
      descent_m: descent,
      score: s.score,
      official_share: s.official,
      breakdown: [
        { key: "official", value: s.official, detail: `${Math.round(s.official * 100)} % on marked hiking trails` },
        { key: "surface", value: s.surface, detail: `${Math.round((1 - s.surface) * distance / 1000 * 10) / 10} km on asphalt or paved paths` },
        { key: "slope", value: Math.max(0, 1 - s.steepDownKm / 2.5), detail: `${s.steepDownKm} km of steep downhill` },
        { key: "forest", value: s.forest, detail: `${Math.round(s.forest * 100)} % through forest` },
        { key: "urban", value: s.remote, detail: s.remote > 0.85 ? "Barely touches a village" : "Passes through a village" },
      ],
      distance_met: Math.abs(distance - targetM) / targetM <= 0.1,
      graph_date: null,
      synthetic: true,
    };
  });
}

/** Uetliberg, Zürich: default start until the user picks one. */
export const DEFAULT_START: LatLon = { lat: 47.3496, lon: 8.4919 };
