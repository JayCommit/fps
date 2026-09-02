# Optional Proxmox guest create

FPS no longer creates LXC/QEMU guests for you. Bring your own Ubuntu 22.04+
or Debian 12+ machine (a Proxmox guest is fine), then run
`deploy/install.sh` **inside** it. See `docs/operations/install.md`.

## API-only empty guests (`fps bootstrap apply`)

The Rust CLI can create **empty** guests through the Proxmox HTTP API from a
laptop. That path still requires `--yes` and `FPS_ALLOW_REAL_PROXMOX=1` (see
`docs/adr/0007-bootstrap-safety.md`). It does **not** install FPS inside the
guest.

```bash
fps bootstrap plan --config deploy/examples/fry-control-plane.deployment.toml --role control-plane
FPS_ALLOW_REAL_PROXMOX=1 FPS_FRY_TOKEN_SECRET=… \
  fps bootstrap apply --config deploy/examples/fry-control-plane.deployment.toml --role control-plane --yes
```

After the guest boots, SSH in and run `deploy/install.sh`.

Existing VMIDs are never overwritten. OPNsense is never mutated.
