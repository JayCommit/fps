# Threat model skeleton (alpha.1)

This is a living document. Beta requires a full review. No critical/high issues
may remain at `0.0.1-beta.1`.

| Scenario | Impact | Alpha.1 mitigations | Residual risk |
|---|---|---|---|
| Control plane compromise | Full tenant control | Argon2id, session hashing, master key outside DB, audit events, CSP, CSRF token issued | Process RCE still fatal; harden host |
| Node compromise | Game workloads on that node | One-time enroll, hashed node token, certs 0600, no host install scripts | Stolen node token can heartbeat/spoof that node |
| Malicious template | Host escape | Not in alpha.1; later: reject privileged mounts, run installers in ephemeral containers | — |
| Compromised game container | Lateral movement | Later: dropped caps, non-root, controlled egress | Docker not used for games yet |
| Stolen desktop token | Account takeover | Desktop not shipped; web tokens in localStorage (alpha.1 limitation) | Move to httpOnly cookies + vault in alpha.5 |
| Supply-chain attack | Malicious binary | Pinned GH Actions SHAs, checksums, signed manifests, cargo.lock | Signing keys must stay in protected environments |
| Split connectivity | Split-brain | Agent keeps running locally; heartbeat timeout marks offline | Reconciliation lands alpha.2 |

Passkeys are an extension point only. TOTP is implemented.
