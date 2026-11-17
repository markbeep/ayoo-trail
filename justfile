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
