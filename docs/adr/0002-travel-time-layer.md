# 0002: Travel time layer

Date: 2026-11-24. Source: owner, planning session for commute times.

## Decision

A map overlay, **Travel time**, colours the places that the owner's travel
origins reach by public transport and walking. It only draws: it has no effect
on the start, the search or routing. It supersedes the deferral in D19.

- Data: GTFS from opentransportdata.swiss. Precomputed into `data/transit.bin`
  and queried on the server. Nothing live or real-time.
- Timetable: the best trip on a Saturday that leaves between 06:00 and 10:00.
  There is no time control.
- The time budget is one-way and door to door: the walk to the stop, the rides,
  the transfers and the walk to the place. There is no initial wait, because you
  leave just in time for the best departure. The last walk is capped at 20 min.
- Drawn as a flat area with two levels. **Shared reach** (strong) is where every
  origin gets within its budget. **Partial reach** (faint) is where some origins
  do but not all, whatever the count.
- Origins: numbered, added with "Travel time from here" in the map menu (always
  shown; it switches the layer on). They are draggable markers while the layer
  is on and are removed from their own map menu. Budgets are edited in a list
  under the layer toggle, default 60 min. Kept in localStorage only, never in
  share links.

Builder's calls:
- The Saturday is the feed's busiest one (the most trips), so it is never a
  holiday. The Swiss feed has exceptions on almost every date, so "a Saturday
  with no exceptions" didn't exist. `--date` overrides it.
- Walking speed is 4.5 km/h in a straight line.
- The walk from an origin to its stops is at most 15 min, and transfer walks
  between stations are at most 400 m. Changing vehicles at one station takes
  2 min.
- Trips that run at a frequency (lifts list 60 s) are expanded to at most one
  departure every 5 min.
- The walk-only reach around an origin is capped at 20 min too.
- Budgets run from 10 to 240 min, with at most 8 origins.
- The area is drawn on 200 m cells.
- `transit.bin` is rebuilt when the yearly timetable changes (mid-December).

## Why

The owner chooses starts by asking "is it within about an hour of home?". With
several people travelling from different places, the useful answer is where
everyone gets in time. A precomputed static timetable keeps the server
self-contained and fast. The Saturday morning window matches when runs start.
