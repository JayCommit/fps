#!/usr/bin/env bash
# Per-boot runtime services for the FPS development environment.
# Starts the Docker daemon (if needed) and brings up the MariaDB dev database,
# then waits until the database is healthy. The control-plane and web UI are
# launched separately as `terminals` in .cursor/environment.json.
set -euo pipefail

cd "$(dirname "$0")/.."

# --- Docker daemon ---------------------------------------------------------
# There is no init system managing dockerd here, so start it ourselves if it
# is not already accepting connections. Idempotent: skip when already up.
if ! sudo docker info >/dev/null 2>&1; then
  sudo nohup dockerd >/tmp/dockerd.log 2>&1 &
  for _ in $(seq 1 60); do
    if sudo docker info >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
fi

if ! sudo docker info >/dev/null 2>&1; then
  echo "start: Docker daemon failed to start; see /tmp/dockerd.log" >&2
  exit 1
fi

# --- MariaDB dev database --------------------------------------------------
# Compose is idempotent: it reuses the existing container across reboots.
sudo docker compose -f deploy/examples/docker-compose.dev.yml up -d

# Wait for the database to report healthy before returning so the
# control-plane can connect and run migrations on its first attempt.
for _ in $(seq 1 60); do
  status="$(sudo docker inspect --format '{{.State.Health.Status}}' examples-mariadb-1 2>/dev/null || true)"
  if [ "$status" = "healthy" ]; then
    echo "start: MariaDB is healthy"
    exit 0
  fi
  sleep 2
done

echo "start: timed out waiting for MariaDB to become healthy" >&2
exit 1
