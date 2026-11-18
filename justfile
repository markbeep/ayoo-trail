# trail-route tasks. Run `just` to list.

default:
    @just --list

# Run the Rust server and the Vite dev server together (Vite proxies /api).
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    trap 'kill 0' EXIT
    cargo run --bin trail-server &
    (cd web && npm run dev) &
    wait

# Vite dev server only (no API needed for the current mock routes).
web:
    cd web && npm run dev

# Rust server only.
server:
    cargo run --bin trail-server

# Build the client, then the release server that serves it.
build:
    cd web && npm ci && npm run build
    cargo build --release --bin trail-server

# fmt, clippy, tests, client typecheck.
check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    cd web && npm run typecheck

# Build data/graph.bin from data/raw (`just import ch`; default demo).
import REGION="demo":
    cargo run --release --bin trail-import -- --region {{REGION}}

# Fetch OSM, swissALTI3D 2 m tiles and Wanderwege into data/raw (`just data REGION=demo|ch`).
data REGION="demo":
    #!/usr/bin/env bash
    set -euo pipefail
    region="{{REGION}}"; region="${region#REGION=}"
    case "$region" in
      demo) bbox=8.95,47.10,9.22,47.34 ;;
      ch) bbox=5.9,45.8,10.6,47.9 ;;
      *) echo "unknown region '$region' (demo|ch)" >&2; exit 1 ;;
    esac
    raw=data/raw
    mkdir -p "$raw/alti3d" "$raw/wanderwege"
    # Download to .part first so an interrupted fetch is not mistaken for a finished one.
    fetch() { [ -f "$2" ] || { curl -fL --retry 3 -o "$2.part" "$1" && mv "$2.part" "$2"; }; }
    fetch https://download.geofabrik.de/europe/switzerland-latest.osm.pbf "$raw/switzerland-latest.osm.pbf"
    url="https://data.geo.admin.ch/api/stac/v0.9/collections/ch.swisstopo.swissalti3d/items?bbox=$bbox&limit=100"
    list=$(mktemp)
    trap 'rm -f "$list"' EXIT
    while [ -n "$url" ]; do
      page=$(curl -fsSL --retry 3 "$url")
      # "<E-N> <year> <href>" per 2 m GeoTIFF asset.
      jq -r '.features[].assets | to_entries[] | select(.key | endswith("_2_2056_5728.tif"))
        | .value.href | . as $h | (split("/") | last | split("_")) as $p | "\($p[2]) \($p[1]) \($h)"' <<<"$page" >>"$list"
      url=$(jq -r 'first(.links[] | select(.rel == "next") | .href) // empty' <<<"$page")
    done
    # Items repeat per survey year; keep the newest year per tile.
    sort -k1,1 -k2,2r "$list" | awk '!seen[$1]++ { print $3 }' \
      | xargs -r -P 8 -n 1 sh -c 'f="$1/${2##*/}"; [ -f "$f" ] || { curl -fsSL --retry 3 -o "$f.part" "$2" && mv "$f.part" "$f"; }' _ "$raw/alti3d"
    echo "swissALTI3D: $(ls "$raw/alti3d" | grep -c '\.tif$') tiles"
    if ! ls "$raw/wanderwege"/*.gpkg >/dev/null 2>&1; then
      zip="$raw/wanderwege/wanderwege.gpkg.zip"
      fetch https://data.geo.admin.ch/ch.swisstopo.swisstlm3d-wanderwege/swisstlm3d-wanderwege/swisstlm3d-wanderwege_2056_5728.gpkg.zip "$zip"
      unzip -o -d "$raw/wanderwege" "$zip"
      rm -f "$zip"
    fi
