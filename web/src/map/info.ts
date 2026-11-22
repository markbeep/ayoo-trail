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

/** SchweizMobil's own route-logo placements (what their map shows), LV03 points with an SVG logo each. */
export const SCHWEIZMOBIL_LOGOS = "https://data.schweizmobil.ch/route_logos/current_prod/logos_wander_segment.geojson";

/** LV03 (E, N) to WGS84 with swisstopo's approximate formulas (~1 m), enough for logo placement. */
export function lv03ToWgs84(e: number, n: number): LatLon {
  const y = (e - 600000) / 1e6, x = (n - 200000) / 1e6;
  const lon = 2.6779094 + 4.728982 * y + 0.791484 * y * x + 0.1306 * y * x * x - 0.0436 * y ** 3;
  const lat = 16.9023892 + 3.238272 * x - 0.270978 * y * y - 0.002528 * x * x - 0.0447 * y * y * x - 0.014 * x ** 3;
  return { lat: (lat * 100) / 36, lon: (lon * 100) / 36 };
}

/** Meters per CSS pixel at zoom 0 for MapLibre's 512 px tiles, at 46.8° N (mid Switzerland). */
const M_PER_PX_Z0 = (40075016.686 * Math.cos((46.8 * Math.PI) / 180)) / 512;
/** SchweizMobil lists, per logo, the map resolutions (m/px) it shows at, already thinned so logos
 * don't crowd; the coarsest one sets the zoom it appears from. */
export function logoMinZoom(resolutions: Record<string, unknown>): number {
  const coarsest = Math.max(...Object.keys(resolutions).map(Number).filter((r) => r > 0));
  return Number.isFinite(coarsest) ? Math.log2(M_PER_PX_Z0 / coarsest) : Infinity;
}

interface RawLogos { features: { geometry: { coordinates: number[] }; properties: { resolutions: Record<string, unknown>; logo_svg?: string; route: number } }[] }
export interface LogoProps { logo: string; route: number; minzoom: number }

export function toLogoFeatures(raw: RawLogos) {
  const features = raw.features.flatMap((f) => {
    const minzoom = logoMinZoom(f.properties.resolutions);
    const [e, n] = f.geometry.coordinates;
    if (!f.properties.logo_svg || !Number.isFinite(minzoom) || e === undefined || n === undefined) return [];
    const p = lv03ToWgs84(e, n);
    return [{
      type: "Feature" as const,
      geometry: { type: "Point" as const, coordinates: [p.lon, p.lat] as [number, number] },
      properties: { logo: f.properties.logo_svg, route: f.properties.route, minzoom } satisfies LogoProps,
    }];
  });
  return { type: "FeatureCollection" as const, features };
}

export async function fetchSchweizMobilLogos() {
  const res = await fetch(SCHWEIZMOBIL_LOGOS);
  return res.ok ? toLogoFeatures((await res.json()) as RawLogos) : null;
}

/** A logo SVG rasterized for MapLibre at `px` CSS pixels (drawn at 2×). */
export async function loadLogo(url: string, px: number) {
  const img = new Image(px * 2, px * 2);
  img.crossOrigin = "anonymous";
  img.src = url;
  await img.decode();
  const c = document.createElement("canvas");
  c.width = c.height = px * 2;
  const ctx = c.getContext("2d")!;
  ctx.drawImage(img, 0, 0, px * 2, px * 2);
  return ctx.getImageData(0, 0, px * 2, px * 2);
}

export const schweizMobilUrl =(n: number) => `https://schweizmobil.ch/en/hiking-in-switzerland/route-${n}`;

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
