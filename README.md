# FPS

Run game servers on machines you own. FPS is a control panel plus a small agent you install on each host. It is **not** Pterodactyl.

**Alpha `0.0.1-alpha.1`.** Fine for local testing. Do not put it on the public internet. All rights reserved.

## Install on a VM / VPS / dedicated server (Ubuntu or Debian)

Create the machine yourself (Proxmox, AWS, Azure, Hetzner, bare metal, …). Then, as root on **that** Ubuntu 22.04+ or Debian 12+ host, pull the installer. It installs packages, builds FPS, and starts systemd.

The repo is private, so export a token that can read contents and keep it under sudo:

```bash
export FPS_GITHUB_TOKEN=ghp_your_token
bash <(curl -fsSL -H "Authorization: Bearer ${FPS_GITHUB_TOKEN}" \
  https://raw.githubusercontent.com/JayCommit/fps/main/deploy/install.sh)
```

Pick **Control plane** (web UI + API + MariaDB), **Game host** (Docker + agent), or **Both** from the menu. Pass `--yes --role control-plane` for a fully unattended run. `curl | bash` is fine too — prompts use the real terminal. Details: `docs/operations/install.md`.

## Local development (about 5 minutes)

You need Docker, [Rust 1.98](https://rustup.rs/), Node 22, and [pnpm 10](https://pnpm.io/).

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

## More

- Production install: `docs/operations/install.md`
- Local quirks (including nested VMs): `docs/operations/local-development.md`
- Security: `SECURITY.md`
- Spec: `docs/product-spec.md`
