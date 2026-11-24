# 0005: Travel time per origin on hover; the client draws the area

Date: 2026-11-24. Source: owner.

## Decision

With the Travel time layer on, pointing at the map shows a tooltip at the cursor
with each travel origin's time to that spot ("1 · 47 min"), or "over budget"
when it is outside that origin's time budget. On touch screens a tap shows the
same tooltip there; with the layer on, a tap no longer opens place info.

Pointing at an origin's row in the list under the Travel time switch shows only
that origin's reach on the map until the pointer leaves the row. Mouse only; not
on touch screens.

The server no longer draws the area. `POST /api/travel` returns, per origin, the
stations it reaches within its budget and the door-to-door seconds to each. The
client draws shared and partial reach from that list and computes the hover
times from it, with no request per pointer move.

The times follow ADR 0002's rules, so a time and the colour under it agree:
best Saturday trip leaving 06:00–10:00, door to door, last walk at most 20 min.

## Why

The PNG was already about 80 kB per update. Sending stations instead is
similar in size and lets the client answer hover queries locally.
