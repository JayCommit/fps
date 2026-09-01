# Desktop (Tauri 2 source)

Windows-first native shell for `FPS`. This is **not** an iframe of a
remote website. The product UI is the shared Vite app in `apps/web`. Session
tokens go through `src-tauri/src/vault.rs` (OS keyring, file `0600` fallback).

**Status:** source landed in `0.0.1-alpha.1` toward milestone `0.0.1-alpha.5`.
This is not a stable desktop release. Signed auto-update is still alpha.5.
Compiling this crate **in this Cloud Agent VM is not a gate pass** — webkit2gtk /
GTK / WebView2 are often missing here. Operators build on Windows, macOS, or a
Linux desktop with Tauri prerequisites.

`src-tauri` is an **isolated Cargo package** (empty `[workspace]` table) so
`cargo test --workspace` at the repo root does not try to compile Tauri.

## Vault

1. Try the `keyring` crate (Windows Credential Manager, macOS Keychain, Linux
   Secret Service).
2. If that fails at runtime (typical on headless Linux), write
   `{app_data}/fps/vault/session.token` with Unix mode `0600` and
   directory `0700`.

## Tray

`src/lib.rs` installs a Tauri 2 tray stub (Show / Quit) when the host has an
indicator stack. If tray creation fails, the main window still runs.

## How to run (operator desktop)

Install Tauri 2 prerequisites for your OS (see
[Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/)): Rust, a
WebView, and on Linux typically `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, and
related packages.

```bash
# from the repository root
cargo install tauri-cli --version "^2" --locked
pnpm --filter @fps/web build
cd apps/desktop/src-tauri
cargo tauri dev
```

`cargo tauri dev` starts the shared UI via `beforeDevCommand`
(`pnpm --filter @fps/web dev` on `http://127.0.0.1:47880`). Start the
control plane on `127.0.0.1:47890` in another terminal (`make control-plane`).
The Vite proxy forwards `/v1` to the API. That local UI is the product, not a
hosted marketing site.

Production bundles set `frontendDist` to `../../web/dist`. If that directory is
missing, `apps/desktop/ui/index.html` is a tiny local page that explains the
control-plane requirement — switch `frontendDist` to `../ui` only as a
fallback, never as a remote iframe.

`cargo check` in this VM may fail with missing webkit/gtk. That failure does
not block alpha.1.
