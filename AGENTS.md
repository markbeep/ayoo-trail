# trail-route

Read this first, then `docs/brief.md`.

## Phase

**Scaffold built; engine still paused.** The owner approved `docs/design.md`
in principle and `docs/milestones.md` holds the 16-task plan. On 2026-11-18 the
owner asked for the scaffold only (D24): the Rust server shell and the client
with map, base-map switching and route rendering from synthetic data. Do not
start the routing algorithm, importer or graph work (plan tasks 1–12) until the
owner says so. Parts of tasks 1, 11 and 13 already exist as scaffold; reconcile
them rather than recreating files when the plan resumes.

## Layout

- `src/` — Rust crate (`trail_route`), `src/bin/server.rs` is `trail-server`.
- `web/` — Vite + Solid + MapLibre client. Synthetic routes live in
  `web/src/routes/mock.ts`; the layer registry in `web/src/map/basemaps.ts`.
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
