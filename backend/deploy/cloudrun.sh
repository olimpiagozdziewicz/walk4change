#!/usr/bin/env bash
# Deploy the walk4change API to Google Cloud Run (free tier).
#
# Runs gcloud via the official google/cloud-sdk Docker image (no host install).
# Reads secrets from deploy/.env.deploy (gitignored) — NEVER commit that file.
#
# Required in deploy/.env.deploy:
#   GCP_PROJECT=your-project-id
#   GCP_REGION=europe-central2            # optional, default below
#   GOOGLE_KEY_FILE=/abs/path/to/sa-key.json
#   DATABASE_URL=postgres://...supabase... # Supabase SESSION-mode connection
#   JWT_SECRET=<32+ chars>                 # generated if absent
#   CORS_ALLOWED_ORIGINS=https://your.vercel.app
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKEND_DIR="$(cd "$HERE/.." && pwd)"
ENV_FILE="$HERE/.env.deploy"

[ -f "$ENV_FILE" ] || { echo "!! missing $ENV_FILE (see header)"; exit 1; }
# shellcheck disable=SC1090
set -a; source "$ENV_FILE"; set +a

GCP_REGION="${GCP_REGION:-europe-central2}"
SERVICE="${SERVICE:-walk4change-api}"
: "${GCP_PROJECT:?set GCP_PROJECT}" "${GOOGLE_KEY_FILE:?set GOOGLE_KEY_FILE}" "${DATABASE_URL:?set DATABASE_URL}"
JWT_SECRET="${JWT_SECRET:-$(head -c 48 /dev/urandom | base64 | tr -d '/+=' | head -c 48)}"
CORS_ALLOWED_ORIGINS="${CORS_ALLOWED_ORIGINS:-*}"

SDK=google/cloud-sdk:slim
KEY_DIR="$(cd "$(dirname "$GOOGLE_KEY_FILE")" && pwd)"
KEY_BASE="$(basename "$GOOGLE_KEY_FILE")"

# Write env vars to a YAML file Cloud Run reads verbatim (avoids shell escaping of
# the DB URL / secrets). Kept in a gitignored deploy dir.
ENV_YAML="$HERE/.run-env.yaml"
umask 077
cat > "$ENV_YAML" <<YAML
DATABASE_URL: "${DATABASE_URL}"
JWT_SECRET: "${JWT_SECRET}"
BIND_ADDR: "0.0.0.0:8080"
CORS_ALLOWED_ORIGINS: "${CORS_ALLOWED_ORIGINS}"
YAML

run_gcloud() {
  docker run --rm \
    -v "$BACKEND_DIR":/work -w /work \
    -v "$KEY_DIR":/keys:ro \
    "$SDK" "$@"
}

echo "==> authenticating service account"
run_gcloud gcloud auth activate-service-account --key-file="/keys/$KEY_BASE" --project="$GCP_PROJECT"

echo "==> enabling required APIs (run, cloudbuild, artifactregistry)"
run_gcloud gcloud services enable run.googleapis.com cloudbuild.googleapis.com artifactregistry.googleapis.com --project="$GCP_PROJECT"

echo "==> deploying $SERVICE to Cloud Run ($GCP_REGION) from source"
# --max-instances 1: the live WS hub is in-memory, so keep a single instance.
# --timeout 3600: allow long-lived WebSocket connections (max 60 min).
run_gcloud gcloud run deploy "$SERVICE" \
  --source /work \
  --project="$GCP_PROJECT" \
  --region="$GCP_REGION" \
  --platform=managed \
  --allow-unauthenticated \
  --port=8080 \
  --max-instances=1 \
  --timeout=3600 \
  --env-vars-file=/work/deploy/.run-env.yaml

echo
echo "==> service URL:"
run_gcloud gcloud run services describe "$SERVICE" --project="$GCP_PROJECT" --region="$GCP_REGION" --format='value(status.url)'
echo
echo "Next: set VITE_API_BASE to that URL on Vercel, and re-run with"
echo "CORS_ALLOWED_ORIGINS=<your vercel url> so the browser is allowed."
