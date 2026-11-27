# 0007: Only ways that exist on swisstopo's map

Date: 2026-11-27. Source: owner.

## Decision

Routes use only OSM ways that swissTLM3D also maps, unless the user turns
this off. The switch is **on by default** and sits with Marked trails.

- Source: `TLM_STRASSE` from the full swissTLM3D (every road and path in CH and
  FL, including unmarked `1m Weg`, `2m Weg` and `Markierte Spur`). Same OGD
  terms as the Wanderwege file.
- Download: only the `swissTLM3D_TLM_STRASSE.shp` entry of the SHP zip
  (434 MB compressed, 1.1 GB raw), fetched by HTTP Range. The whole zip is
  3.6 GB and the GeoPackage 4.8 GB as one file, so neither is downloaded.
- Matching: the same rule as official trails (≥ 50 % of an edge's 10 m samples
  within 20 m of a TLM line). The result is the edge flag `IN_TLM` in
  `graph.bin` (format version 3), so the raw file need not be kept.
- With the switch on, edges without the flag are impassable, like edges above
  the hardest allowed grade. Edges outside CH and FL are blocked too; the owner
  expects that from the option.
- "Clear all" leaves the switch on, as it leaves the hardest grade at T3
  (builder's call).
- Share links carry `v=0` when the switch is off; no `v` means on.

## Why

OSM keeps paths that no longer exist. A shared loop near Ebnat-Kappel used two
stretches (14.2–14.4 km and 14.5–14.6 km) that are neither on swisstopo nor on
the ground. swisstopo's map is more current for Switzerland.
