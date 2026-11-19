// GPX 1.1 export of one route (D47): a single track with elevation per point.
import type { Route } from "./types";

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

export function toGpx(route: Route, name: string): string {
  const pts = route.geometry.coordinates
    .map(([lon, lat, z]) => `      <trkpt lat="${lat.toFixed(6)}" lon="${lon.toFixed(6)}"><ele>${z.toFixed(1)}</ele></trkpt>`)
    .join("\n");
  return `<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="trail-route" xmlns="http://www.topografix.com/GPX/1/1">
  <metadata><name>${esc(name)}</name></metadata>
  <trk>
    <name>${esc(name)}</name>
    <type>trail_running</type>
    <trkseg>
${pts}
    </trkseg>
  </trk>
</gpx>
`;
}

/** "Loop A" + 15 300 m → "trail-route-loop-a-15km.gpx". */
export function gpxFileName(loopName: string, distance_m: number): string {
  const slug = loopName.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
  return `trail-route-${slug}-${Math.round(distance_m / 1000)}km.gpx`;
}

export function downloadGpx(route: Route, loopName: string, title: string) {
  const url = URL.createObjectURL(new Blob([toGpx(route, title)], { type: "application/gpx+xml" }));
  const a = Object.assign(document.createElement("a"), { href: url, download: gpxFileName(loopName, route.distance_m) });
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
