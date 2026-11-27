---
name: ayoo
description: Trail-running loop planner set like a Swiss printed timetable (Kursbuch).
colors:
  paper: "#fbfbf8"
  ink: "#141414"
  well: "#efefea"
  ground: "#e6e5df"
  ground-deep: "#e4e4dd"
  muted: "#5b5b56"
  line: "#dcdcd6"
  signal: "#d5281b"
  signal-deep: "#a91d12"
  avoid: "#a3231a"
  prefer: "#1f6b3a"
  warn: "#8a4b0c"
  warn-wash: "#f7ecdd"
  route-active: "#d5281b"
  route-candidate: "#5c5c58"
  route-casing: "#ffffff"
  here: "#1a8cff"
  cost-1: "#2f8a57"
  cost-2: "#9bb53a"
  cost-3: "#e0a92a"
  cost-4: "#d9661f"
  cost-5: "#b3261e"
  map-land: "#f2efe9"
  map-wood: "#dde4cf"
  map-grass: "#e9ecdc"
  map-rock: "#e6e1d8"
  map-ice: "#f5f8fa"
  map-water: "#bfd4df"
  map-road: "#ffffff"
  map-road-casing: "#d9d1c4"
  map-path: "#8c7158"
  map-label: "#4d4652"
  map-halo: "#f2efe9"
  map-shade: "#3b342f"
  map-forest: "#5f8a4c"
typography:
  wordmark:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 800
    letterSpacing: "-0.01em"
  figure-lead:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "24px"
    fontWeight: 800
    lineHeight: 1
    fontFeature: "tnum"
  headline:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "18px"
    fontWeight: 800
    letterSpacing: "-0.01em"
  title:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 800
  body:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.35
    fontFeature: "tnum"
  row:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 700
    fontFeature: "tnum"
  caption:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 500
  column-head:
    fontFamily: "Archivo Narrow, Archivo, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 700
rounded:
  none: "0px"
spacing:
  cell: "6px"
  gap: "8px"
  edge: "12px"
  row: "28px"
  panel: "400px"
components:
  button-primary:
    backgroundColor: "{colors.signal}"
    textColor: "{colors.route-casing}"
    rounded: "{rounded.none}"
    height: "38px"
    typography: "{typography.wordmark}"
  button-primary-hover:
    backgroundColor: "{colors.signal-deep}"
  button-primary-unavailable:
    backgroundColor: "{colors.well}"
    textColor: "{colors.muted}"
  segment:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    rounded: "{rounded.none}"
    height: "30px"
    typography: "{typography.row}"
  segment-on:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.paper}"
  icon-button:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    rounded: "{rounded.none}"
    size: "36px"
  loop-row:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    height: "{spacing.row}"
    padding: "0 12px"
  loop-row-on:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.paper}"
  scrub-field:
    backgroundColor: "{colors.well}"
    textColor: "{colors.ink}"
    rounded: "{rounded.none}"
    padding: "2px 6px"
  column-head-row:
    backgroundColor: "{colors.well}"
    textColor: "{colors.muted}"
    typography: "{typography.column-head}"
    height: "22px"
---

# Design System: ayoo

## Overview

**Creative North Star: "The Kursbuch"**

The planner reads like a Swiss printed timetable. Loops and settings are rows in strict columns, figures are tabular, and structure is drawn with ink and hairline rules rather than with boxes floating over the map. One paper column, 400px wide, sits flush against the left edge at full height; the map fills everything else. On a phone the same column becomes the bottom sheet with a square top edge.

Density is the point. Rows are 28px, cell padding is 6px, section padding is 10–12px. The system is monochrome (one warm-grey neutral scale from paper to ink) and spends colour only where it carries meaning: signal red for the active loop and the one primary action, dark red and green for the words "avoid" and "prefer", amber for warnings, and the map's own palette underneath. It replaces the D43 rounded-card consumer look (D114); rounded floating cards with soft shadows and roomy padding are the confirmed anti-reference.

**Key Characteristics:**
- One flush paper column, ruled into sections by 1px ink lines.
- Visible column rules: 1px hairlines between every column of a table row.
- Square corners everywhere; no shadows on UI surfaces.
- Ink is the pressable colour; the selected state is inverted (ink ground, paper text).
- Signal red is rare: the active loop on the map, its score, the primary button, the caret.
- State is shown by line form (a dashed rule for stale results), not by hue.

## Colors

A one-scale monochrome of warm paper and near-black ink, with a single signal red and a few semantic word colours.

