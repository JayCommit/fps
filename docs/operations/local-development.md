# Local development

## Prerequisites

- Rust 1.98.0 (see `rust-toolchain.toml`)
- Node.js 22 and pnpm 10
- MariaDB 10.11+ listening on `127.0.0.1:3306`

```sql
CREATE DATABASE fps;
CREATE DATABASE fps_test;
CREATE USER 'fps'@'127.0.0.1' IDENTIFIED BY 'local-dev-only';
GRANT ALL ON fps.* TO 'fps'@'127.0.0.1';
GRANT ALL ON fps_test.* TO 'fps'@'127.0.0.1';
```

Or: `docker compose -f deploy/examples/docker-compose.dev.yml up -d`

## One command

```bash
cp .env.example .env
make dev
```

This starts the control plane on `http://127.0.0.1:47890` and the web UI on
`http://127.0.0.1:47880`.

Open the UI, create the owner (12+ character password), then enroll a node:

```bash
cargo run -p fps-control-plane -- serve
# in another shell after creating a token in the UI:
cargo run -p fps-node-agent -- enroll \
  --url http://127.0.0.1:47890 \
  --token <token> \
  --data-dir ./data/agent \
  --allow-insecure-http
cargo run -p fps-node-agent -- run --data-dir ./data/agent --allow-insecure-http
```

## Tests

```bash
make test
```

Requires MariaDB. Bootstrap tests use an in-process fake Proxmox API and never
touch real hosts.

## Docker on nested containers

A production game node is a full VM. Overlayfs works there.

This Cloud Agent environment is a nested container. Docker Engine can fail
container creates with `overlay: filesystem … not supported as upperdir`. For
local smoke tests only, start dockerd with `--storage-driver=vfs` and make
`/var/run/docker.sock` readable by the agent process. Do not use vfs on a
real Homer node.
