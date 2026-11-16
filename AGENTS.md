# trail-route

Read this first, then `docs/brief.md`.

## Phase

**Design iteration — implementation paused.** The owner approved
`docs/design.md` in principle and `docs/milestones.md` holds the 16-task
implementation plan, but the owner has explicitly paused execution: the project
is still in the design phase. Do not write product code, scaffold, or start the
plan until the owner says so.

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
