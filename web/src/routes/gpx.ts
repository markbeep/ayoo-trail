// GPX 1.1 export of one route (D47): a single track with elevation per point, and the
// user's checkpoints as waypoints (D98), which COROS shows as checkpoints.
import type { Checkpoint, LatLon, Route } from "./types";

/** Checkpoints further than this from the exported loop are left out. */
export const CHECKPOINT_MAX_M = 100;

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** Index of the track point nearest `p`, and its distance in meters (flat; fine at this range). */
export function nearestOnRoute(coords: readonly (readonly number[])[], p: LatLon): { i: number; d: number } {
  const k = Math.cos((p.lat * Math.PI) / 180);
  let best = { i: 0, d: Infinity };
  coords.forEach(([lon = 0, lat = 0], i) => {
    const d = Math.hypot((lon - p.lon) * k, lat - p.lat) * 111_320;
    if (d < best.d) best = { i, d };
  });
  return best;
}

/** Waypoints on the track point nearest each checkpoint (Suunto needs them exactly on it), in route order. */
function waypoints(route: Route, checkpoints: readonly Checkpoint[]): string {
  const coords = route.geometry.coordinates;
  return checkpoints
    .map((c, n) => ({ ...nearestOnRoute(coords, c), name: c.name.trim() || `CP${n + 1}` }))
    .filter((c) => c.d <= CHECKPOINT_MAX_M)
    .sort((a, b) => a.i - b.i)
    .map(({ i, name }) => {
      const [lon, lat, z] = coords[i]!;
      return `  <wpt lat="${lat.toFixed(6)}" lon="${lon.toFixed(6)}"><ele>${z.toFixed(1)}</ele><name>${esc(name)}</name></wpt>\n`;
    })
    .join("");
}

export function toGpx(route: Route, name: string, checkpoints: readonly Checkpoint[] = []): string {
  const pts = route.geometry.coordinates
    .map(([lon, lat, z]) => `      <trkpt lat="${lat.toFixed(6)}" lon="${lon.toFixed(6)}"><ele>${z.toFixed(1)}</ele></trkpt>`)
    .join("\n");
  return `<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="ayoo" xmlns="http://www.topografix.com/GPX/1/1">
  <metadata><name>${esc(name)}</name></metadata>
${waypoints(route, checkpoints)}  <trk>
    <name>${esc(name)}</name>
    <type>trail_running</type>
    <trkseg>
${pts}
    </trkseg>
  </trk>
</gpx>
`;
}

/** "Loop A" + 15 300 m → "ayoo-loop-a-15km.gpx". */
export function gpxFileName(loopName: string, distance_m: number): string {
  const slug = loopName.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
  return `ayoo-${slug}-${Math.round(distance_m / 1000)}km.gpx`;
}

export function downloadGpx(route: Route, loopName: string, title: string, checkpoints: readonly Checkpoint[] = []) {
  const url = URL.createObjectURL(new Blob([toGpx(route, title, checkpoints)], { type: "application/gpx+xml" }));
  const a = Object.assign(document.createElement("a"), { href: url, download: gpxFileName(loopName, route.distance_m) });
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
