# FPS product specification

This document is the normalized, authoritative product specification for
`0.0.1` development. Requirements must not be weakened in implementation.
Display name: `FPS`. Package / service / filesystem name: `fps`.

Version of this document: `0.0.1-alpha.1`.

## 1. Product vision

A fast, modern, all-in-one platform that lets an administrator deploy and operate
isolated game servers from:

1. A responsive web control panel.
2. A polished Rust-powered desktop application.
3. A documented API and command-line interface.

The platform includes its own control plane and game-node agent. It does **not**
depend on Pterodactyl Panel, Wings, Pelican, AMP, or another game-management
engine at runtime. Pterodactyl Egg import is a compatibility feature only;
imported Eggs are translated into this platform's safe, versioned native template
format.

The product is initially private and invitation-only. Architecture must support
multiple users, roles, multiple game nodes, and eventual public use. Billing and
public self-registration are out of scope for `0.0.1`.

Branding is centralized in `crates/branding`.

## 2. Infrastructure intent

Bring-your-own Linux. The operator creates the VM, VPS, dedicated server, or
cloud instance. FPS does not provision hypervisors.

- **Control plane**: Ubuntu 22.04+ or Debian 12+. Local MariaDB by default.
- **Game host**: Ubuntu 22.04+ or Debian 12+ **full VM** (or dedicated/VPS)
  with Docker Engine. Never LXC for the game runtime.

`deploy/install.sh` takes a fresh OS to a running role (menu + y/n, or
`--yes` unattended). Optional `fps bootstrap apply` can still create empty
Proxmox guests via the HTTP API; it does not install FPS inside them.

## 3. Engineering principles

SMART means deterministic, inspectable automation: resource-aware scheduling with
human-readable explanations; safe default resources; automatic port allocation;
health-aware restarts with crash-loop detection; reconciliation; capacity
forecasts; update compatibility checks; idempotency keys.

DRY: Rust workspace; one OpenAPI contract; shared UI between web and desktop;
one native template schema; permissions defined once; typed configuration.

Speed: async I/O, streaming, pagination, bounded buffers, performance budgets in
`docs/performance.md`.

Visual quality: dark-first operations cockpit, WCAG 2.2 AA, centralized tokens.

## 4. Technology

- Rust workspace, axum + tokio, SQLx + MariaDB/MySQL first (Postgres later behind
  repository boundaries).
- React + TypeScript + Vite shared UI; Tauri 2 desktop (Windows-first).
- Docker Engine API via a Rust client isolated behind a runtime adapter.
- OpenTelemetry-compatible tracing and Prometheus metrics.

Departures require an ADR.

## 5. Repository layout

See the tree in `docs/architecture/overview.md`. Generated files are reproduced by
`make generate`.

## 6–16. Domain, security, features, testing

The full requirement set from the originating specification is in force, including:

- Users, invitations, sessions, TOTP MFA, recovery codes, passkey extension point.
- Roles: owner, administrator, operator, viewer. Backend is the security boundary.
- mTLS (or equivalent rotating node identity) and one-time enrollment tokens.
- Application master key outside the database; secret redaction.
- Native templates (alpha.2) and Egg import (alpha.3).
- Node agent: enroll, heartbeat, Docker capability, desired-state jobs, continue
  serving games during control-plane loss.
- Live console (alpha.4), desktop (alpha.5), bootstrap CLI (alpha.1 plan/preflight).
- GitHub Releases as the update source; SemVer channels alpha/beta/stable;
  signed `update-manifest.json`; never `/releases/latest` for prereleases.
- Testing pyramid with real MariaDB and Docker in integration tests.

## 18. Milestones

See `docs/roadmap.md`. Current implemented milestone: **0.0.1-alpha.1**.

## Assumptions recorded

- Alpha is private and invitation-only.
- Tauri 2 is Windows-first and cross-platform-ready (desktop ships alpha.5).
- MariaDB/MySQL is the first database backend.
- Game node uses a full VM.
- OPNsense is documented/generated, not mutated, in the first release.
- Licence and GitHub visibility remain undecided (`LICENSE` is all-rights-reserved).
- Identifiers are UUIDv7.
- Local development may set `FPS_ALLOW_INSECURE_HTTP=true`. Production must not.
