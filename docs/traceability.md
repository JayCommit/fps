# Traceability — requirements to milestones and tests

| Requirement | Milestone | Tests |
|---|---|---|
| Branding centralized | alpha.1 | `crates/branding` unit tests |
| Argon2id passwords | alpha.1 | `crates/auth` hash/verify |
| TOTP + recovery codes | alpha.1 | `crates/auth` totp/recovery + `vertical_slice` recovery login |
| Setup owner (once) | alpha.1 | `vertical_slice` (transactional lock) |
| Session login / 401 | alpha.1 | `vertical_slice` login tests |
| Refresh rotation / expiry | alpha.1 | `vertical_slice` refresh test |
| Viewer RBAC | alpha.1 | `vertical_slice` viewer cannot enroll |
| Enrollment token one-time | alpha.1 | `vertical_slice` replay rejected |
| Node heartbeat health | alpha.1 | `vertical_slice` status=online |
| Node mTLS heartbeat | alpha.1 | `enroll_heartbeat` agent test |
| Node cert issuance | alpha.1 | `vertical_slice` PEM present |
| Permissions defined once | alpha.1 | `domain` permission tests + dump-permissions |
| Update channel policy | alpha.1 | `crates/updater` |
| Signed manifest | alpha.1 | `crates/updater` signature tests + `release-manifest` CLI |
| Bootstrap plan / preflight | alpha.1 | `plan_and_preflight` |
| Bootstrap apply (fake) | alpha.1 | `plan_and_preflight` apply test |
| Refuse LXC game node | alpha.1 | bootstrap config test |
| Never overwrite existing VMID | alpha.1 | preflight in-use + fake cluster/resources |
| OpenAPI from source | alpha.1 | `dump-openapi` + CI generate check |
| Secret redaction | alpha.1 | config + redact tests |
| Web dashboard/nodes | alpha.1 | UI vitest + manual/preview |
| Native templates | alpha.2 (shipped in this tree) | `crates/templates` + `vertical_slice` catalogue/install job |
| Egg import | alpha.3 (shipped in this tree) | `crates/templates` import_egg |
| Stored console logs | alpha.4 (HTTP poll, not live WS) | agent log chunks + `/v1/servers/{id}/logs` |
| Desktop app | alpha.5 (source only) | `apps/desktop` Tauri 2; compile-in-VM is not a gate |
| Users / invitations / audit / revoke | this revision | `vertical_slice` invitation + revoke |
| `/docs` + `/metrics` auth | this revision | `vertical_slice` metrics_and_docs_require_auth |
