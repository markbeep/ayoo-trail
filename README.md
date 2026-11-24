# ayoo

> [!NOTE]
> This project is very much in its prototyping stage: it's a place to test ideas.
> Expect features, data formats and behaviour to change without notice.

Generates trail-running loops in Switzerland based on multiple adjustable metrics.
Give it a start, a distance and what you care about; it returns a few distinct loops,
scored against your settings, that you can drag-edit, share as a link and download as GPX.

![UI screenshot](assets/screenshot.webp)

## What it routes by

Each metric is a cost term with an importance slider (Off … Normal … Above all). The
generator places turning points where your weighted costs are low and routes between them.

**Built**

- **Distance**: loops within ±10 % of the target.
- **Elevation**: a climb target (Gentle, Hilly, Mountain or meters) and how closely to match it, or as much as possible.
- **Gradient**: whether you like steep or gentle uphills and downhills.
- **Surface**: avoid asphalt and paved paths.
- **Technical terrain**: the hardest trail grade allowed (T1–T6), and whether to avoid or seek harder ones.
- **Marked trails**: prefer official Swiss hiking trails.
- **Forest**: prefer running through forest.
- **Away from towns**: avoid villages and built-up areas.
- **Quiet roads**: avoid busy road classes.
- **Avoid/prefer areas**: circles you place on the map (long-press or right-click) and resize.
- **Views**: how far you see from each spot; off by default, and turning it up pulls loops toward
  ridges and peaks. Needs `data/view.bin` (`just view`).

**Planned**

- Popular trails.

## Map layers

Base maps: Outdoor, Swiss topo, Satellite. Overlays: relief shading, marked trails, forest,
360° view, SchweizMobil hiking routes, webcams and **Travel time**.

Travel time is only a map layer, not a cost term: it colours where one or more travel origins
get by public transport within their own time budgets (Saturday morning). It is dark where
everyone gets there and light where only some do. Add an origin from the map menu
(right-click or long-press: *Travel time from here*). Point at the map (tap on a phone) to see
each origin's time to that spot.

## Run it

Needs [mise](https://mise.jdx.dev) (pinned Rust, Node, just), plus `curl`, `jq` and `unzip`.

```sh
mise install && (cd web && npm ci)
just data demo      # OSM, swissALTI3D and Wanderwege into data/raw (`ch` for all of Switzerland, ~55 GB)
just import demo    # build data/graph.bin
just dev            # API on :8080, client on http://localhost:5173
```

Other commands:

- `just check` runs fmt, clippy, the tests and the typecheck.
- `just smoke` runs an end-to-end health, generate and route check.
- `just view demo` builds `data/view.bin` for the Views term and the 360° view overlay (~80 s for `ch`).
- `just transit` fetches the Swiss GTFS feed and builds `data/transit.bin` for the Travel time layer.
- `docker compose up` runs the import and the server as one image.

### Keys and settings

`just dev` needs no keys. Everything is optional and read from the environment (there is no
`.env` loading, so export it in the shell that runs `just dev`):

| Variable | Default | What it does |
|---|---|---|
| `WINDY_API_KEY` | unset | Turns on the Webcams overlay ([Windy Webcams API](https://api.windy.com/webcams), free tier). Without it the overlay stays empty. |
| `DATA_DIR` | `data` | Where `graph.bin`, `view.bin` and `transit.bin` live. |
| `BIND` | `0.0.0.0:8080` | Server address. |
| `STATIC_DIR` | `web/dist` | Built client the server serves. |
| `RUST_LOG` | `info` | Log filter. |

Optional data files, each skipped with a warning when missing: `view.bin` (Views, 360° view) and
`transit.bin` (Travel time). The GTFS feed for `just transit` downloads without a key.

Data: © swisstopo, © OpenStreetMap contributors, timetable from opentransportdata.swiss.
