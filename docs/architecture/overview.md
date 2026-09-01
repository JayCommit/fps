# Architecture overview

```text
Administrator ── web (Vite/React :47880) ──┐
Desktop (alpha.5) ─────────────────────────┤
CLI `fps` ────────────────────────┤
                                           ▼
                              control-plane (:47890 HTTP API, :47891 mTLS)
                                           │
                    MariaDB/MySQL ◄────────┤
                                           │
                              node mTLS heartbeat (bearer HTTP is loopback opt-in)
                                           │
                                           ▼
                              node-agent on game VM (Docker Engine)
```

## Workspace

- `crates/*` — shared domain, protocol, auth, updater, config, branding
- `services/control-plane` — axum API, SQLx migrations
- `services/node-agent` — enroll / heartbeat / Docker probe
- `services/bootstrap` — `fps bootstrap …`
- `apps/web` — control panel
- `packages/*` — tokens, generated OpenAPI client
- `deploy/` — examples, systemd, Proxmox notes

## Node identity (alpha.1)

1. Operator creates a one-time enrollment token (hashed at rest, 15 minute TTL).
2. Agent `enroll` exchanges the token for `node_id`, rotating bearer token, and
   an mTLS client certificate issued by the control-plane CA.
3. Production heartbeats present the client certificate on the node mTLS port.
   Loopback may use `Authorization: Bearer` on the public API when
   `FPS_ALLOW_INSECURE_HTTP` is set.
4. Tokens cannot be replayed. Incompatible protocol versions are rejected.

## Local insecure HTTP

`FPS_ALLOW_INSECURE_HTTP=true` is for loopback development only.
Certificate material is still issued so the production mTLS path stays real.
