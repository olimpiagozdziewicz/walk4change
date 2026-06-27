#!/usr/bin/env bash
# Bring up the local walk4change stack (Postgres + API) using plain Docker.
# Use this when the `docker compose` plugin is not installed. It is idempotent:
# safe to run repeatedly. Supabase stays prod-only — this only touches local.
set -euo pipefail

BACKEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$BACKEND_DIR"

NET=walk4change-net
DB_NAME=walk4change-db
API_NAME=walk4change-api
IMAGE=walk4change-api:dev

# Internal connection: API talks to the DB container over the user network on 5432.
DATABASE_URL="postgres://walk:walk@${DB_NAME}:5432/walk4change"
JWT_SECRET="${JWT_SECRET:-local-dev-secret-change-me-min-32-characters}"
CORS_ALLOWED_ORIGINS="${CORS_ALLOWED_ORIGINS:-http://localhost:5173,http://localhost:3000}"

echo "==> ensuring docker network '$NET'"
docker network inspect "$NET" >/dev/null 2>&1 || docker network create "$NET" >/dev/null

echo "==> ensuring Postgres container '$DB_NAME'"
if docker ps -a --format '{{.Names}}' | grep -qx "$DB_NAME"; then
  docker start "$DB_NAME" >/dev/null 2>&1 || true
else
  docker run -d --name "$DB_NAME" --network "$NET" \
    -e POSTGRES_USER=walk -e POSTGRES_PASSWORD=walk -e POSTGRES_DB=walk4change \
    -p 5433:5432 \
    -v walk4change_pgdata:/var/lib/postgresql/data \
    postgis/postgis:16-3.4 >/dev/null
fi
# Attach to the network if it was created outside of it (ignore if already attached).
docker network connect "$NET" "$DB_NAME" >/dev/null 2>&1 || true

echo "==> waiting for Postgres to accept connections"
for _ in $(seq 1 30); do
  if docker exec "$DB_NAME" pg_isready -U walk -d walk4change >/dev/null 2>&1; then
    echo "    db ready"
    break
  fi
  sleep 1
done

if [[ -n "${FORCE_REBUILD:-}" ]] || ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
  echo "==> building API image (first build compiles Rust deps — a few minutes)"
  docker build -t "$IMAGE" .
else
  echo "==> API image '$IMAGE' already built — skipping (FORCE_REBUILD=1 to rebuild)"
fi

echo "==> (re)starting API container '$API_NAME'"
docker rm -f "$API_NAME" >/dev/null 2>&1 || true
docker run -d --name "$API_NAME" --network "$NET" \
  -e DATABASE_URL="$DATABASE_URL" \
  -e JWT_SECRET="$JWT_SECRET" \
  -e BIND_ADDR=0.0.0.0:8080 \
  -e CORS_ALLOWED_ORIGINS="$CORS_ALLOWED_ORIGINS" \
  -p 8080:8080 \
  "$IMAGE" >/dev/null

echo "==> waiting for API health"
for _ in $(seq 1 30); do
  if curl -fsS http://localhost:8080/api/v1/health >/dev/null 2>&1; then
    echo "    API healthy at http://localhost:8080/api/v1"
    exit 0
  fi
  sleep 1
done

echo "!! API did not become healthy. Logs:" >&2
docker logs --tail 50 "$API_NAME" >&2
exit 1
