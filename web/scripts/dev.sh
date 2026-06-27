#!/usr/bin/env bash
# Run the SeaSteps frontend (Vite dev server) in a Node 20 container.
# Needed because Vite 8 requires Node >= 20.19 and the host has Node 18.
# The browser runs on the host, so VITE_API_BASE=http://localhost:8080 reaches
# the backend's published port. Ctrl-C to stop.
set -euo pipefail

WEB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$WEB_DIR"

PORT="${PORT:-5173}"

exec docker run --rm -it \
  --name walk4change-web \
  -v "$WEB_DIR":/app \
  -v walk4change_web_node_modules:/app/node_modules \
  -w /app \
  -p "${PORT}:5173" \
  node:20-alpine \
  sh -lc "npm install --no-audit --no-fund && npm run dev -- --host 0.0.0.0"
