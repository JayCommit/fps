# CHANGELOG

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
- Proxmox host installer (`deploy/proxmox/install.sh`) that can be `curl | bash`'d from GitHub: creates the LXC or VM and fully builds FPS inside it (web UI + API on Fry, Docker + agent on Homer). Role menu works over `/dev/tty` (whiptail on Proxmox).
- Control plane can serve the production web UI (`FPS_WEB_ROOT`, optional `FPS_WEB_BIND` on port 47880).
- Desktop **source** (Tauri 2). Compiling Tauri in this environment is not a gate.
