# ayoo

Read this first, then `docs/brief.md`.

## Phase

**v1 built** (2026-11-20, `docs/milestones.md`): import, generator, API, client
with drag editing, avoid areas, share links and GPX, Docker image. Next work
comes from the backlog and open questions in `docs/brief.md`.

## Layout

- `src/` — Rust crate (`trail-calc`, lib `trail_calc`), `src/bin/server.rs` is `trail-calc-server`.
- `web/` — Vite + Solid + MapLibre client. Routes come from `/api/generate`
  (`web/src/state.ts`); the layer registry is in `web/src/map/basemaps.ts`.
- `PRODUCT.md`, `.impeccable/` — impeccable design context. The surface brief
  `.impeccable/surfaces/web-src-app-tsx.md` is the visual direction contract.
- `just dev` (server + Vite), `just web`, `just check`, `just smoke`.

## What it is

A web app that generates trail-running routes algorithmically: give it a
distance and a set of preference layers (views, surface, technicality,
forest, away from towns, avoid/prefer areas) and it returns scenic routes — quick
defaults for casual use, many more controls for power users. Layers are
pluggable, and layer gating/tiers may come later.

## Docs

| File | Role |
|------|------|
| `docs/brief.md` | Vision, goals, feature backlog, open questions. |
| `docs/adr/` | One file per decision (`NNNN-slug.md`), from 2026-11-24 on. |
| `docs/decisions.md` | Closed log of decisions D1–D116; read-only. |
| `docs/agents/` | Config for the engineering skills (issue tracker, labels, domain docs). |
| `docs/layers.md` | Per-layer data sources, licenses, algorithms, precompute cost. |
| `docs/design.md` | As-built design: architecture, graph format, cost model, generation, API, frontend, setup. |
| `docs/milestones.md` | v1 plan status (done). |
| `PRODUCT.md` | Product context for design work (impeccable). |
| `DESIGN.md` | Visual design system (impeccable), written from the built UI. |

## Working rules

- Version control is **jj**. Use only jj commands: `jj status`, `jj diff`,
  `jj log`, `jj commit -m "..."`, `jj bookmark set`.
- Work in small steps: failing test, implement, pass, `jj commit`. Do not skip
  the test step or widen a step's scope.
- Do not invent product decisions. Anything unclear goes into the Open Questions
  section of `docs/brief.md` and gets asked.
- The owner wants to be asked about details rather than have choices made for
  them, especially for product scope, UX, and data sources. Technical
  implementation choices may be proposed as a default with the reason.
- Record every settled decision as a new ADR in `docs/adr/` in the same turn,
  and move the question out of the open list. `docs/decisions.md` is closed
  at D116; read it, don't append to it.
- Mark inferences as inferences; never present a guess as a requirement.

## Agent skills

### Issue tracker

GitHub Issues on `markbeep/ayoo-trail`, via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

The default five: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `GLOSSARY.md` plus `docs/adr/` (D1–D116 stay in `docs/decisions.md`). See `docs/agents/domain.md`.
