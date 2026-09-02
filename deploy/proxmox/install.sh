#!/usr/bin/env bash
# The Proxmox guest-creator was removed. Bring your own Ubuntu/Debian machine.
set -euo pipefail
cat <<'EOF' >&2
The Proxmox-focused installer is gone.

Create the VM / VPS / dedicated server yourself (Proxmox, AWS, Azure, Hetzner,
bare metal, …), then install FPS on Ubuntu 22.04+ or Debian 12+:

  export FPS_GITHUB_TOKEN=ghp_your_token
  bash <(curl -fsSL -H "Authorization: Bearer ${FPS_GITHUB_TOKEN}" \
    https://raw.githubusercontent.com/JayCommit/fps/main/deploy/install.sh)

From a clone:  sudo -E bash deploy/install.sh

Optional: `fps bootstrap` can still create empty Proxmox guests via the HTTP API.
It does not install FPS inside them.
EOF
exit 2