### Primary
- **Ink** (ink): text, every 1px structural rule, pressable outlines, the black header bar, the inverted selected row and the "on" segment.
- **Signal Red** (signal): the active loop on the map, the active loop's score, the elevation-profile cursor, the primary "Find loops" button, the input caret and the wordmark's trail line. Hover and busy states darken to **Deep Signal** (signal-deep).

### Neutral
- **Paper** (paper): the panel, menus, map tools, the layers card. Every UI surface is paper.
- **Stone Well** (well): value wells (scrub fields, the elevation area), column-head rows, group-head rows, row hover.
- **Ground** (ground) and **Deep Ground** (ground-deep): page background and the map's empty canvas; deep ground also marks hover on in-field buttons.
- **Muted** (muted): column heads, units, hints, captions, the "acts on" column.
- **Hairline** (line): row rules, column rules, segment dividers, quiet button outlines.

### Semantic
- **Avoid Red** (avoid) and **Prefer Green** (prefer): only the × values in the preference rows. They are words, never fills.
- **Warn Amber** (warn) on **Warn Wash** (warn-wash): warning badges and query errors, always with a 1px currentColor border.
- **Location Blue** (here): the your-location dot, the platform convention, kept apart from the red route.
- **Route Candidate** (route-candidate) and **Route Casing** (route-casing): unselected loops on the map and the white casing under every loop line.

### Map and cost
The outdoor base map (map-land through map-forest) is quiet and warm so the red loop reads above it. The five-step cost ramp (cost-1 green through cost-5 red, cheap to expensive) belongs to the "colour by cost" debug view (D54) only.

### Named Rules
**The Signal Rule.** Red marks exactly one thing per screen: the active loop (its map line and score) and the action that produces loops. Never use signal red for a heading, a link, a decoration or a second button.

**The Ink-Is-Pressable Rule.** Buttons, segments, toggles and fields are drawn in ink. Selected means inverted: ink ground, paper text.

## Typography

**Body Font:** Archivo (self-hosted variable, 100–900, with system-ui)
**Label Font:** Archivo Narrow (self-hosted, 400–700, falls back to Archivo)

**Character:** A plain grotesque set heavy (600–800) at small sizes, with tabular figures on by default for the whole page; the narrow cut is the timetable's column-head register.

### Hierarchy
- **Figure lead** (800, 24px, line-height 1): the selected loop's score on the route card.
- **Headline** (800, 18px, −0.01em): the route card's loop name.
- **Wordmark** (800, 15px, −0.01em): the header bar and the primary button label.
- **Title** (800, 14px): section heads such as "Settings".
- **Body** (400, 14px, 1.35): base text; empty-state guidance.
- **Row** (600–800, 13px): table rows, segments, chips, menu items. Labels 700, figures 800, secondary cells 600.
- **Caption** (500–600, 12px, muted): hints, profile axis, notes, search result kinds.
- **Column head** (Archivo Narrow 700, 12px, muted, sentence case): table column heads, query cell labels ("Start", "Distance, km"), layers-card section titles. Group-head explanations use the narrow cut at 400.

### Named Rules
**The Tabular Rule.** Every figure is tabular (`font-variant-numeric: tabular-nums` on body) and right-aligned in its column.

**The Narrow Head Rule.** Archivo Narrow is for column heads and cell labels only. Never set body copy, buttons or values in it.

## Layout

A single 400px column flush left, full height, bounded by a 1px ink rule on its right; the map takes the rest. Inside, sections stack edge to edge with 1px ink rules between them: header bar (36px), query, loops table, route detail with the preference rows (or the preference rows alone before a route exists). Horizontal inset is 12px everywhere.

Tables share one rhythm: 28px rows, 1px hairline under each row, 1px hairline between columns, 6px cell padding, figures right-aligned. Column heads sit on a stone row with an ink rule below. Loops use columns `1fr 64px 64px 52px` (loop, km, up, score); preference rows are described below. The query is one ruled grid: start across the top, distance segments and the primary button, then the climb segments across, then target and "stick to it" side by side.

Map tools stack at the top right, 12px from the edges (10px on phones), with 6px gaps; zoom buttons share a border.

Below 760px the panel becomes a bottom sheet with snap heights (closed tab 30px, peek 188px, half 52dvh, full 100dvh − 72px), a square ink top edge and a 36×3px grab bar. The wordmark floats top left, and the map attribution and scale move to the top so the sheet never covers them. Inputs go to 16px on coarse pointers so iOS does not zoom.

## Elevation & Depth

Flat. UI surfaces carry no shadows; depth is drawn with 1px ink borders and the paper-on-ground contrast. MapLibre popups have their shadows removed and take an ink border. The only box-shadows are functional rings: the selected basemap swatch (a paper gap and a 2px ink ring), a 15% ink inset on swatches, and the paper halo around the elevation-profile dot.

