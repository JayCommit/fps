# Dependency and licence review (alpha.1)

Product licence: undecided (all rights reserved). Third-party crates retain
their own licences (primarily MIT / Apache-2.0).

Direct Rust runtime crates of note:

| Crate | Role | Licence (typical) | Notes |
|---|---|---|---|
| axum 0.8.9 | HTTP | MIT | Specified |
| sqlx 0.8.6 | MariaDB | MIT/Apache-2.0 | 0.9 requires newer MSRV policy; 0.8 is current compatible stable used here |
| argon2 0.5 | Passwords | MIT/Apache-2.0 | OWASP parameters documented |
| rcgen | Node CA | MIT/Apache-2.0 | Internal CA only |
| bollard | Docker API | Apache-2.0 | Isolated adapter; CLI is not used |
| ed25519-dalek | Manifest signatures | BSD-3 | Test keys generated at runtime |
| reqwest | HTTP client | MIT/Apache-2.0 | rustls only |
| utoipa | OpenAPI | MIT/Apache-2.0 | Canonical contract |

High-privilege / unsafe:

- `bollard` talks to the Docker socket (root-equivalent on the node). Only the
  node agent uses it, never the control plane.
- `FPS_ALLOW_INSECURE_HTTP` disables TLS verification in the agent HTTP
  client for loopback.

Frontend: React 19, Vite 7, TanStack Query, Tailwind 4 — MIT.

CI will run `cargo deny` / audit when the deny configuration is enforced on PRs.
