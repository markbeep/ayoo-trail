# navilau

Read this first, then `docs/brief.md`.

## Phase

**Engine and importer in implementation (D29).** Plan tasks 1–8 and 11 plus a
dev-only `?dev=route` client mode are being built, with the 2026-11-18
revisions (D30–D37) overriding `docs/milestones.md` where they differ. Loop
generation and `/api/generate` (D39) plus the map/overlay redesign (D40–D42)
are in progress; no response cache (generation takes tens of ms, D39).
Fine-grained weights and climb goals (D51–D53, D55–D56) and drag editing with
colour-by-cost (D54) are built.

## Layout

- `src/` — Rust crate (`navilau`), `src/bin/server.rs` is `navilau-server`.
- `web/` — Vite + Solid + MapLibre client. Routes come from `/api/generate`
  (`web/src/state.ts`); the layer registry is in `web/src/map/basemaps.ts`.
- `PRODUCT.md`, `.impeccable/` — impeccable design context. The surface brief
  `.impeccable/surfaces/web-src-app-tsx.md` is the visual direction contract.
- `just dev` (server + Vite), `just web`, `just check`.

## What it is

A web app that generates trail-running routes algorithmically: give it a
distance and a set of preference layers (views, remoteness, surface,
technicality, snow, POIs, transit access) and it returns scenic routes — quick
defaults for casual use, many more controls for power users. Layers are
pluggable, and layer gating/tiers may come later.

## Docs

| File | Role |
|------|------|
| `docs/brief.md` | Vision, goals, full feature backlog, open questions. Live source of truth during brainstorming. |
| `docs/decisions.md` | Append-only log of decisions and why. |
| `docs/routing-engines.md` | Research: engine options, comparison, recommendation. |
| `docs/layers.md` | Per-layer data sources, licences, algorithms, precompute cost. |
| `docs/design.md` | Approved design: architecture, graph format, cost model, API, frontend, setup. |
| `docs/milestones.md` | 16-task implementation plan, executed subagent-driven. |
| `PRODUCT.md` | Product context for design work (impeccable). |
| `DESIGN.md` | Visual design system (impeccable), written from the built UI. |

## Working rules

- Version control is **jj**. Use only jj commands: `jj status`, `jj diff`,
  `jj log`, `jj commit -m "..."`, `jj bookmark set`.
- Execute `docs/milestones.md` task by task: failing test, implement, pass,
  `jj commit`. Do not skip the test step or widen a task's scope.
- Do not invent product decisions. Anything unclear goes into the Open Questions
  section of `docs/brief.md` and gets asked.
- The owner wants to be asked about details rather than have choices made for
  them, especially for product scope, UX, and data sources. Technical
  implementation choices may be proposed as a default with the reason.
- Record every settled decision in `docs/decisions.md` in the same turn, and
  move the question out of the open list.
- Mark inferences as inferences; never present a guess as a requirement.
