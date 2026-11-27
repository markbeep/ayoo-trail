# 0008: Settings sit under the score bars

Date: 2026-11-27. Source: owner. Refines D107, D115, D116; returns to D27's
"explain, then adjust".

## Decision

The settings card goes. Each preference is edited on the score bar it acts on.

- One row per score bar, under the bar's name (Marked trails, Surface,
  Gradient, Technical terrain, Forest, Away from towns, Quiet roads, Views).
  Tapping a row opens its controls inline.
- With no route yet, the same rows show without bars, where the route card
  will be.
- The climb goal (Off, Gentle, Hilly, Mountain, Max, target, how strictly to
  follow it) moves to the query card next to the distance. The Elevation bar
  only shows how well the goal was met.
- The Marked trails panel holds the way limits: its ×, the hardest allowed
  grade (T1–T6) and the switch "swisstopo paths only" (ADR 0007; the
  owner picked this label over the longer "Only ways on swisstopo's map"). The Technical
  terrain panel keeps only "harder grades" and its strength.
- Reuse, start stem, overlap and the avoid/prefer area strengths go into one
  collapsed "Advanced" fold at the end of the rows.
- Less text: no intro sentence and no per-row hints; a panel has one short
  hint line. Defaults and Clear all stay.
- The custom distance takes one decimal (16.5 km).

## Why

The owner found the settings card convoluted: generic groups (Ground, Hills,
Technical, Climb) that don't say what's in them, Views under "Ground", too
much text, and setting names that differ from the bars. They liked adjusting a
setting under the bar that scores it.
