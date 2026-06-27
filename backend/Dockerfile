# syntax=docker/dockerfile:1

# ── Build stage ──────────────────────────────────────────────────────────────
# Compiles the API and the `seed` binary in release mode. Migrations are
# embedded at compile time via sqlx::migrate!(), so no DB is needed to build.
FROM rust:1-bookworm AS builder
WORKDIR /build

# Copy the whole backend crate (target/ excluded via .dockerignore).
COPY . .

# Build the api package (`walk4change-api` + `seed`) and the replay demo tool
# (`walk4change-replay`, used to simulate two people walking together).
RUN cargo build --release -p walk4change-api -p walk4change-replay

# ── Runtime stage ────────────────────────────────────────────────────────────
# Slim Debian + ca-certificates (rustls verifies TLS against system roots when
# talking to a remote Postgres such as Supabase in prod). curl is used by the
# container healthcheck.
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/walk4change-api     /usr/local/bin/walk4change-api
COPY --from=builder /build/target/release/seed                /usr/local/bin/seed
COPY --from=builder /build/target/release/walk4change-replay  /usr/local/bin/walk4change-replay
# Demo GPS tracks for the replay tool (resolved relative to WORKDIR /app).
COPY --from=builder /build/fixtures                           /app/fixtures

ENV BIND_ADDR=0.0.0.0:8080
EXPOSE 8080

HEALTHCHECK --interval=5s --timeout=3s --start-period=20s --retries=10 \
    CMD curl -fsS http://localhost:8080/api/v1/health || exit 1

CMD ["walk4change-api"]
