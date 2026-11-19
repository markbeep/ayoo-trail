---
name: ayoo
description: A warm, friendly trail-loop planner on a full-bleed outdoor map with relief, in stone and night indigo.
colors:
  stone-ground: "#ece8e1"
  stone-deep: "#e2ddd4"
  card-white: "#ffffff"
  trail-ink: "#26222b"
  slate-muted: "#625b6b"
  hairline: "#e7e2da"
  night-indigo: "#3d45c4"
  night-indigo-deep: "#2f36a0"
  indigo-wash: "#eef0fc"
  amber-warn: "#a4570d"
  amber-wash: "#fbf0e2"
  route-active: "#3d45c4"
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
    backgroundColor: "{colors.night-indigo}"
    textColor: "{colors.card-white}"
    typography: "{typography.title}"
    rounded: "{rounded.button}"
    height: "54px"
  button-primary-hover:
    backgroundColor: "{colors.night-indigo-deep}"
  link-button:
    textColor: "{colors.night-indigo}"
    typography: "{typography.label}"
  link-button-hover:
    textColor: "{colors.night-indigo-deep}"
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
    backgroundColor: "{colors.indigo-wash}"
    textColor: "{colors.night-indigo-deep}"
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
    backgroundColor: "{colors.night-indigo}"
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

# Design System: ayoo

<!-- impeccable:design-map -->
Name: ayoo
Colors: stone-ground #ece8e1, stone-deep #e2ddd4, card-white #ffffff, trail-ink #26222b, slate-muted #625b6b, hairline #e7e2da, night-indigo #3d45c4, night-indigo-deep #2f36a0, indigo-wash #eef0fc, amber-warn #a4570d, amber-wash #fbf0e2 (+3 more in the frontmatter)
Fonts: Nunito Sans Variable (display, headline, title, body, label, caption)
Spacing: tight 8px, gap 12px, edge 16px, edge-phone 12px, card-y 18px, card-x 20px, panel-width 410px
Radius: inner 10px, ctl 13px, field 14px, button 16px, tile 18px, card 22px, sheet 26px, pill 999px
North Star: "The Course Card"
Rules:
- The Overprint Rule. Indigo is reserved for the active route, things you can press, and the headline score.
- The Solid Candidate Rule. Candidate routes are thin, solid, muted lines with a white casing, never dashed, because swisstopo's topo map already draws railways dashed.
- The Sentence Case Rule. Every label is a plain sentence-case phrase.
- The Heavy Numbers Rule. Scores, distances and bar values are set at 800 to 900 with tabular figures, so they line up and stay legible on a phone held outdoors.
- The Soft Lift Rule. Shadows are always soft and blurred, with a small downward offset.
- The Well Rule. Inside a card, interactive surfaces sink rather than lift: stone wells on white, with an indigo inset ring when active.
Elevation:
- Card: --shadow-card
- Float: --shadow-float
- Primary glow (an indigo-tinted version of the card shadow): the primary button only.
- Sheet (an upward ink shadow): the phone bottom sheet.
Don't:
- Don't color terrain, data bars or map overlays indigo.
- Don't dash route lines.
- Don't use flat under-shadows, hard offset shadows or borders to lift cards.
- Don't use uppercase tracking labels or kicker lines.
- Don't open the planner into a drawer of sliders.
- Don't cover the map attribution or the scale with chrome or the sheet.
Do:
- Do keep indigo to the active route, pressable controls and the headline score (The Overprint Rule).
- Do draw score bars, elevation and every other data mark in trail ink on stone.
- Do draw candidate routes as thin solid muted lines with a white casing, never dashed.
- Do float chrome as white cards with `--shadow-card`, and put controls inside them as stone wells.
- (+5 more)
Map: docs/design.md
Full: .impeccable/design-full.md (read a section when you need its detail)
