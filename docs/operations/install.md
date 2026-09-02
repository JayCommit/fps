# Install FPS (Ubuntu / Debian)

The operator creates the machine. This installer does **not** provision
Proxmox, AWS, Azure, or any hypervisor. Run it **on** the Ubuntu or Debian
VM / VPS / dedicated server that should become the panel or a game host.

| Role | What gets installed | Typical box |
|---|---|---|
| **Control plane** | Web UI, API, MariaDB, systemd | Any Ubuntu 22.04+ / Debian 12+ |
| **Game host** | Docker Engine + node agent | Full VM or dedicated (not LXC) |
| **Both** | Everything on one machine | Lab / single VPS |

## One command

SSH in as root. The repository is public:

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/JayCommit/fps/main/deploy/install.sh)
```

`curl … | bash` also works. Prompts read `/dev/tty`, so the role menu still
appears when stdin is the pipe.

The installer draws a menu (whiptail when present, numbered list otherwise):
**control plane**, **game host**, or **both**. It then asks y/n questions
(public address, local vs remote MariaDB, unencrypted HTTP, enroll now, start
services) and a final confirm. Builds from source take 15–40 minutes.

If FPS is **already installed**, the installer detects it and offers:

1. **Reconfigure** — change public IP, CORS, HTTP, and database URL without rebuilding
2. **Upgrade** — rebuild from source, keep secrets, still refresh the public URL
3. **Repair** — rewrite systemd units and restart

```bash
sudo bash deploy/install.sh --reconfigure --public-host NEW_IP
sudo bash deploy/install.sh --reconfigure --database-url 'mysql://fps:secret@db.internal:3306/fps'
```

Unattended reconfigure (no cargo/pnpm build):

```bash
sudo bash deploy/install.sh --role control-plane --yes --reconfigure \
  --public-host 203.0.113.10
```

If you already cloned the repo onto the machine:

```bash
sudo bash deploy/install.sh --role control-plane
sudo bash deploy/install.sh --role game-host
```

### Unattended (no prompts)

```bash
sudo bash deploy/install.sh --role control-plane --yes
sudo bash deploy/install.sh --role game-host --yes \
  --control-plane-url http://PANEL_IP:47890 \
  --enroll-token PASTE_TOKEN_HERE
```

`--dry-run` prints apt/cargo/systemctl commands and does not mutate anything.

### Remote MariaDB

Local MariaDB is the default. To use a database on another host, skip the local
server and pass a URL (password is URL-encoded by the installer when prompted):

```bash
sudo bash deploy/install.sh --role control-plane --yes --no-mariadb \
  --database-url 'mysql://fps:secret@db.internal:3306/fps'
```

Or interactively: answer **n** to “Install MariaDB on this machine?” and fill in
host, port, database, user, and password. Create the empty `fps` database on the
remote server first; the control plane runs migrations on startup.

`--reconfigure` can switch an existing panel to a remote database the same way.

## What “fully” means

**Control plane**

1. Detects Ubuntu or Debian; refuses other distros.
2. Installs MariaDB, Rust 1.98, Node 22, pnpm.
3. Builds `fps-bootstrap` (the `fps` CLI), `fps-control-plane`, and the web UI (`pnpm --filter @fps/web build`). Do not pass `cargo -p fps` — that package id does not exist.
4. Serves the panel from the control plane (`FPS_WEB_ROOT`, UI on **47880**, API on **47890**).
5. Starts `fps-control-plane.service`.

Open `http://MACHINE_IP:47880` and create the owner account.

**Game host**

1. Installs Docker Engine from `download.docker.com/linux/{ubuntu|debian}`. Ubuntu 26.04 / Debian testing use the noble / bookworm pockets until Docker publishes their own.
2. Builds `fps-bootstrap` and `fps-node-agent`.
3. Does **not** enroll unless you pass `--enroll-token` and `--control-plane-url`.

Then in the panel: **Nodes → create an enrollment token**, and on the game host:

```bash
fps-node-agent enroll \
  --url http://PANEL_IP:47890 \
  --token PASTE_TOKEN_HERE \
  --data-dir /var/lib/fps/agent \
  --allow-insecure-http

systemctl enable --now fps-node-agent.service
```

If you answered **y** to “Allow unencrypted HTTP?”, `/etc/fps/node-agent.env` must
contain `FPS_ALLOW_INSECURE_HTTP=true`. The systemd unit does not pass
`--allow-insecure-http`; the env file (or a stored `http://` identity from enroll)
is how `run` is allowed to heartbeat over HTTP. Re-run
`sudo bash deploy/install.sh --reconfigure` on the game host to write the key,
then `systemctl restart fps-node-agent`.

Game hosts **cannot** be LXC. The installer refuses a game-host role inside a
container.

## Supported OS

- Ubuntu 22.04 LTS, 24.04 LTS, and 26.04 (26.04 uses Docker's noble apt pocket)
- Debian 12 and newer (11 warns; testing/sid uses bookworm Docker packages)
- Architectures: `amd64` and `arm64`

## Firewall

Open, at minimum:

- Administrators → panel TCP **47880** (web UI) and TCP **47890** (API)
- Game hosts → panel TCP **47890** (API) and TCP **47891** (node mTLS)
- Players → the game ports you allocate on each game host

## Units-only (binaries already on disk)

```bash
fps install --role control-plane --start
# or:
sudo bash deploy/install/install.sh --role game-host --start
```

## Optional: create empty Proxmox guests from a laptop

`fps bootstrap apply` can still create empty guests through the Proxmox HTTP
API. That path requires `--yes` and `FPS_ALLOW_REAL_PROXMOX=1` (see
`docs/adr/0007-bootstrap-safety.md`). It does **not** install FPS inside the
guest. Prefer `deploy/install.sh` on the machine itself. Details:
`docs/operations/proxmox.md`.
