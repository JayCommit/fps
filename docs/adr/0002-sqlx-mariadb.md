# ADR 0002 — SQLx and MariaDB first

- Status: accepted
- Date: 2026-09-01

## Decision

Use SQLx 0.8 with the MySQL driver against MariaDB 10.11+. Hide SQL behind
repository modules so PostgreSQL can be added later without rewriting domain
logic. Runtime-checked queries (not `query!` macros) so the crate builds without
a live database.

## Why

Fry already runs `maria02`. SQLx is async, migration-capable, and widely
maintained.

## Alternatives rejected

- Diesel: sync-oriented by default.
- SeaORM: extra abstraction without a current payoff.
- SQLite as the primary store: does not match the deployment class.

## Consequences

Integration tests require MariaDB. Schema version is `1` in alpha.1.
