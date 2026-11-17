---
name: trail-route
description: A warm, friendly trail-loop planner on a full-bleed swisstopo map, in stone and course purple.
colors:
  stone-ground: "#ece8e1"
  stone-deep: "#e2ddd4"
  card-white: "#ffffff"
  trail-ink: "#26222b"
  slate-muted: "#625b6b"
  hairline: "#e7e2da"
  course-purple: "#7a3fb0"
  course-purple-deep: "#622f91"
  purple-wash: "#f4eefa"
  amber-warn: "#a4570d"
  amber-wash: "#fbf0e2"
  route-active: "#7a3fb0"
  route-candidate: "#6f6878"
  route-casing: "#ffffff"
typography:
  display:
    fontFamily: "Nunito Sans Variable, Nunito Sans, system-ui, sans-serif"
    fontSize: "32px"
    fontWeight: 900
    lineHeight: 1
  headline:
    fontFamily: "Nunito Sans Variable, Nunito Sans, system-ui, sans-serif"
    fontSize: "23px"
    fontWeight: 900
    letterSpacing: "-0.015em"
  title:
    fontFamily: "Nunito Sans Variable, Nunito Sans, system-ui, sans-serif"
    fontSize: "17px"
    fontWeight: 800
    letterSpacing: "-0.005em"
  body:
    fontFamily: "Nunito Sans Variable, Nunito Sans, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: 1.45
    fontFeature: "tnum"
  label:
    fontFamily: "Nunito Sans Variable, Nunito Sans, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 800
  caption:
    fontFamily: "Nunito Sans Variable, Nunito Sans, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 600
rounded:
  inner: "10px"
  ctl: "13px"
  field: "14px"
  button: "16px"
  tile: "18px"
  card: "22px"
  sheet: "26px"
  pill: "999px"
spacing:
  tight: "8px"
  gap: "12px"
  edge: "16px"
  edge-phone: "12px"
  card-y: "18px"
  card-x: "20px"
  panel-width: "410px"
components:
  button-primary:
    backgroundColor: "{colors.course-purple}"
    textColor: "{colors.card-white}"
    typography: "{typography.title}"
    rounded: "{rounded.button}"
    height: "54px"
  button-primary-hover:
    backgroundColor: "{colors.course-purple-deep}"
  link-button:
    textColor: "{colors.course-purple}"
    typography: "{typography.label}"
  link-button-hover:
    textColor: "{colors.course-purple-deep}"
  round-button:
    backgroundColor: "{colors.card-white}"
    textColor: "{colors.trail-ink}"
    rounded: "{rounded.pill}"
    size: "44px"
  round-button-hover:
    backgroundColor: "{colors.stone-ground}"
  card:
    backgroundColor: "{colors.card-white}"
    textColor: "{colors.trail-ink}"
    rounded: "{rounded.card}"
    padding: "18px 20px"
  field:
    backgroundColor: "{colors.stone-ground}"
    textColor: "{colors.trail-ink}"
    rounded: "{rounded.field}"
    padding: "0 14px"
    height: "48px"
  field-active:
    backgroundColor: "{colors.purple-wash}"
    textColor: "{colors.course-purple-deep}"
  chip:
    backgroundColor: "{colors.stone-ground}"
    textColor: "{colors.trail-ink}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    padding: "0 13px"
    height: "36px"
  chip-hover:
    backgroundColor: "{colors.stone-deep}"
  segmented:
    backgroundColor: "{colors.stone-ground}"
    textColor: "{colors.slate-muted}"
    typography: "{typography.label}"
    rounded: "{rounded.ctl}"
    padding: "3px"
  segmented-on:
    backgroundColor: "{colors.course-purple}"
    textColor: "{colors.card-white}"
    rounded: "{rounded.inner}"
    height: "40px"
  route-tile:
    backgroundColor: "{colors.card-white}"
    textColor: "{colors.trail-ink}"
    rounded: "{rounded.tile}"
    padding: "11px 14px"
  badge-warn:
    backgroundColor: "{colors.amber-wash}"
    textColor: "{colors.amber-warn}"
    rounded: "{rounded.pill}"
    padding: "5px 10px"
