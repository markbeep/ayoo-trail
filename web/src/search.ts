// Place search for the start field: swisstopo SearchServer, called from the browser (D57).
import type { LatLon } from "./routes/types";

/** `area` is the municipality swisstopo appends after " - ", when there is one. */
export interface Place { name: string; kind: string; area: string | null; pos: LatLon }

const URL_BASE = "https://api3.geo.admin.ch/rest/services/api/SearchServer";
/** swisstopo origins without an <i>kind</i> in the label. */
const ORIGIN_KIND: Record<string, string> = { gg25: "Municipality", zipcode: "Postcode", address: "Address", district: "District", kantone: "Canton" };

const strip = (s: string) =>
  s.replace(/<[^>]*>/g, "").replace(/&amp;/g, "&").replace(/&#39;|&apos;/g, "'").replace(/&quot;/g, '"').replace(/\s+/g, " ").trim();

/** One SearchServer result as plain text (labels arrive as HTML, which is never rendered). */
export function toPlace(r: { attrs: { label: string; lat: number; lon: number; origin?: string } }): Place {
  const label = r.attrs.label;
  const kind = /<i>(.*?)<\/i>/.exec(label)?.[1];
  const text = strip(kind ? label.replace(/<i>.*?<\/i>/, "") : label);
  const cut = text.indexOf(" - ");
  const k = kind ? strip(kind) : (ORIGIN_KIND[r.attrs.origin ?? ""] ?? "Place");
  return {
    name: cut > 0 ? text.slice(0, cut) : text,
    kind: k.charAt(0) + k.slice(1).toLowerCase(), // sentence case: "Populated Place" → "Populated place"
    area: cut > 0 ? text.slice(cut + 3) : null,
    pos: { lat: r.attrs.lat, lon: r.attrs.lon },
  };
}

export async function searchPlaces(text: string, signal?: AbortSignal): Promise<Place[]> {
  const q = new URLSearchParams({ searchText: text, type: "locations", sr: "4326", limit: "6" });
  const res = await fetch(`${URL_BASE}?${q}`, { signal });
  if (!res.ok) throw new Error(`search failed (${res.status})`);
  const body = (await res.json()) as { results?: Parameters<typeof toPlace>[0][] };
  return (body.results ?? []).map(toPlace).filter((p) => Number.isFinite(p.pos.lat) && Number.isFinite(p.pos.lon));
}
