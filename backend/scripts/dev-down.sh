#!/usr/bin/env bash
# Tear down the local walk4change stack started by dev-up.sh.
# Pass --purge to also delete the Postgres data volume.
set -euo pipefail

API_NAME=walk4change-api
DB_NAME=walk4change-db
NET=walk4change-net

docker rm -f "$API_NAME" >/dev/null 2>&1 || true

if [[ "${1:-}" == "--purge" ]]; then
  docker rm -f "$DB_NAME" >/dev/null 2>&1 || true
  docker volume rm walk4change_pgdata >/dev/null 2>&1 || true
  docker network rm "$NET" >/dev/null 2>&1 || true
  echo "stack removed (data volume purged)"
else
  docker stop "$DB_NAME" >/dev/null 2>&1 || true
  echo "stack stopped (db data kept; run with --purge to wipe)"
fi