---

# Design System: trail-route

This file covers the visual system only. The architecture spec (server, graph format, cost model, API) is in `docs/design.md`, which is a separate document.

## Overview

**Creative North Star: "The Course Card"**

trail-route works like an orienteering map: the swisstopo topo or satellite image fills the screen, and the planner sits on it as a few soft white cards on warm stone. The one colour that matters is course purple, the colour orienteering uses to overprint a course on a map. Here it marks the loop you have selected, the things you can press, and the score of the selected loop. Everything else stays ink, muted slate or stone, so the purple line on the map is always the loudest thing on screen.

The tone is friendly and plain. Labels are short sentence-case phrases ("Why it scores 88", "Find loops", "Adjust"). Rounded humanist type in heavy weights makes the numbers easy to read. The depth controls live next to the thing they change: each score bar opens its own setting inline, so the planner never needs a settings drawer. Density stays moderate. Cards are generous, but one column of 410px is enough to hold the whole quick path.

The map is the work surface. Cards float over it in a single left column on a laptop. On a phone they move into a bottom sheet that can be dragged between snap points. Data keeps a neutral voice: elevation, score bars and terrain never use the accent.

**Key Characteristics:**
- Full-bleed swisstopo map, with chrome as floating soft-cornered white cards
- One accent, course purple, reserved for the active route, pressable controls and the headline score
- Rounded humanist sans (Nunito Sans) in heavy weights, with tabular numerals throughout
- Soft, lifted cards on a warm stone ground; controls sit as stone wells inside the cards
- Plain-language, sentence-case labels; synthetic data is labelled as synthetic

## Colors

Warm neutrals with a single overprint accent: stone and ink carry the interface, and purple marks the course.

### Primary
- **Course Purple** (`course-purple`): the active route line on the map, the primary "Find loops" button, the selected segment in segmented controls, link buttons ("Change", "Adjust"), the selected-tile outline and score, the focus ring, and the headline score on the route card. The same value is exposed as `route-active` for map paint.
- **Course Purple Deep** (`course-purple-deep`): hover and busy state of the primary button and link buttons, and text on an active field.
- **Purple Wash** (`purple-wash`): background of an active field, the inline Adjust panel, and the "Settings changed" strip. It is a quiet stage for purple controls and never fills a surface on its own.

### Secondary
- **Amber Warn** (`amber-warn`) on **Amber Wash** (`amber-wash`): the only warning pair, used for honest data caveats such as "Closest match to your distance".

### Neutral
- **Stone Ground** (`stone-ground`): app background, the phone sheet surface, control wells (fields, chips, segmented tracks), score-bar tracks and the elevation-profile fill.
- **Stone Deep** (`stone-deep`): hover state for stone wells, and the map backdrop while tiles load.
- **Card White** (`card-white`): every floating card, tile, brand pill and round map button.
- **Trail Ink** (`trail-ink`): all primary text, score-bar fills, the elevation line and the map scale bar.
- **Slate Muted** (`slate-muted`): secondary text such as meta lines, hints, captions, unselected segments and "Later" tags. It is darkened from the sketch's #6f6878 so it reads on stone and white in bright outdoor light.
- **Hairline** (`hairline`): reserved divider tone. The shipped build separates by space and wells, not by lines.
- **Route Candidate** (`route-candidate`): sibling loops on the map, drawn as thin solid lines.
- **Route Casing** (`route-casing`): the white halo under every route line.

### Named Rules
**The Overprint Rule.** Purple is reserved for the active route, things you can press, and the headline score. Data (score bars, elevation profile, terrain, map overlays) never takes it. Bars are ink on stone.

**The Solid Candidate Rule.** Candidate routes are thin, solid, muted lines with a white casing, never dashed, because swisstopo's topo map already draws railways dashed. The active route is a thicker purple line over a wider white casing.

## Typography

**Display Font:** Nunito Sans Variable (falls back to Nunito Sans, then system-ui)
**Body Font:** the same family
**Label/Mono Font:** the same family, with tabular numerals on the body (`font-variant-numeric: tabular-nums`)

