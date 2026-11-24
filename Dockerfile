# syntax=docker/dockerfile:1
# ayoo: one small image with both binaries and the built client (D11, D17).
# The graph is not baked in: mount it at /data (graph.bin, optional view.bin and transit.bin), so it can be rebuilt independently.

# glibc, matching the lockfile: the Solid compiler ships native binaries per libc.
FROM node:26.13.1-slim AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.100.0-alpine AS rust
# musl-dev: the bundled SQLite (rusqlite, importer only) needs a C toolchain.
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked --bins \
    && cp target/release/trail-calc-server target/release/trail-calc-import target/release/trail-calc-view /

# Static musl binaries, no shell, runs as uid 65532.
FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=rust /trail-calc-server /trail-calc-import /trail-calc-view /app/
COPY --from=web /web/dist /app/web
ENV STATIC_DIR=/app/web DATA_DIR=/data BIND=0.0.0.0:8080 RUST_LOG=info
EXPOSE 8080
VOLUME ["/data"]
ENTRYPOINT ["/app/trail-calc-server"]
