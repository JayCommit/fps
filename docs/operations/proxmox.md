# Proxmox deployment (Fry + Homer)

Two independent Proxmox hosts. Not a cluster. Game runtime is always a full VM.

| Host | Role | What you install |
|---|---|---|
| **Fry** | Control plane | Web panel + API + MariaDB |
| **Homer** | Game host | Docker Engine + node agent |

## 1. Create the guest (from a laptop that can reach the Proxmox API)

Copy an example, fill in real values (token secret stays in an env var, never in the file):

```bash
# Fry only
fps bootstrap plan --config deploy/examples/fry-control-plane.deployment.toml --role control-plane
FPS_ALLOW_REAL_PROXMOX=1 FPS_FRY_TOKEN_SECRET=… \
  fps bootstrap apply --config deploy/examples/fry-control-plane.deployment.toml --role control-plane --yes

# Homer only
fps bootstrap plan --config deploy/examples/homer-game-host.deployment.toml --role game-host
FPS_ALLOW_REAL_PROXMOX=1 FPS_HOMER_TOKEN_SECRET=… \
  fps bootstrap apply --config deploy/examples/homer-game-host.deployment.toml --role game-host --yes
```

`apply` requires `--yes`. Real Proxmox also requires `FPS_ALLOW_REAL_PROXMOX=1`.
Existing VMIDs are never overwritten. CI never sets that variable.

`--role both` (default) uses `deploy/examples/fry-homer.deployment.toml` and creates both guests.

`apply` only creates the guest. It does not install FPS inside it.

## 2. Inside the guest: pick web or game host

```bash
fps install
```

The installer asks:

1. **Control plane** — web panel + API (Fry)
2. **Game host** — Docker + node agent (Homer)
3. **Both** — lab only

Non-interactive:

```bash
fps install --role control-plane
fps install --role game-host --start
```

Same picker without the CLI, if you only copied the deploy tree:

```bash
sudo bash deploy/install/install.sh --role control-plane
sudo bash deploy/install/install.sh --role game-host
```

Units are written and not started unless you pass `--start`. Edit `/etc/fps/*.env` first.

## 3. Finish each role

Control plane: MariaDB, `FPS_MASTER_KEY`, open the UI, create the owner.

Game host: Docker Engine, enrollment token from the UI, `fps-node-agent enroll …`.

## OPNsense (not automated)

This release prints firewall notes. It does not log into OPNsense.

- Allow administrators to the control-plane UI/API (`47890/tcp`).
- Allow game nodes to the control-plane node identity port (`47891/tcp`) and API.
- Site-to-site WireGuard between `opn02` (Fry) and `opn01` (Homer) must already
  route the guest subnets.

Game nodes **must** be full VMs. LXC is rejected at config validation.