**Character:** One rounded humanist sans in many weights. Hierarchy comes from weight (600 to 900) more than from size, so the heavy numbers read as friendly rather than technical.

### Hierarchy
- **Display** (900, 32px, line-height 1): the headline score on the route card. It is the only display-size text.
- **Headline** (900, 23px, -0.015em): the route name ("Loop A").
- **Title** (800 to 900, 17px): query questions, the brand pill and the primary button label.
- **Body** (400 base, 15px, line-height 1.45): base text. Running text is set at 600 to 700 in practice.
- **Label** (800, 14px): chips, segmented options, link buttons and score-bar values. Bar labels and layer rows use 700.
- **Caption** (600 to 800, 12 to 13px): hints, the "Why it scores N" heading, profile axis values, tile meta and the synthetic-data note, all in slate muted.

### Named Rules
**The Sentence Case Rule.** Every label is a plain sentence-case phrase. There are no uppercase tracking labels and no kicker lines above headings.

**The Heavy Numbers Rule.** Scores, distances and bar values are set at 800 to 900 with tabular figures, so they line up and stay legible on a phone held outdoors.

## Layout

The map fills the viewport. On a laptop, a floating left column (410px wide, inset 16px from the left, top and bottom edges) stacks the brand pill, the query card, the selected-route card and the sibling-route tiles, with a 12px gap between them. The column scrolls on its own and lets map clicks pass through the space around the cards. Map tools (layers, then zoom) stack at the top right, inset 16px. Route tiles sit in an auto-fill grid with a minimum width of 118px and an 8px gap.

At 760px and below, the column becomes a **bottom sheet** on the stone ground, with 26px top corners and a drag handle. It snaps between three heights: **peek** (188px, showing the query chips), **half** (52dvh) and **full** (100dvh minus 72px). Height animates over .38s on the shared ease-out curve, and the animation is skipped while the sheet is being dragged. The page edge inset drops to 12px. The brand pill and map tools float at the top and respect the safe-area inset. Map attribution and scale move to the top left so the sheet never covers them. Route tiles become a horizontal scroll-snap strip of 132px tiles.

Spacing follows a small rhythm: an 8px tight step, a 12px gap, a 16px edge, and card padding of 18px by 20px (16px on phones). Touch targets are at least 36px for chips and 40 to 54px for everything else.

## Elevation & Depth

The system is lifted: white cards float over the map on a soft offset-plus-blur shadow, and controls inside the cards are recessed as stone wells instead of having their own shadows. Depth comes from two shadows and the white-on-stone contrast. Borders are not used for depth.

### Shadow Vocabulary
- **Card** (`--shadow-card`, a 1px tight contact shadow plus a 10px-offset, 28px-blur ambient shadow in ink at 35%): every card, tile, brand pill and the first-run hint.
- **Float** (`--shadow-float`, a slightly stronger version of the card shadow): round map buttons, which sit directly on map imagery.
- **Primary glow** (a purple-tinted version of the card shadow): the primary button only.
- **Sheet** (an upward ink shadow): the phone bottom sheet.

### Named Rules
**The Soft Lift Rule.** Shadows are always soft and blurred, with a small downward offset. Cards never use a flat under-shadow or a hard offset block shadow.

**The Well Rule.** Inside a card, interactive surfaces sink rather than lift: stone wells on white, with a purple inset ring when active.

## Shapes

Generously rounded throughout, with radius stepping down as elements nest: card 22px, tile 18px, primary button 16px, field 14px, segmented track 13px, inner segment 10px. Chips, tags, badges and the brand pill are full pills, map buttons are circles, and the phone sheet has 26px top corners. Selection is drawn with an inset 2px purple ring (fields, tiles) or a white-then-purple double ring (layer swatches), never with a separate border colour. Score-bar tracks are 10px tall with 5px rounded ends.

## Components

### Buttons
Tactile and confident, with one clear primary action per card.
- **Shape:** gently rounded (16px), full width, 54px tall.
- **Primary:** course purple with white 17px/900 label, an optional leading icon, and a purple-tinted lift shadow.
- **Hover / Focus:** hover deepens to course purple deep; active scales to .985; disabled or busy shows the deep tone with a spinner. Focus anywhere is a 3px purple outline offset by 2px.
- **Link button:** purple 14px/800 text with an optional 14px icon; hover deepens and underlines (offset 3px). Used for "Change", "Adjust" and "Find loops again".
- **Round map button:** 44px white circle with the float shadow and a 20px icon; hover turns stone.

