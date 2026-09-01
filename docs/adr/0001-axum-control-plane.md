# ADR 0001 — Axum for the control plane

- Status: accepted
- Date: 2026-09-01

## Decision

Use `axum` 0.8 with `tokio` for the control-plane HTTP API.

## Why

Matches the product specification, has first-class extractors, Tower middleware
for CORS/CSP/request IDs, and pairs with `utoipa` for the canonical OpenAPI
document.

## Alternatives rejected

- Actix-web: viable but extra ecosystem split.
- Warp: less conventional extractors for this team.
- A custom hyper-only server: unnecessary abstraction.

## Consequences

Handlers stay thin; domain rules live in `crates/domain` and `crates/auth`.
