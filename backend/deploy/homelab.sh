#!/usr/bin/env bash
# Run the walk4change API in your homelab (Docker) against Supabase, exposed to
# the internet via a Cloudflare Tunnel (free, HTTPS + WebSocket, no port-forward).
#
# Architecture:
#   Vercel frontend (browser) -> Cloudflare HTTPS -> tunnel -> THIS backend -> Supabase
#
# Secrets come from deploy/.env.homelab (gitignored). NEVER commit it.
# Required:
#   DATABASE_URL=postgresql://...pooler.supabase.com:5432/postgres   # Supabase SESSION pooler
#   JWT_SECRET=<32+ chars>
#   CORS_ALLOWED_ORIGINS=https://<your-app>.vercel.app
# Optional (for a STABLE public hostname — strongly recommended):
#   CLOUDFLARED_TUNNEL_TOKEN=<token from Cloudflare Zero Trust dashboard>
#     -> named tunnel; you map a hostname (e.g. api.yourdomain.com) in the dashboard.
#   If unset, a QUICK tunnel is used: instant but the URL is random and changes
#   on every restart (you'd have to rebuild the Vercel frontend each time).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="$HERE/.env.homelab"
IMAGE="${IMAGE:-walk4change-api:dev}"
PORT="${PORT:-8080}"

[ -f "$ENV_FILE" ] || { echo "!! missing $ENV_FILE (see header)"; exit 1; }
# shellcheck disable=SC1090
set -a; source "$ENV_FILE"; set +a
: "${DATABASE_URL:?set DATABASE_URL}" "${JWT_SECRET:?set JWT_SECRET}"
CORS_ALLOWED_ORIGINS="${CORS_ALLOWED_ORIGINS:-*}"

echo "==> (re)starting API container against Supabase"
docker rm -f walk4change-prod >/dev/null 2>&1 || true
docker run -d --name walk4change-prod --restart unless-stopped \
  -e DATABASE_URL="$DATABASE_URL" \
  -e JWT_SECRET="$JWT_SECRET" \
  -e BIND_ADDR="0.0.0.0:8080" \
  -e CORS_ALLOWED_ORIGINS="$CORS_ALLOWED_ORIGINS" \
  -e APP_URL="${APP_URL:-}" \
  -e SMTP_HOST="${SMTP_HOST:-}" \
  -e SMTP_PORT="${SMTP_PORT:-587}" \
  -e SMTP_USER="${SMTP_USER:-}" \
  -e SMTP_PASS="${SMTP_PASS:-}" \
  -e SMTP_FROM="${SMTP_FROM:-}" \
  -p "$PORT:8080" \
  "$IMAGE" >/dev/null

echo "==> waiting for health"
for _ in $(seq 1 30); do
  curl -fsS "http://localhost:$PORT/api/v1/health" >/dev/null 2>&1 && { echo "    healthy"; break; }
  sleep 1
done

echo "==> starting Cloudflare Tunnel"
docker rm -f walk4change-tunnel >/dev/null 2>&1 || true
if [ -n "${CLOUDFLARED_TUNNEL_TOKEN:-}" ]; then
  echo "    named tunnel (stable hostname from your Cloudflare dashboard)"
  docker run -d --name walk4change-tunnel --restart unless-stopped --network host \
    cloudflare/cloudflared:latest tunnel --no-autoupdate run --token "$CLOUDFLARED_TUNNEL_TOKEN" >/dev/null
  echo "    -> public URL = the hostname you mapped in Zero Trust > Tunnels"
else
  echo "    QUICK tunnel (ephemeral URL — changes on restart)"
  docker run -d --name walk4change-tunnel --network host \
    cloudflare/cloudflared:latest tunnel --no-autoupdate --url "http://localhost:$PORT" >/dev/null
  for _ in $(seq 1 30); do
    URL=$(docker logs walk4change-tunnel 2>&1 | grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' | head -1)
    [ -n "${URL:-}" ] && break; sleep 1
  done
  echo "    -> public URL: ${URL:-<see: docker logs walk4change-tunnel>}"
fi

echo
echo "Set VITE_API_BASE on Vercel to the public URL above, then redeploy the frontend."
echo "Seed demo data (Ana & Bek) into Supabase once:  docker run --rm -e DATABASE_URL=… -e JWT_SECRET=… $IMAGE seed"
