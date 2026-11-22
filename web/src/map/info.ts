// Info overlays (D102, D103): Windy webcams through our server, SchweizMobil hiking routes through
// geo.admin.ch identify. Popups are built with DOM nodes, never innerHTML: titles come from third parties.

import type { LatLon } from "../routes/types";

export interface WebcamProps { id: number; title: string; image: string | null; url: string }
export interface SchweizMobilRoute { number: number; title: string }
interface WebcamCollection {
  type: "FeatureCollection";
  features: { type: "Feature"; geometry: { type: "Point"; coordinates: [number, number] }; properties: WebcamProps }[];
}

/** Webcams start at this zoom, so one request (≤ 50 webcams) covers the view. */
export const WEBCAM_MIN_ZOOM = 10;
export const WINDY_CREDIT = '<a href="https://www.windy.com/webcams" target="_blank" rel="noopener">Webcams provided by windy.com</a>';

/** GeoJSON webcams in a lon/lat box, or null when the server has none (no key, provider down). */
export async function fetchWebcams(west: number, south: number, east: number, north: number) {
  const bbox = [north, east, south, west].map((x) => x.toFixed(4)).join(",");
  const res = await fetch(`/api/webcams?bbox=${bbox}`);
  return res.ok ? ((await res.json()) as WebcamCollection) : null;
}

/** Distinct SchweizMobil hiking routes within `tolerancePx` of a point, by number. */
export async function identifySchweizMobil(p: LatLon, bounds: [number, number, number, number], size: { w: number; h: number }, tolerancePx = 8) {
  const q = new URLSearchParams({
    geometryType: "esriGeometryPoint", geometry: `${p.lon},${p.lat}`, sr: "4326",
    mapExtent: bounds.join(","), imageDisplay: `${size.w},${size.h},96`, tolerance: String(tolerancePx),
    layers: "all:ch.astra.wanderland", returnGeometry: "false",
  });
  const res = await fetch(`https://api3.geo.admin.ch/rest/services/api/MapServer/identify?${q}`);
  if (!res.ok) return [];
  const body = (await res.json()) as { results?: { attributes?: { chmobil_route_number?: number; chmobil_title?: string } }[] };
  const seen = new Map<number, SchweizMobilRoute>();
  for (const r of body.results ?? []) {
    const n = r.attributes?.chmobil_route_number;
    if (typeof n === "number" && !seen.has(n)) seen.set(n, { number: n, title: r.attributes?.chmobil_title ?? `Route ${n}` });
  }
  return [...seen.values()].sort((a, b) => a.number - b.number);
}

export const schweizMobilUrl = (n: number) => `https://schweizmobil.ch/en/hiking-in-switzerland/route-${n}`;

const el = <K extends keyof HTMLElementTagNameMap>(tag: K, props: Partial<HTMLElementTagNameMap[K]> = {}, ...kids: Node[]) => {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...kids);
  return e;
};
const out = (href: string, text: string) => el("a", { href, target: "_blank", rel: "noopener", textContent: text });

export function webcamPopup(w: WebcamProps): HTMLElement {
  const box = el("div", { className: "info-pop" }, el("strong", { textContent: w.title }));
  // Never stretched (Windy's terms): natural size, scaled down only to fit.
  if (w.image) box.append(el("a", { href: w.url, target: "_blank", rel: "noopener" }, el("img", { src: w.image, alt: w.title, className: "info-img" })));
  box.append(out(w.url, "Open on Windy"), el("small", { textContent: "Webcams provided by windy.com" }));
  return box;
}

export function schweizMobilPopup(routes: SchweizMobilRoute[]): HTMLElement {
  const list = el("ul");
  for (const r of routes) list.append(el("li", {}, out(schweizMobilUrl(r.number), `${r.number} · ${r.title}`)));
  return el("div", { className: "info-pop" }, el("strong", { textContent: "SchweizMobil hiking" }), list);
}
