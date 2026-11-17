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

export function lineLength(coords: readonly (readonly number[])[]): number {
  let d = 0;
  for (let i = 1; i < coords.length; i++) d += haversine(coords[i - 1]!, coords[i]!);
  return d;
}

/** Gain and loss in meters with a vertical deadband, like the importer (design §7). */
export function climbAndDescent(coords: readonly (readonly number[])[], deadband = 2): [number, number] {
  let gain = 0;
  let loss = 0;
  let ref = coords[0]?.[2] ?? 0;
  for (const c of coords) {
    const z = c[2] ?? ref;
    const dz = z - ref;
    if (dz >= deadband) {
      gain += dz;
      ref = z;
    } else if (dz <= -deadband) {
      loss -= dz;
      ref = z;
    }
  }
  return [gain, loss];
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
