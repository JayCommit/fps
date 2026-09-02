# ADR 0007 — Bootstrap never mutates real Proxmox without an explicit flag

- Status: accepted
- Date: 2026-09-01

## Decision

`fps bootstrap apply` requires `--yes`. Real Proxmox mutation additionally
requires `FPS_ALLOW_REAL_PROXMOX=1`. CI and default local use only the
fake Proxmox API. Existing VMIDs are never overwritten.

The host installer `deploy/install.sh` runs on the Ubuntu/Debian machine the
operator already created. It does not talk to Proxmox. `fps bootstrap apply`
is the only remaining Proxmox mutation path and stays gated as above.

## Why

The specification forbids automatically destroying or colliding with real guests.
