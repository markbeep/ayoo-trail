# ayoo — v1 implementation status

The 16-task v1 plan (2026-11-17, revised by D29–D42) is executed. The full plan
text, with its per-task tests, is in the jj history before 2026-11-20; the code
and `docs/decisions.md` are the record of what was built.

- [x] **M1 — Engine core** (tasks 1–5): graph file, snapping, cost model, A*.
- [x] **M2 — Import** (tasks 6–8): Swiss OSM, swissALTI3D DEM, Wanderwege into `graph.bin`.
- [x] **M3 — Generator** (tasks 9–10): seeds, via placement, radius and climb tuning, dedup (D39, D83–D87, D92, D94).
- [x] **M4 — API** (tasks 11–12): `health`, `route`, `generate`. No response cache (D39).
- [x] **M5 — Frontend** (tasks 13–14): map, query, loop cards, GPX, drag editing (D54), avoid areas (D90), share links (D91).
- [x] **M6 — Ship** (tasks 15–16): Docker image and compose stack (D64, D88), `just smoke`, README.

Next work is tracked in `docs/brief.md` (backlog and open questions).
