# Proxmox deployment and OPNsense notes

Alpha.1 implements `fps bootstrap {init,plan,doctor,apply,status,upgrade,uninstall-plan}`.

`apply` requires `--yes`. Real Proxmox mutations also require
`FPS_ALLOW_REAL_PROXMOX=1`. CI never sets that variable.

Example (placeholders only): `deploy/examples/fry-homer.deployment.toml`.

```bash
fps bootstrap plan --config deploy/examples/fry-homer.deployment.toml
fps bootstrap plan --config deploy/examples/fry-homer.deployment.toml --fake-base http://127.0.0.1:9xxx
```

Game nodes **must** be full VMs. LXC is rejected at config validation.

## OPNsense (not automated)

Alpha.1 prints the minimal rules; it does not log into OPNsense.

- Allow administrators to the control-plane UI/API (`47890/tcp`).
- Allow game nodes to the control-plane node identity port (`47891/tcp`) and API.
- Site-to-site WireGuard between `opn02` (Fry) and `opn01` (Homer) must already
  route the guest subnets.
