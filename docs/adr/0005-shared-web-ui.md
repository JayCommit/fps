# ADR 0005 — Shared Vite UI, Tauri deferred to alpha.5

- Status: accepted
- Date: 2026-09-01

## Decision

Ship the control panel as React + Vite in `apps/web`, with design tokens and the
API client as packages. Keep `apps/desktop` as a reserved layout without a fake
webview wrapper. Tauri 2 ships in `0.0.1-alpha.5` as specified.

## Why

Alpha.1 forbids jumping ahead to superficial later-milestone screens. Sharing
packages now prevents a second UI from diverging.
