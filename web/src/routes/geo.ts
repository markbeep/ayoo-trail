const R = 6_371_000;
const rad = (d: number) => (d * Math.PI) / 180;

/** Great-circle distance in meters between two [lon, lat] positions. */
export function haversine(a: readonly number[], b: readonly number[]): number {
  const [lon1 = 0, lat1 = 0] = a;
  const [lon2 = 0, lat2 = 0] = b;
  const dLat = rad(lat2 - lat1);
  const dLon = rad(lon2 - lon1);
  const h = Math.sin(dLat / 2) ** 2 + Math.cos(rad(lat1)) * Math.cos(rad(lat2)) * Math.sin(dLon / 2) ** 2;
  return 2 * R * Math.asin(Math.sqrt(h));
}

/** Bounding box [[west, south], [east, north]] of a coordinate list. */
export function bounds(coords: readonly (readonly number[])[]): [[number, number], [number, number]] {
  let w = Infinity, s = Infinity, e = -Infinity, n = -Infinity;
  for (const [lon = 0, lat = 0] of coords) {
    w = Math.min(w, lon); e = Math.max(e, lon);
    s = Math.min(s, lat); n = Math.max(n, lat);
  }
  return [[w, s], [e, n]];
}

/** Distance in meters from the first position to each position. */
export function cumulative(coords: readonly (readonly number[])[]): number[] {
  const out = [0];
  for (let i = 1; i < coords.length; i++) out.push(out[i - 1]! + haversine(coords[i - 1]!, coords[i]!));
  return out;
}

/** Index of the position nearest [lon, lat] (planar, fine at route scale). */
export function nearestIndex(coords: readonly (readonly number[])[], lon: number, lat: number): number {
  const k = Math.cos((lat * Math.PI) / 180);
  let best = 0, bestD = Infinity;
  coords.forEach(([x = 0, y = 0], i) => {
    const d = ((x - lon) * k) ** 2 + (y - lat) ** 2;
    if (d < bestD) { bestD = d; best = i; }
  });
  return best;
}

/** Coordinate index of each waypoint along the route, in order: the first is 0, the last the end
 * (a loop's start and finish coincide), the rest searched forward from the previous one. */
export function waypointIndices(coords: readonly (readonly number[])[], wps: readonly { lat: number; lon: number }[]): number[] {
  let from = 0;
  return wps.map((p, i) => {
    if (i === 0) return 0;
    if (i === wps.length - 1) return coords.length - 1;
    from += nearestIndex(coords.slice(from), p.lon, p.lat);
    return from;
  });
}

/** The leg (between waypoints `leg` and `leg + 1`) that coordinate `k` lies on. */
export function legAt(coords: readonly (readonly number[])[], wps: readonly { lat: number; lon: number }[], k: number): number {
  const idx = waypointIndices(coords, wps);
  let leg = 0;
  for (let i = 0; i < idx.length - 1; i++) if (idx[i]! <= k) leg = i;
  return leg;
}
