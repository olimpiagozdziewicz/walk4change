#!/usr/bin/env bash
# Add a nature zone (multiplier x3) centered on a lat/lng, so the "nature" bonus
# triggers wherever you run the two-phone demo. Inserts a square polygon of the
# given radius into Supabase's nature_zones table.
#
# Usage:
#   DATABASE_URL=postgres://... ./deploy/add-nature-zone.sh <lat> <lng> [radius_m] [name]
# Example (Gdańsk, 800 m square):
#   ./deploy/add-nature-zone.sh 54.3520 18.6466 800 "Demo Zone"
set -euo pipefail

LAT="${1:?usage: add-nature-zone.sh <lat> <lng> [radius_m] [name]}"
LNG="${2:?usage: add-nature-zone.sh <lat> <lng> [radius_m] [name]}"
RADIUS_M="${3:-800}"
NAME="${4:-Demo Nature Zone}"
: "${DATABASE_URL:?set DATABASE_URL (Supabase session pooler)}"

# Build a square polygon (WKT is "lng lat") around the center using a simple
# equirectangular approximation: 1 deg lat ~= 111320 m; lng scaled by cos(lat).
read -r WKT < <(python3 - "$LAT" "$LNG" "$RADIUS_M" <<'PY'
import math, sys
lat, lng, r = float(sys.argv[1]), float(sys.argv[2]), float(sys.argv[3])
dlat = r / 111320.0
dlng = r / (111320.0 * max(math.cos(math.radians(lat)), 1e-6))
pts = [
    (lng - dlng, lat - dlat),
    (lng + dlng, lat - dlat),
    (lng + dlng, lat + dlat),
    (lng - dlng, lat + dlat),
    (lng - dlng, lat - dlat),
]
print("SRID=4326;POLYGON((" + ", ".join(f"{x:.6f} {y:.6f}" for x, y in pts) + "))")
PY
)

echo "Inserting nature zone '$NAME' (~${RADIUS_M}m) at $LAT,$LNG"
SQL="INSERT INTO nature_zones (id, name, geom, multiplier, active)
     VALUES (gen_random_uuid(), '${NAME//\'/\'\'}', ST_GeogFromText('${WKT}'), 3.0, true);"

docker run --rm -e PGCONNECT_TIMEOUT=15 postgres:16-alpine \
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -c "$SQL"

echo "Done. Active zones now:"
docker run --rm postgres:16-alpine psql "$DATABASE_URL" -tAc \
  "SELECT name FROM nature_zones WHERE active = true;"
