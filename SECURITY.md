# Security policy

Do not file public issues with exploit details until a licence and disclosure
inbox exist. For now, treat reports as private to the repository owners.

Known alpha.1 limitations:

- Web access tokens live in `localStorage` (httpOnly cookies + desktop vault come later).
- TLS is optional on loopback via `FPS_ALLOW_INSECURE_HTTP`.
- Game workloads, backups, and the live console are not in this milestone.

See `docs/security/threat-model.md`.
