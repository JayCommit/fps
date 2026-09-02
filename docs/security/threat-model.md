# Threat model skeleton (alpha.1)

This is a living document. Beta requires a full review. No critical/high issues
may remain at `0.0.1-beta.1`.

| Scenario | Impact | Alpha.1 mitigations | Residual risk |
|---|---|---|---|
| Control plane compromise | Full tenant control | Argon2id, session hashing, master key outside DB, audit events, CSP, CSRF token issued | Process RCE still fatal; harden host |
| Node compromise | Game workloads on that node | One-time enroll, hashed node token, certs 0600, no host install scripts | Stolen node token can heartbeat/spoof that node |
| Malicious template | Host escape | Not in alpha.1; later: reject privileged mounts, run installers in ephemeral containers | — |
| Compromised game container | Lateral movement | Dropped privileged mounts; jobs talk to Docker Engine via bollard; no host install scripts | Docker socket is still root-equivalent on the node; isolate game hosts |
| Stolen desktop token | Account takeover | Desktop vault + optional control-plane URL; web tokens still in localStorage (alpha limitation) | Move to httpOnly cookies in a later alpha |
| Supply-chain attack | Malicious binary | Pinned GH Actions SHAs, checksums, signed manifests, Cargo.lock | Signing keys must stay in protected environments |
| Split connectivity | Split-brain | Agent keeps running locally; heartbeat timeout marks offline; crash-loop restart (max 3) | Full reconciliation still incomplete |

Passkeys are an extension point only. TOTP is implemented.
