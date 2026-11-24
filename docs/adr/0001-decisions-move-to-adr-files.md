# 0001: Decisions move from docs/decisions.md to ADR files

Date: 2026-11-24. Source: owner, during setup of the engineering skills.

## Decision

From now on, each settled decision is a file in `docs/adr/` (`NNNN-slug.md`).
`docs/decisions.md` is closed at D116: it stays as the read-only record of
D1–D116 and gets no new entries.

## Why

The engineering skills (`domain-modeling`, `grill-with-docs`,
`improve-codebase-architecture`) read and write ADRs in `docs/adr/`. Keeping
two live logs would split the record, so the owner chose one place for new
decisions.
