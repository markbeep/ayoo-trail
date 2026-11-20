#!/usr/bin/env bash
# End-to-end smoke: health, generate and route against a real graph (`just smoke`).
set -euo pipefail
cd "$(dirname "$0")/.."

if [ ! -f data/graph.bin ]; then
  just import demo
fi

cargo build --release --bin trail-calc-server

LOG="$(mktemp)"
SERVER_PID=""
cleanup() {
  if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  rm -f "$LOG"
}
trap cleanup EXIT

BIND=127.0.0.1:18080 DATA_DIR="$PWD/data" ./target/release/trail-calc-server >"$LOG" 2>&1 &
SERVER_PID=$!

up=0
for _ in $(seq 1 120); do
  if curl -sf http://127.0.0.1:18080/api/health >/dev/null 2>&1; then
    up=1
    break
  fi
  sleep 0.5
done
if [ "$up" -ne 1 ]; then
  echo "server did not become healthy" >&2
  cat "$LOG" >&2
  exit 1
fi
echo "health ok"

post() { curl -sf -X POST -H 'content-type: application/json' -d "$2" "http://127.0.0.1:18080$1"; }

GEN_REQ='{"start":{"lat":47.2985,"lon":9.095},"distance_m":15000,"profile":{},"count":3}'
GEN_RES="$(post /api/generate "$GEN_REQ")" || { echo "generate failed" >&2; exit 1; }
if ! printf '%s' "$GEN_RES" | jq -e '.routes | length >= 1' >/dev/null; then
  echo "$GEN_RES" >&2
  exit 1
fi
echo "generate ok"

WPS="$(printf '%s' "$GEN_RES" | jq -c '.routes[0].waypoints')"
ROUTE_REQ="$(jq -cn --argjson wps "$WPS" '{waypoints: $wps, profile: {}}')"
ROUTE_RES="$(post /api/route "$ROUTE_REQ")" || { echo "route failed" >&2; exit 1; }
if ! printf '%s' "$ROUTE_RES" | jq -e '.geometry.coordinates | length > 1' >/dev/null; then
  echo "$ROUTE_RES" >&2
  exit 1
fi
echo "route ok"
