# Authentication (alpha.1)

## Argon2id parameters

Default (Fry-class 32 GiB control plane, shared with other VMs):

- Variant: Argon2id
- Memory: 19 MiB (`19456` KiB)
- Iterations: 2
- Parallelism: 1

This matches the OWASP Password Storage Cheat Sheet minimum that trades RAM for
CPU on a 4c/8t Xeon E3-1270 v3. Interactive login should stay well under 500 ms
on that host; record measurements in `docs/performance.md` when the box is
available.

Tests may use 8192 KiB / 1 iteration via `FPS_ARGON2_*`.

Passwords must be at least 12 characters.

## Sessions

Opaque 256-bit tokens, SHA-256 at rest, 12 hour access TTL, 14 day refresh with
rotation. The web UI and CLI authenticate with Bearer tokens. Cookie session
authentication is not accepted in alpha.1 (no CSRF-bound cookie path yet).

`X-Forwarded-For` is ignored unless `FPS_TRUST_FORWARDED_HEADERS=true`.

## MFA

Optional TOTP (SHA1, 6 digits, 30s, ±1 window) with encrypted secret (AES-256-GCM using the
application master key). Enrollment stores a pending secret until confirm succeeds, so
starting TOTP cannot disable an already-enabled factor. Hashed recovery codes are
accepted in place of the current TOTP code at login and are consumed on use.
