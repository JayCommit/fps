#!/usr/bin/env bash
# Idempotent bootstrap for the FPS development environment.
# Runs after the repository is checked out. Prepares durable dependency state:
# system packages (Docker for the dev database), the local .env file, JS
# workspace dependencies, and a warm Rust build of the control-plane.
#
# The Docker daemon and the MariaDB container are NOT started here; per-boot
# runtime services live in ./.cursor/start.sh.
set -euo pipefail

cd "$(dirname "$0")/.."

# --- System packages: Docker engine + Compose plugin -----------------------
# The dev database (MariaDB) runs as a container via the documented compose
# file (deploy/examples/docker-compose.dev.yml).
if ! command -v docker >/dev/null 2>&1; then
  sudo apt-get update -qq
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq docker.io docker-compose-v2
fi

# Inside the nested Cloud Agent VM, Docker's default overlayfs driver cannot
# create image whiteout files ("operation not permitted"). The vfs driver has
# no such restriction and works everywhere.
sudo mkdir -p /etc/docker
if [ ! -f /etc/docker/daemon.json ] || ! grep -q '"storage-driver": *"vfs"' /etc/docker/daemon.json; then
  echo '{"storage-driver":"vfs"}' | sudo tee /etc/docker/daemon.json >/dev/null
fi

# --- Local environment file ------------------------------------------------
# .env is git-ignored; seed it from the checked-in example on first setup.
if [ ! -f .env ]; then
  cp .env.example .env
fi

# --- JavaScript workspace --------------------------------------------------
# The repository does not commit lockfiles, so a plain install is expected.
pnpm install

# --- Rust control-plane ----------------------------------------------------
# Warm the cargo cache and produce the debug binary used by `make control-plane`.
cargo build -p fps-control-plane

echo "install: FPS environment bootstrap complete"