### Chips
- **Style:** stone pill, 36px tall, 14px/800 ink text, with an optional leading icon. Chips summarise the collapsed query ("Uetliberg, Zürich", "15 km", "Hilly").
- **State:** hover turns stone deep. A purple link button at the end of the row reopens the query.

### Segmented control
- **Style:** a stone track (13px radius, 3px inset) holding 40px options in 14px/800 slate muted.
- **State:** the selected option fills purple with white text and a small purple contact shadow. It can include a numeric input segment that takes the same "on" fill. On a white-on-purple-wash Adjust panel the track turns white.

### Cards / Containers
- **Corner Style:** 22px.
- **Background:** card white.
- **Shadow Strategy:** the card shadow (see Elevation & Depth).
- **Border:** none.
- **Internal Padding:** 18px by 20px (16px on phones).

### Inputs / Fields
- **Style:** a 48px stone well with 14px radius, 700-weight text and a muted leading icon. Fields are buttons that start an action, such as picking a start on the map.
- **Focus / Active:** purple-wash fill, an inset 2px purple ring, deep-purple text and a purple icon.

### Navigation
There is no navigation bar. The brand pill (white, full pill, 17px/900, with the mark) floats at the top left as identity only. The map layers card at the top right is the only other persistent chrome: swatch rows (40 by 30px, 9px radius) with a white-then-purple double ring on the selected base map, and future layers listed in muted text with a "Later" pill. On phones it opens from a round button.

### Route card (signature)
The selected loop explains its own score. The header holds the 23px route name and the 32px purple score. Below it come a muted meta line (distance, climb, descent), an elevation profile (64px, stone area with a 2px ink line, muted min and max captions), and "Why it scores N" bars. Each bar is a full-width button with a label, an ink-on-stone track, a heavy value and an "Adjust" link. Opening one drops in a purple-wash panel (.28s, rising 4px into place) that states the reason and offers a segmented choice. The card ends with a muted note when the data is synthetic.

### Route tiles
Small white tiles (18px radius, card shadow) give each sibling loop's name, score and a muted distance and climb line. The selected tile gets an inset purple ring and a purple score. Hover lifts the tile 1px.

### Map routes
The active loop is drawn as a purple line (4 to 6.5px by zoom) over a white casing (7 to 11px). Candidate loops are thin solid lines in the candidate tone (2.5 to 4px) over a translucent white casing. Paint colours are read from the CSS tokens at runtime.

## Do's and Don'ts

### Do:
- **Do** keep purple to the active route, pressable controls and the headline score (The Overprint Rule).
- **Do** draw score bars, elevation and every other data mark in trail ink on stone.
- **Do** draw candidate routes as thin solid muted lines with a white casing, never dashed.
- **Do** float chrome as white cards with `--shadow-card`, and put controls inside them as stone wells.
- **Do** step radius down as elements nest (22 / 18 / 16 / 14 / 13 / 10px), and use full pills for chips and tags.
- **Do** use slate muted (#625b6b) for secondary text, not lighter tones, so text stays legible outdoors.
- **Do** keep the phone layout as a bottom sheet with peek, half and full snaps at 760px and below, with attribution moved to the top.
- **Do** label synthetic or weak data plainly, in muted caption text or the amber warn badge.
- **Do** use lucide line icons at 14 to 20px, inline with text.

### Don't:
- **Don't** colour terrain, data bars or map overlays purple.
- **Don't** dash route lines. swisstopo draws railways dashed.
- **Don't** use flat under-shadows, hard offset shadows or borders to lift cards.
- **Don't** use uppercase tracking labels or kicker lines. Labels are sentence-case phrases.
- **Don't** open the planner into a drawer of sliders. Settings open inline, next to the reason they affect.
- **Don't** cover the map attribution or the scale with chrome or the sheet.
