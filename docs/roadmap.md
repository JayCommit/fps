# Roadmap and release gates

Development begins at `0.0.1-alpha.1`. Do not mark a milestone complete unless its
acceptance tests pass and documentation matches reality. This tree is **not**
`0.0.1` stable.

## 0.0.1-alpha.1 — foundation

- [x] Monorepo, CI, architecture, ADRs, versioning
- [x] Control plane: setup, authentication, migrations, health/version, OpenAPI, node model
- [x] Agent: enrollment, certificate identity, heartbeat, Docker capability reporting
- [x] Web shell: login, dashboard, nodes, error/empty/loading states
- [x] Bootstrap CLI: config, preflight, plan, fake-Proxmox tests
- [x] Signed-manifest unit tests and `fps release-manifest`
- [x] Vertical-slice test: setup → enroll → heartbeat
- [x] Node mTLS listener; HTTP bearer heartbeats only when `ALLOW_INSECURE_HTTP` is set
- [x] Bootstrap apply against fake Proxmox; real hosts require `--yes` and `FPS_ALLOW_REAL_PROXMOX=1`

## Shipped beyond the original alpha.1 gate (this revision)

These items have automated tests and a web UI. They are **not** a stable 0.0.1
release.

- [x] Users, invitations (accept + password), role/status patch, audit list
- [x] Node revoke (`POST /v1/nodes/{id}/revoke`)
- [x] `/docs` and `/metrics` require `diagnostics.read`
- [x] Native templates, catalogue seed, Egg import API
- [x] Scheduler + allocations + install/start/stop/backup/files jobs on heartbeat
- [x] Agent Docker job runtime (bollard)
- [x] Server logs (stored chunks), in-app notifications, interval schedules
- [x] Web pages for servers, templates, users, audit, backups, notifications, invite accept
- [x] Bootstrap install artifacts (`fps install-artifacts`, systemd units, `deploy/install/install.sh`)
- [x] Role picker: `fps install` / `install.sh --role` for control plane vs game host vs both
- [x] `fps bootstrap apply --role` creates only the chosen Proxmox guest(s)
- [x] Ubuntu/Debian `curl | bash` installer takes a fresh VM/VPS to a running control plane or game host
- [x] Updater GitHub listing never uses `/releases/latest`
- [x] Desktop **source** (Tauri 2, file/keyring vault). Compiling Tauri in this VM is not a gate.

## Still open

- Signed desktop installers and verified client updates
- Load, upgrade/rollback, and backup-restore demonstration gates for 0.0.1
- Product identity placeholders (`FPS`, GitHub owner/repo)
- httpOnly cookie sessions (Bearer + localStorage remain the alpha path)

## Shipped in this revision (ops)

- [x] Installer builds `-p fps-bootstrap` (the CLI crate; binary name is `fps`)
- [x] Ubuntu 26.04 / Debian testing Docker apt pockets fall back to noble/bookworm
- [x] Live WebSocket console (`/v1/servers/{id}/console`) with HTTP log fallback
- [x] Resource sample graphs from heartbeats
- [x] Backup restore API/UI (`POST /v1/backups/{id}/restore`)
- [x] File read/write and exec jobs
- [x] Settings UI, TOTP enroll UI, in-app update check (`/v1/updates/check`)
- [x] Crash-loop restart (max 3) and schedulable-node pick (online Docker, fresh heartbeat)
- [x] Desktop companion: control-plane URL, `api_fetch`, vault, `fps login` / `fps status`
- [x] Server addons: curated install/uninstall from the panel (CS2 MetaMod/CSS, Rust Oxide, Paper plugins, FiveM resources, GMod ULX)

## 0.0.1-beta.1 — hardening

Feature freeze, threat-model review, load tests, upgrade/rollback tests.

## 0.0.1 — first stable

One-command Ubuntu/Debian deploy, verified updates, backup restoration demonstration.
