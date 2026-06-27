#!/usr/bin/env bash
# Simulate two people (Ana & Bek) walking TOGETHER in a NATURE zone, live.
#
# Replays two GPS tracks concurrently over the WebSocket API. Both tracks sit
# inside the seeded Brzeźno nature zone (x3) and run side by side (together x1.5),
# so the scoring engine stacks them to x4.5. Prints per-ping points + multipliers.
#
# Requires: ./scripts/dev-up.sh (stack running) and ./scripts/seed.sh (Ana & Bek).
#
# Usage:
#   ./scripts/replay.sh                  # replay starts its own walk
#   ./scripts/replay.sh <session-id>     # stream into an existing walk
#                                        # (e.g. one started in the app's /live screen)
set -euo pipefail

NET=walk4change-net
IMAGE=walk4change-api:dev
PW="${SEED_PASSWORD:-demodemo}"

SESSION_ARG=()
if [[ -n "${1:-}" ]]; then
  SESSION_ARG=(--session "$1")
fi

docker run --rm --network "$NET" "$IMAGE" \
  walk4change-replay \
    --base http://walk4change-api:8080 \
    "${SESSION_ARG[@]}" \
    --email ana@demo.walk4change --password "$PW" --track fixtures/track_a.json \
    --friend-email bek@demo.walk4change --friend-password "$PW" --friend-track fixtures/track_b.json
