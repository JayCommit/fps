# Proxmox deployment (Fry + Homer)

Two independent Proxmox hosts. Not a cluster. The installer below creates the
guest **and fully builds FPS inside it**.

| Host | Role | Guest | What gets installed |
|---|---|---|---|
| **Fry** | Control plane | LXC (default) | Web panel, API, MariaDB |
| **Homer** | Game host | **QEMU VM only** | Docker Engine + node agent |

## One command on the Proxmox host

SSH in as root. The repository is private, so pass a GitHub token that can read
it (`contents:read`) and keep the token when using sudo (`-E`):

```bash
export FPS_GITHUB_TOKEN=ghp_your_token
bash <(curl -fsSL -H "Authorization: Bearer ${FPS_GITHUB_TOKEN}" \
  https://raw.githubusercontent.com/JayCommit/fps/main/deploy/proxmox/install.sh)
```

`curl … | bash` also works. Prompts read `/dev/tty`, so the role menu still appears when stdin is the pipe.

The installer asks **web UI (Fry)** vs **game host (Homer)** (whiptail menu on Proxmox, numbered list otherwise), then VMID, RAM,
disk, bridge, and IP. Type `yes` to create the guest. It clones this repo inside
the guest and builds from source (15–40 minutes). Existing VMIDs are never
overwritten.

If you already cloned the repo onto the Proxmox host:

```bash
sudo -E bash deploy/proxmox/install.sh --role control-plane
sudo -E bash deploy/proxmox/install.sh --role game-host
```

Non-interactive example:

```bash
sudo -E bash deploy/proxmox/install.sh \
  --role control-plane --yes \
  --vmid 101 --hostname fry \
  --cores 4 --memory 8192 --disk 32 \
  --storage local-lvm --bridge vmbr0 --ip dhcp
```

`--dry-run` prints `pct` / `qm` commands and does not mutate anything.

### What “fully” means

**Control plane (LXC)**

1. `pct create` Debian 12, start, wait for network.
2. `pct exec` installs MariaDB, Rust 1.98, Node 22, pnpm.
3. Builds `fps-control-plane` and the web UI (`pnpm --filter @fps/web build`).
4. Serves the panel from the control plane (`FPS_WEB_ROOT`, UI on **47880**, API on **47890**).
5. Starts `fps-control-plane.service`.

Open `http://GUEST_IP:47880` and create the owner account.

**Game host (QEMU VM)**

1. Imports a Debian 12 cloud image, cloud-init, qemu-guest-agent.
2. First boot installs Docker Engine, builds `fps-node-agent`.
3. Does **not** enroll unless you pass `--enroll-token` and `--control-plane-url`.

Then in the Fry UI: **Nodes → create an enrollment token**, and on Homer:

```bash
fps-node-agent enroll \
  --url http://FRY_IP:47890 \
  --token PASTE_TOKEN_HERE \
  --data-dir /var/lib/fps/agent \
  --allow-insecure-http

systemctl enable --now fps-node-agent.service
```

Game hosts **cannot** be LXC. The installer refuses `--role game-host --guest-type lxc`.

### Re-run provision without recreating the guest

```bash
sudo -E bash deploy/proxmox/install.sh --role control-plane --provision-only --vmid 101 --yes
```

## API-only alternative (`fps bootstrap apply`)

The Rust CLI can create empty guests through the Proxmox HTTP API from a laptop.
That path still requires `--yes` and `FPS_ALLOW_REAL_PROXMOX=1` (see
`docs/adr/0007-bootstrap-safety.md`). It does **not** install FPS inside the
guest. Prefer `deploy/proxmox/install.sh` on the host.

```bash
fps bootstrap plan --config deploy/examples/fry-control-plane.deployment.toml --role control-plane
FPS_ALLOW_REAL_PROXMOX=1 FPS_FRY_TOKEN_SECRET=… \
  fps bootstrap apply --config deploy/examples/fry-control-plane.deployment.toml --role control-plane --yes
```

## In-guest units only

If the guest already exists and binaries are on disk:

```bash
fps install --role control-plane --start
fps install --role game-host --start
```

## OPNsense (not automated)

This installer prints firewall notes. It does not log into OPNsense.

- Allow administrators to the web UI (`47880/tcp`) and API (`47890/tcp`).
- Allow game nodes to the API (`47890/tcp`) and node identity port (`47891/tcp`).
- Site-to-site WireGuard between `opn02` (Fry) and `opn01` (Homer) must already
  route the guest subnets.
