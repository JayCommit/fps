# ADR 0004 — Node identity: one-time token + issued client certificate

- Status: accepted
- Date: 2026-09-01

## Decision

Enrollment uses a hashed, expiring, single-use token. The control plane issues
a per-node client certificate from an internal CA stored in the data directory.
The dedicated node port requires mTLS (client certificate fingerprint bound to
the node row). Hashed bearer heartbeats on the public HTTP API are allowed only
when `FPS_ALLOW_INSECURE_HTTP` is set for loopback development.

## Why

Satisfies “mTLS or equivalent mutually authenticated, rotating node identity”
without blocking loopback tests.

## Security implications

Bearer tokens are 256-bit random, stored as SHA-256. Certificates and keys are
0600 on disk. Enrollment tokens are shown once.
