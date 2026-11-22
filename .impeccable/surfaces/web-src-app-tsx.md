---
version: 1
slug: "web-src-app-tsx"
primary_target: "web/src/App.tsx"
related_targets: []
---

# Surface: the planner (web/src/App.tsx)

Mode: Operate. Trail runners plan loops on a laptop and tweak them on a phone at the trailhead.
Job: pick a start and distance, read and tune every routing setting, compare loops, edit, export.
Constraints: map stays the main work area; map attribution and scale stay uncovered; phone bottom sheet behaviour (D105) stays.
Owner direction (2026-11-22): much less rounding, much less padding, dense rows, power-user friendly, no generic AI look.

## Direction contract

THESIS: The planner reads like a Swiss printed timetable (Kursbuch): loops and settings are rows in strict columns with tabular figures and hairline rules. It refuses the category's rounded floating cards with soft shadows and roomy padding.

OWN-WORLD: Paper white #fbfbf8 ground, ink #141414, one black header bar, stone #efefea for value wells and header rows, hairline #c9c9c3/#dcdcd6 rules, visible vertical column rules. Signal red #d5281b only for the active loop and the primary action; avoid/prefer words in dark red/green. Archivo for text and figures, Archivo Narrow for column heads. Corners 0–2 px. Rows 26–28 px. Stale/refreshing rows change line form (dashed rule), not hue.

STORY: The runner sees start, distance and Find loops in one ruled row, reads the loops as timetable rows (km, up, down, score) with the chosen one inverted, scrubs any × in the settings table, and the map's red loop follows.

FIRST VIEWPORT: Desktop: 400 px left panel flush to the edge, full height: black header bar with wordmark; query row; loops table; settings table in the same column rhythm; route detail below. Map fills the rest, layers card squared at top right. Phone: same panel as the bottom sheet, square top edge.

FORM: Kursbuch timetable, position 4 of my ordered list (assigned by the roll). seed:65f4ad04898bf149. Raises: emission-line rail (state by line form), Crouwel grid (visible column rules), monochrome canon (one neutral scale, red used sparingly).

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
