#!/usr/bin/env bash
# Seed demo data (users Ana & Bek, friendship, nature zone, rewards) into the
# local DB by running the `seed` binary inside a throwaway container on the
# stack network. Requires dev-up.sh to have been run first.
set -euo pipefail

NET=walk4change-net
DB_NAME=walk4change-db
IMAGE=walk4change-api:dev

docker run --rm --network "$NET" \
  -e DATABASE_URL="postgres://walk:walk@${DB_NAME}:5432/walk4change" \
  -e JWT_SECRET="${JWT_SECRET:-local-dev-secret-change-me-min-32-characters}" \
  -e SEED_PASSWORD="${SEED_PASSWORD:-demodemo}" \
  "$IMAGE" seed
