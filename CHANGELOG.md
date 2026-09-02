# CHANGELOG

## Unreleased

### Added

- Schema version **5**: host CPU %, used memory, uptime, remote node settings, Docker prune, and panel uninstall.
- After enrollment, the panel manages the host: live CPU / memory / disk bars, heartbeat interval, maintenance, Docker prune, and uninstall (agent stops FPS containers, wipes identity, disables `fps-node-agent`).
- Heartbeats return `settings` and accept `control_ack` so agents apply panel changes without a new protocol version.
- Installer detects an existing panel and can **reconfigure** public IP, CORS, HTTP, and a remote MariaDB URL without rebuilding (`--reconfigure`, `--database-url`).
- Seeded templates for FiveM (txAdmin), CS2, Rust, Valheim, Palworld, Factorio, Terraria, GMod, TeamSpeak, Satisfactory, Paper, and Bedrock, with game icons in the panel.
- Dedicated **Deploy** and **Create template** pages; catalogue, servers, nodes, and dashboard use card layouts and an environment key/value editor.

### Changed

- Re-running the installer updates `FPS_PUBLIC_URL` / `FPS_CORS_ORIGINS` on an existing `/etc/fps/control-plane.env` instead of ignoring `--public-host`.
- Native templates expose `game` and `environment` on `TemplateSummary` so the UI can iconify and prefill deploy forms.

### Fixed

- Installer prompt “Allow unencrypted HTTP?” now writes `FPS_ALLOW_INSECURE_HTTP` to `/etc/fps/node-agent.env`. The agent systemd unit never passed `--allow-insecure-http`, so enrolled nodes refused HTTP heartbeats.
- Agent `run` honors a stored `http://` node endpoint from enroll, so existing game hosts recover after upgrade without re-enrolling.
- Insecure HTTP enroll advertises the public hostname instead of `http://0.0.0.0:47890`.
- Host installer builds the `fps-bootstrap` crate (`cargo build -p fps-bootstrap`). Passing `-p fps` failed with `package ID specification 'fps' did not match any packages` after a successful clone.
- Ubuntu 26.04 (resolute) and other post-24.04 Ubuntu/Debian testing releases install Docker Engine from the noble/bookworm apt pockets.
- Query `access_token` authenticates WebSocket upgrades only. Ordinary HTTP routes stay Bearer-only.
- Crash-loop restart no longer treats `installing` servers as crashed, so restore/install is not marked failed while the container is still down.

### Added

- Live WebSocket console, resource graphs, backup restore, file read/write, exec, settings, TOTP enroll UI, and `fps login` / `fps status` / `fps check-update`.
- Schema version **4**: `resource_samples` plus server crash counters.
- Desktop companion `api_fetch` so the Tauri shell can talk to a remote control plane.

### Changed

- Public GitHub install: `curl` the installer with no `FPS_GITHUB_TOKEN`.
- Fresh-machine installer is Ubuntu/Debian (`deploy/install.sh`): menu + progress, unattended `--yes`, builds from a VM/VPS/dedicated server the operator already created. The Proxmox guest-creator (`pct`/`qm`) is gone; `deploy/proxmox/install.sh` is a pointer to the new script.

## 0.0.1-alpha.1 — 2026-09-01

### Added

- Product identity is **FPS** (package `fps`, env prefix `FPS_`).
- Control plane with owner setup, Argon2id auth, optional TOTP, OpenAPI, node enrollment, and heartbeats.
- Node agent enrollment, Docker capability reporting, and heartbeat loop.
- Web control panel (setup, login, dashboard, nodes).
- Bootstrap CLI with validated config, plan, preflight, apply (fake Proxmox / explicit real-host opt-in), and tests.
- Signed `update-manifest.json` generation (`fps release-manifest`) and Ed25519 verification tests.
- Node mTLS listener; bearer heartbeats are local-dev opt-in only.
- Invitation-only identity: user list, role/status patch, invite accept, audit list, node revoke.
- `/docs` and `/metrics` require `diagnostics.read`.
- Native templates, Egg import, catalogue seed, scheduler, allocations, and install/start/stop/backup/files jobs on heartbeat.
- Agent Docker job runtime (bollard) with stored log chunks and in-app notifications.
- Web pages for servers, templates, users, audit, backups, notifications, and invite accept.
- Bootstrap install artifacts (`fps install-artifacts`) and updater listing that never uses GitHub `/releases/latest`.
- Host installer (`fps install` / `deploy/install/install.sh`) that picks control plane, game host, or both.
- Control plane can serve the production web UI (`FPS_WEB_ROOT`, optional `FPS_WEB_BIND` on port 47880).
- Desktop **source** (Tauri 2). Compiling Tauri in this environment is not a gate.
