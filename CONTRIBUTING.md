# Contributing

1. Use `0.0.1-alpha.N` SemVer. Conventional commits (`feat:`, `fix:`, `docs:`, `test:`, `chore:`).
2. Keep branding in `crates/branding`.
3. Run `make test` and `make generate` (commit generated OpenAPI if it changed).
4. Do not add placeholder handlers, hard-coded credentials, or `unwrap()` on service paths.
5. Every bug fix should add a regression test when reasonable.
6. Desktop and Egg import are later milestones — do not fake them.

Code of conduct: be precise, assume the operator is capable, and do not surprise
them with destructive defaults.