### Named Rules
**The Ruled Not Floated Rule.** Separate things with a 1px rule, not with a shadow, gap or card. A section ends with an ink line.

## Shapes

Square. Every radius is 0, including MapLibre's attribution, its info button and its popups, which are overridden. The one circle is the elevation-profile dot (9px), a plotted point rather than a container. Borders are always 1px: ink for structure and pressable outlines, hairline for row and column rules, dashed ink for stale state.

## Components

### Buttons
- **Primary** ("Find loops"): signal fill, white 800 text, square, 38px min height (fills its grid cell in the query). Hover and active darken to deep signal. Busy keeps deep signal with a spinning loader. With no start chosen (D111) it goes plainly unavailable: stone well, muted text, hairline border, not-allowed cursor.
- **Icon button** (map tools): 36px paper square, 1px ink border, stone hover. On the route card, 30px squares with a hairline border that turns ink on hover.
- **Link button** ("Change", "Clear all"): ink 700 text with a hairline underline offset 3px; the underline turns ink on hover.
- **Focus:** a 2px ink outline everywhere, offset 1px (inset −2px inside fields and menus).

### Segments
- **Style:** a row of equal cells in a 1px ink frame, hairline dividers, 30px tall (28px in preference panels), 700 13px.
- **State:** hover fills stone; on is inverted ink. A custom-value cell ("Other") is an inline input inside the same frame.

### Loops table (signature)
Timetable rows, one per loop: name, km, up, score. The column head row is stone with Archivo Narrow heads. The selected loop is inverted (ink ground, paper text) and its score turns a light signal red so it stays legible on ink. Rows fade to 35% while a live refresh replaces their figures.

### Preference rows (signature)
Preferences live on the score bars (ADR 0008). One 30px row per bar, named as the bar: name `116px`, bar track `1fr`, score `30px`, the × it is set to `62px` (coloured avoid or prefer by its effect, muted "off" at ×1), chevron `14px`. Without a route the bar and score cells stay empty and the rows still line up. Tapping a row turns it stone and opens its panel below, hung off a 1px ink rule on the left: an optional muted fact line (the loop's figure for this term), then control lines (`1fr 104px`, label and scrub field, hairline between). The Marked trails panel also holds the T1–T6 segment and the "swisstopo paths only" switch. A collapsed "Advanced" fold ends the list. The head row carries "Why it scores N" (or "Preferences") with Defaults and Clear all as link buttons.

### Scrub field
A right-aligned 800 tabular value on a stone well. Drag left or right to change it on a log scale; click to type (D106). Hover draws an ink border, the cursor is ew-resize; editing swaps to a paper input with an ink border. Disabled drops to 45% opacity.

### Inputs / Fields
- **Style:** stone well, hairline border, 34px, 700 text; inside the query grid the field drops its border and background and sits in the ruled cell.
- **Focus:** paper ground with an ink border; caret is signal red.
- **Search results:** a ruled list in an ink frame, 36px rows, the keyboard-highlighted row in stone.

### Layers card and map menu
Paper panels with a 1px ink border, square. Section titles are Archivo Narrow over an ink rule. Basemap rows show a 28×20 swatch; overlays end in a square switch (ink frame, ink knob that slides 12px and inverts when on). The map context menu is a 228px ruled list of 34px rows.

### Stale and edited notices
A full-width line inside the route card, 13px 700: stale results get a dashed ink border (state by line form), an edited route a solid ink border.

### Header bar and icon
The ink bar carries the wordmark in paper with the trail mark in signal red. The home-screen icon (D113) is the same mark: a red trail line from a paper-white start square on ink.

## Do's and Don'ts

### Do:
- **Do** put new controls in the 400px column as ruled rows on the 28px rhythm with visible column rules.
- **Do** draw pressable things in ink and show selection by inverting to ink with paper text.
- **Do** separate sections with a 1px ink rule and rows with a 1px hairline.
- **Do** set column heads and cell labels in Archivo Narrow 700 12px, muted, sentence case.
- **Do** show changed state by line form (dashed, solid) before reaching for a new colour.
- **Do** keep the map palette and the cost ramp exactly as tokenised; the route must read above the map.

### Don't:
- **Don't** use rounded corners, floating cards or drop shadows on UI surfaces.
- **Don't** spend signal red on anything but the active loop and the primary action.
- **Don't** fill rows or cells with avoid or prefer colours; they colour the effect word only.
- **Don't** pad roomily: 12px section inset, 6px cell padding, 28px rows.
