#!/usr/bin/env bash
# One-command live demo: brings up the stack, seeds, starts a walk as Ana,
# prints (and opens) a one-click deep link to watch the live map, then streams
# the two walkers (Ana + Bek) into that session.
#
#   make demo                 # from backend/
#   ./scripts/demo.sh
#
# Env overrides: API, WEB_URL, WEB_DIR, SEED_PASSWORD, LOOPS.
set -euo pipefail

BACKEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$BACKEND_DIR"

API="${API:-http://localhost:8080}"
WEB_URL="${WEB_URL:-http://localhost:5173}"
WEB_DIR="${WEB_DIR:-$BACKEND_DIR/../../walk4change/web}"
PW="${SEED_PASSWORD:-demodemo}"
LOOPS="${LOOPS:-2}"

jqget() { python3 -c "import sys,json;print(json.load(sys.stdin)$1)"; }

echo "==> 1/5 bringing up backend stack (db + api)"
./scripts/dev-up.sh >/dev/null

echo "==> 2/5 seeding demo data (Ana & Bek)"
./scripts/seed.sh >/dev/null 2>&1 || true

echo "==> 3/5 logging in as Ana and starting a walk"
TOKEN=$(curl -fsS -X POST "$API/api/v1/auth/login" -H 'content-type: application/json' \
  -d "{\"email\":\"ana@demo.walk4change\",\"password\":\"$PW\"}" | jqget "['token']")
[ -n "$TOKEN" ] || { echo "!! login failed"; exit 1; }

SESSION=$(curl -fsS -X POST "$API/api/v1/walks" -H "authorization: Bearer $TOKEN" | jqget "['data']['id']")
[ -n "$SESSION" ] || { echo "!! could not start walk"; exit 1; }

# ── 4/5 ensure the frontend is up (start it in a Node 20 container if needed) ──
echo "==> 4/5 checking frontend at $WEB_URL"
if ! curl -fsS -o /dev/null "$WEB_URL" 2>/dev/null; then
  if [ -d "$WEB_DIR" ]; then
    echo "    frontend down — starting it from $WEB_DIR"
    docker rm -f walk4change-web >/dev/null 2>&1 || true
    docker run -d --name walk4change-web \
      -v "$(cd "$WEB_DIR" && pwd)":/app -v walk4change_web_node_modules:/app/node_modules \
      -w /app -p 5173:5173 node:20-alpine \
      sh -lc "npm install --no-audit --no-fund && npm run dev -- --host 0.0.0.0" >/dev/null
    for _ in $(seq 1 40); do curl -fsS -o /dev/null "$WEB_URL" 2>/dev/null && break; sleep 2; done
  else
    echo "    !! frontend not running and WEB_DIR not found ($WEB_DIR)."
    echo "       Start it: (cd web && ./scripts/dev.sh), then re-open the link below."
  fi
fi

LINK="$WEB_URL/live?token=$TOKEN&watch=$SESSION"

echo
echo "================================================================"
echo " Live demo ready. Open this link (auto-logs-in + auto-watches):"
echo
echo "   $LINK"
echo
echo " session: $SESSION"
echo "================================================================"
echo

# Best-effort: open it in the default browser.
( command -v xdg-open >/dev/null 2>&1 && xdg-open "$LINK" >/dev/null 2>&1 & ) || true

# Give the browser a moment to load + subscribe before the walkers start.
if [ -t 0 ]; then
  read -r -p "Press Enter to start the two walkers (Ana + Bek)… " _
else
  sleep 6
fi

echo "==> 5/5 streaming two walkers into the session (${LOOPS}x)"
for i in $(seq 1 "$LOOPS"); do
  echo "--- lap $i/$LOOPS ---"
  ./scripts/replay.sh "$SESSION"
done

echo
echo "Done. Re-run more laps any time:  make replay SESSION=$SESSION"
