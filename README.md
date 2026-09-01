# FPS

Run game servers on machines you own. FPS is a control panel plus a small agent you install on each host. It is **not** Pterodactyl.

**Alpha `0.0.1-alpha.1`.** Fine for local testing. Do not put it on the public internet. All rights reserved.

## You need

- Docker
- [Rust 1.98](https://rustup.rs/)
- Node 22 and [pnpm 10](https://pnpm.io/)

## Get it running (about 5 minutes)

```bash
git clone https://github.com/JayCommit/fps.git
cd fps
cp .env.example .env
pnpm install

# database
docker compose -f deploy/examples/docker-compose.dev.yml up -d

# terminal 1 — API
make control-plane

# terminal 2 — web UI
make web
```

Open [http://127.0.0.1:47880](http://127.0.0.1:47880). Create the owner account (password at least 12 characters).

## Attach a machine and start a server

In the UI: **Nodes → create an enrollment token**. Then on the machine that has Docker:

```bash
cargo run -p fps-node-agent -- enroll \
  --url http://127.0.0.1:47890 \
  --token PASTE_TOKEN_HERE \
  --data-dir ./data/agent \
  --allow-insecure-http

cargo run -p fps-node-agent -- run \
  --data-dir ./data/agent \
  --allow-insecure-http
```

When the node shows **online** and Docker **available**, go to **Servers**, deploy the **HTTP Echo** template, and wait one heartbeat. You should get a running container.

## Proxmox hosts (Fry / Homer)

On each guest, run the installer and pick a side:

```bash
fps install
# 1 = control plane (web + API)
# 2 = game host (Docker + agent)
```

Or non-interactive: `fps install --role control-plane` / `fps install --role game-host`.

To create the guests first: `docs/operations/proxmox.md`.

## More

- Local quirks (including nested VMs): `docs/operations/local-development.md`
- Security: `SECURITY.md`
- Spec: `docs/product-spec.md`
