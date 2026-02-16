#!/bin/sh
set -e

DATA_SERVICE_URL="${DATA_SERVICE_URL:-http://data-service:8080}"
MAX_RETRIES=6
RETRY_INTERVAL=10

# Wait for data-service
echo "[seed] Waiting for data-service..."
retries=0
until curl -sf "${DATA_SERVICE_URL}/headpat" > /dev/null 2>&1; do
  retries=$((retries + 1))
  if [ "$retries" -ge "$MAX_RETRIES" ]; then
    echo "[seed] ERROR: data-service not ready after $((MAX_RETRIES * RETRY_INTERVAL))s"
    exit 1
  fi
  sleep "$RETRY_INTERVAL"
done
echo "[seed] data-service is ready."

# Check if already seeded
existing=$(curl -sf "${DATA_SERVICE_URL}/api/v1/staff" | grep -c '"id"' || true)
if [ "$existing" -gt 0 ]; then
  echo "[seed] Data already exists (${existing} staff found). Skipping."
  exit 0
fi

# Import staff
echo "[seed] Importing staff..."
curl -sf -X POST -H "Content-Type: application/json" \
  -d @/data/staff.json "${DATA_SERVICE_URL}/api/v1/staff/batch" > /dev/null
echo "[seed] Staff imported!"

# Import groups
echo "[seed] Importing groups..."
curl -sf -X POST -H "Content-Type: application/json" \
  -d @/data/groups.json "${DATA_SERVICE_URL}/api/v1/groups/batch" > /dev/null
echo "[seed] Groups imported!"

echo "[seed] NOTE: Memberships require real UUIDs, use Swagger UI to assign staff to groups."
echo "[seed] Done."
