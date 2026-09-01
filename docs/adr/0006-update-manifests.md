# ADR 0006 — Ed25519 update manifests, never GitHub /latest for prereleases

- Status: accepted
- Date: 2026-09-01

## Decision

`update-manifest.json` is canonical JSON signed with Ed25519. Channel selection
uses SemVer precedence. GitHub `/releases/latest` is not used for alpha or beta.

## Why

GitHub excludes prereleases from “latest”. The updater crate encodes this policy
with tests.
