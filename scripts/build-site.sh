#!/usr/bin/env bash
# Build the combined SeaSteps site (landing at / + app under /app/) into ./site,
# ready to deploy as ONE Vercel project. The Vite app is built with base=/app/.
#
# Required build env (baked into the app bundle):
#   VITE_API_BASE, VITE_SUPABASE_URL, VITE_SUPABASE_ANON_KEY
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${OUT:-$ROOT/site}"

echo "==> building web app (base=/app/)"
( cd "$ROOT/web" && npm install --no-audit --no-fund && npm run build )

echo "==> assembling combined site at $OUT"
rm -rf "$OUT"; mkdir -p "$OUT/app"
cp "$ROOT/index.html" "$ROOT/privacy.html" "$ROOT/regulamin.html" "$ROOT/favicon.svg" "$ROOT/app-preview.png" "$OUT/"
cp -r "$ROOT/web/dist/." "$OUT/app/"

cat > "$OUT/vercel.json" <<'JSON'
{
  "rewrites": [
    { "source": "/app", "destination": "/app/index.html" },
    { "source": "/app/:path*", "destination": "/app/index.html" }
  ]
}
JSON

echo "==> done. Deploy with:  cd $OUT && vercel deploy --prod"
