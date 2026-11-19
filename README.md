# trail-route

Generates trail-running loops in Switzerland from a start point, a distance and
a few preferences. Rust (axum) server plus a SolidJS + MapLibre client. See
`docs/brief.md` and `docs/design.md` for the background.

## Local Development

Requirements: [mise](https://mise.jdx.dev) (installs the pinned Rust, Node and
just from `mise.toml`), plus `curl`, `jq` and `unzip` for the data download.

```sh
mise install                 # Rust, Node, just
(cd web && npm ci)           # client dependencies

just data demo               # download OSM, swissALTI3D tiles and Wanderwege into data/raw
just import demo             # build data/graph.bin (demo region: Wattwil–Ziegelbrücke)

just dev                     # API on :8080, client on http://localhost:5173
```

Open http://localhost:5173, pick a start inside the demo region, and press
"Find loops".

- Use `ch` instead of `demo` for all of Switzerland. The download (~42k
  elevation tiles, ~55 GB) and import take much longer. `just data` shows
  progress and an ETA, and an interrupted run resumes where it stopped. The tile
  list is cached in `data/raw/alti3d/.tiles-<region>.txt`; delete it to pick up
  new survey years.
- Without `data/graph.bin` the server still starts, but `/api/route` and
  `/api/generate` answer 503.
- `just check` runs fmt, clippy, the Rust tests and the client typecheck.
- `?dev=route` in the URL turns on the click-to-route debug mode.
- Server settings: `BIND` (default `0.0.0.0:8080`), `DATA_DIR` (default
  `./data`), `STATIC_DIR` (default `web/dist`, used by `just build`).
