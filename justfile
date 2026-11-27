# ayoo tasks. Run `just` to list.

default:
    @just --list

# Run the Rust server and the Vite dev server together (Vite proxies /api).
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    trap 'kill 0' EXIT
    cargo run --bin trail-calc-server &
    (cd web && npm run dev -- --host) &
    wait

# Vite dev server only (no API needed for the current mock routes).
web:
    cd web && npm run dev -- --host

# Rust server only.
server:
    cargo run --bin trail-calc-server

# Build the client, then the release server that serves it.
build:
    cd web && npm ci && npm run build
    cargo build --release --bin trail-calc-server

# fmt, clippy, tests, client typecheck.
check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    cd web && npm run typecheck

# End-to-end smoke: health, generate and route on a local server (builds the demo graph if missing).
smoke:
    scripts/smoke.sh

# Build data/graph.bin from data/raw (`just import ch --force` to replace an existing graph; default demo).
import REGION="demo" *ARGS:
    cargo run --release --bin trail-calc-import -- --region {{REGION}} {{ARGS}}

# Build data/view.bin, the 360° view scores, from data/raw/alti3d (D100; ~80 s for ch).
view REGION="demo" *ARGS:
    cargo run --release --bin trail-calc-view -- --region {{REGION}} {{ARGS}}

# Fetches next year's timetable if published, else this year's. After the December
# timetable change: delete data/raw/gtfs, then `just transit --force`.
# Build data/transit.bin for the Travel time layer from the Swiss GTFS feed (ADR 0002; ~10 s).
transit *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    raw=data/raw
    if [ ! -f "$raw/gtfs/stop_times.txt" ]; then
      mkdir -p "$raw/gtfs"
      y=$(date +%Y)
      for year in $((y + 1)) "$y"; do
        if curl -fL --retry 3 -o "$raw/gtfs.zip.part" "https://data.opentransportdata.swiss/en/dataset/timetable-$year-gtfs2020/permalink"; then
          mv "$raw/gtfs.zip.part" "$raw/gtfs.zip"
          break
        fi
      done
      [ -f "$raw/gtfs.zip" ] || { echo "no GTFS feed found for $((y + 1)) or $y" >&2; exit 1; }
      unzip -o -q -d "$raw/gtfs" "$raw/gtfs.zip"
      rm -f "$raw/gtfs.zip"
    fi
    cargo run --release --bin trail-calc-transit -- {{ARGS}}

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
    mkdir -p "$raw/alti3d" "$raw/wanderwege" "$raw/tlm"
    # Download to .part first so an interrupted fetch is not mistaken for a finished one.
    fetch() { [ -f "$2" ] || { curl -fL --retry 3 -o "$2.part" "$1" && mv "$2.part" "$2"; }; }
    fetch https://download.geofabrik.de/europe/switzerland-latest.osm.pbf "$raw/switzerland-latest.osm.pbf"
    # Resolved tile URLs (newest survey year per tile), cached so a rerun skips the slow listing.
    # Delete it to pick up new survey years.
    urls="$raw/alti3d/.tiles-$region.txt"
    if [ -f "$urls" ]; then
      echo "swissALTI3D: using cached tile list $urls"
    else
      url="https://data.geo.admin.ch/api/stac/v0.9/collections/ch.swisstopo.swissalti3d/items?bbox=$bbox&limit=100"
      list=$(mktemp)
      trap 'rm -f "$list"' EXIT
      pages=0
      while [ -n "$url" ]; do
        page=$(curl -fsSL --retry 3 "$url")
        # "<E-N> <year> <href>" per 2 m GeoTIFF asset.
        jq -r '.features[].assets | to_entries[] | select(.key | endswith("_2_2056_5728.tif"))
          | .value.href | . as $h | (split("/") | last | split("_")) as $p | "\($p[2]) \($p[1]) \($h)"' <<<"$page" >>"$list"
        url=$(jq -r 'first(.links[] | select(.rel == "next") | .href) // empty' <<<"$page")
        pages=$((pages + 1))
        # The API reports no total, so show pages and items listed so far.
        printf '\rswissALTI3D: listing page %d, %d items' "$pages" "$(wc -l <"$list")" >&2
      done
      echo >&2
      # Items repeat per survey year; keep the newest year per tile.
      sort -k1,1 -k2,2r "$list" | awk '!seen[$1]++ { print $3 }' >"$urls.part"
      mv "$urls.part" "$urls"
    fi
    total=$(wc -l <"$urls")
    # Names still missing; progress counts how many of these have appeared.
    todo=$(mktemp)
    trap 'rm -f "${list:-}" "$todo"' EXIT
    while read -r u; do f="${u##*/}"; [ -f "$raw/alti3d/$f" ] || echo "$f"; done <"$urls" >"$todo"
    missing=$(wc -l <"$todo")
    echo "swissALTI3D: $((total - missing))/$total tiles present, $missing to download"
    if [ "$missing" -gt 0 ]; then
      grep -F -f "$todo" "$urls" \
        | xargs -r -P 8 -n 1 sh -c 'f="$1/${2##*/}"; [ -f "$f" ] || { curl -fsSL --retry 3 -o "$f.part" "$2" && mv "$f.part" "$f"; }' _ "$raw/alti3d" &
      pid=$!
      t0=$(date +%s)
      report() {
        got=$( (cd "$raw/alti3d" && ls -1) | grep -c -x -F -f "$todo" || true)
        el=$(( $(date +%s) - t0 ))
        eta="--"
        if [ "$got" -gt 0 ] && [ "$el" -gt 0 ]; then
          s=$(( (missing - got) * el / got ))
          eta=$(printf '%dh%02dm' $((s / 3600)) $((s % 3600 / 60)))
        fi
        printf '\rswissALTI3D: %d/%d tiles (%d%%), %s/min, ETA %s   ' \
          "$((total - missing + got))" "$total" "$(( (total - missing + got) * 100 / total ))" \
          "$(( el > 0 ? got * 60 / el : 0 ))" "$eta" >&2
      }
      while kill -0 "$pid" 2>/dev/null; do report; sleep 5; done
      report; echo >&2
      wait "$pid"
    fi
    if ! ls "$raw/wanderwege"/*.gpkg >/dev/null 2>&1; then
      zip="$raw/wanderwege/wanderwege.gpkg.zip"
      fetch https://data.geo.admin.ch/ch.swisstopo.swisstlm3d-wanderwege/swisstlm3d-wanderwege/swisstlm3d-wanderwege_2056_5728.gpkg.zip "$zip"
      unzip -o -d "$raw/wanderwege" "$zip"
      rm -f "$zip"
    fi
    # Only TLM_STRASSE.shp (434 MB of the 3.6 GB zip), by HTTP Range (ADR 0007).
    if [ ! -f "$raw/tlm/TLM_STRASSE.shp" ]; then
      cargo run --release --bin trail-calc-zip-entry -- \
        https://data.geo.admin.ch/ch.swisstopo.swisstlm3d/swisstlm3d_2026-02/swisstlm3d_2026-02_2056_5728.shp.zip \
        'TLM_STRASSEN\swissTLM3D_TLM_STRASSE.shp' "$raw/tlm/TLM_STRASSE.shp"
    fi
