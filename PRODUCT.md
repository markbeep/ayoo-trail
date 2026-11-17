# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Stack

Settled in `docs/design.md` §5–6 before init: Rust (axum) server that also
serves the static client; TypeScript + SolidJS + MapLibre GL client built with
Vite. No SSR, no PWA/offline (D23).

## Users

Trail runners in Switzerland who plan loop runs ahead of time. The first user is
the owner, who today plans in a six-tool loop (relief map, SBB, TrailRouter,
gpx.studio, swisstopo, condition sites) and wants it collapsed into one screen.
The interface is designed for the public from the start: a trail runner who has
never seen the app must be able to use it without explanation.

Usage is split equally between planning on a laptop and checking or tweaking a
route on a phone before a run.

## Product Purpose

Generate scenic trail-running loops from a start point, a target distance and a
few preferences, quickly enough to replace manual planning. Success: open the
site, set start, distance and hilliness, get several genuinely different loops
that already respect trail existence, surface, technicality and steepness, drag
a point to adjust, export the GPX.

## Positioning

Routes are generated against verification data a runner otherwise checks by
hand: official Swiss hiking-network membership, swisstopo maps and imagery,
slope direction (steep up preferred over steep down). Each route states its
official-trail share honestly instead of hiding weak data.

## Operating Context

- Map-centric: swisstopo topographic map (default) and SWISSIMAGE satellite,
  fetched directly from `wmts.geo.admin.ch`. Satellite exists to confirm that a
  trail is real.
- Further map layers will be added later (snow, heatmaps, viewshed, sun/shade,
  POIs); whether these are overlays or separate base layers is undecided.
- Output goes to GPX, then to a sports watch (e.g. Coros).

## Capabilities and Constraints

- v1: Switzerland only; loops only; no accounts, billing or database.
- Quick path is minimal (start, distance, hilliness, one button); power controls
  (weight sliders, max SAC grade, exclusion markers) exist but stay collapsed.
- Current scaffold stage: map navigation, base-layer switching, and route line
  rendering from mock data. No routing backend yet. Manual click-to-draw is not
  in scope (D13).
- Attribution required: ©swisstopo on the map, © OpenStreetMap contributors.
- Undecided: tiering/paywalls for layers (later), overlay vs. base-layer model
  for future layers.

## Brand Commitments

Product name "trail-route" is a working name. No logo, voice or visual assets
exist yet.

## Evidence on Hand

No screenshots, testimonials, users or benchmarks exist. Route data in the
scaffold is synthetic and must be labelled as such.

## Product Principles

1. The map is the work surface; controls serve it and never crowd it.
2. Trust over flattery: show where data is weak (unofficial trails, missing tags).
3. Quick by default, deep on request; never open into a wall of sliders.
4. Layers are additive; new ones must not reshape the core screen.

## Accessibility & Inclusion

Used outdoors on phones before runs as well as at a desk: legible in bright
light, touch targets sized for a phone. No formal standard set beyond that.
