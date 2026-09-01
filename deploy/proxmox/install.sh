#!/usr/bin/env bash
# FPS Proxmox installer — run as root on the Proxmox host (Fry or Homer).
#
# Private repo (typical for this alpha):
#   export FPS_GITHUB_TOKEN=ghp_...     # contents:read
#   curl -fsSL -H "Authorization: Bearer ${FPS_GITHUB_TOKEN}" \
#     https://raw.githubusercontent.com/JayCommit/fps/main/deploy/proxmox/install.sh \
#     | sudo -E bash
#
# Already cloned:
#   sudo -E bash deploy/proxmox/install.sh
#
# This script creates the LXC or VM AND fully builds FPS inside it.
set -euo pipefail

FPS_GIT_OWNER="${FPS_GIT_OWNER:-JayCommit}"
FPS_GIT_REPO="${FPS_GIT_REPO:-fps}"
FPS_GIT_REF="${FPS_GIT_REF:-main}"
FPS_RAW_BASE="${FPS_RAW_BASE:-https://raw.githubusercontent.com/${FPS_GIT_OWNER}/${FPS_GIT_REPO}/${FPS_GIT_REF}}"

ROLE=""
VMID=""
HOSTNAME_GUEST=""
CORES=""
MEMORY=""
DISK=""
BRIDGE=""
IP_CIDR=""
GATEWAY=""
DNS=""
PASSWORD=""
SSH_PUBKEY=""
GUEST_TYPE=""
CONTROL_PLANE_URL=""
ENROLL_TOKEN=""
PROVISION_ONLY=0
DRY_RUN=0
YES=0
ASSUME_PROXMOX=0
EXISTING_VMIDS=""
TEMPLATE_STORAGE=""
DISK_STORAGE=""
BUNDLE_DIR=""
EPHEMERAL_KEY=""

usage() {
  cat <<'EOF'
Usage: install.sh [options]

Run on the Proxmox VE host. Creates a guest and fully installs FPS inside it.

  --role ROLE             control-plane (Fry web UI + API) or game-host (Homer)
  --vmid ID               guest VMID (default 101 control-plane, 201 game-host)
  --hostname NAME         fry | homer
  --cores N               vCPU count
  --memory MB             RAM in MiB
  --disk GB               disk size in GiB
  --storage NAME          disk storage (default: first rootdir/images storage)
  --template-storage NAME storage for LXC templates / snippets / cloud images
  --bridge NAME           Linux bridge (default vmbr0)
  --ip CIDR|dhcp          guest IPv4 (default dhcp)
  --gateway ADDR          IPv4 gateway (required when --ip is static)
  --dns ADDR              DNS server (default 1.1.1.1)
  --password PASS         LXC root password (generated if omitted)
  --ssh-key FILE|STRING   SSH public key for the guest
  --guest-type lxc|vm     default lxc for control-plane, vm for game-host
  --control-plane-url URL game-host: enroll URL (optional)
  --enroll-token TOKEN    game-host: enroll immediately (optional)
  --provision-only        skip guest create; bootstrap an existing VMID
  --yes                   do not ask for confirmation
  --dry-run               print pct/qm commands; do not mutate
  --assume-proxmox        skip /etc/pve detection (tests)
  --existing-vmids LIST   comma-separated VMIDs treated as in-use (tests)
  --help                  this text

Environment:
  FPS_GITHUB_TOKEN        GitHub PAT for a private clone (contents:read)
  FPS_GIT_URL             override clone URL
  FPS_GIT_REF             git branch or tag (default main)
  FPS_CLOUD_IMAGE_URL     Debian cloud qcow2 for the game-host VM

Game hosts are always QEMU VMs. LXC is refused for Docker game runtime.
Existing VMIDs are never overwritten. OPNsense is never mutated.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --role) ROLE="${2:?}"; shift 2 ;;
    --vmid) VMID="${2:?}"; shift 2 ;;
    --hostname) HOSTNAME_GUEST="${2:?}"; shift 2 ;;
    --cores) CORES="${2:?}"; shift 2 ;;
    --memory) MEMORY="${2:?}"; shift 2 ;;
    --disk) DISK="${2:?}"; shift 2 ;;
    --storage) DISK_STORAGE="${2:?}"; shift 2 ;;
    --template-storage) TEMPLATE_STORAGE="${2:?}"; shift 2 ;;
    --bridge) BRIDGE="${2:?}"; shift 2 ;;
    --ip) IP_CIDR="${2:?}"; shift 2 ;;
    --gateway) GATEWAY="${2:?}"; shift 2 ;;
    --dns) DNS="${2:?}"; shift 2 ;;
    --password) PASSWORD="${2:?}"; shift 2 ;;
    --ssh-key) SSH_PUBKEY="${2:?}"; shift 2 ;;
    --guest-type) GUEST_TYPE="${2:?}"; shift 2 ;;
    --control-plane-url) CONTROL_PLANE_URL="${2:?}"; shift 2 ;;
    --enroll-token) ENROLL_TOKEN="${2:?}"; shift 2 ;;
    --provision-only) PROVISION_ONLY=1; shift ;;
    --yes) YES=1; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    --assume-proxmox) ASSUME_PROXMOX=1; shift ;;
    --existing-vmids) EXISTING_VMIDS="${2:?}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

here=""
src="${BASH_SOURCE[0]:-}"
if [[ -n "${src}" && -f "${src}" && "${src}" != /dev/fd/* && "${src}" != /proc/self/fd/* ]]; then
  here="$(cd "$(dirname "${src}")" && pwd)"
fi

resolve_bundle() {
  if [[ -n "${here}" && -f "${here}/lib.sh" ]]; then
    BUNDLE_DIR="${here}"
    return 0
  fi
  BUNDLE_DIR="$(mktemp -d /tmp/fps-proxmox-XXXXXX)"
  local files=(lib.sh guest-control-plane.sh guest-game-host.sh)
  local f
  for f in "${files[@]}"; do
    if ! github_fetch_file "deploy/proxmox/${f}" >"${BUNDLE_DIR}/${f}"; then
      echo "failed to download deploy/proxmox/${f} from GitHub." >&2
      echo "Clone the repo on the Proxmox host and run deploy/proxmox/install.sh," >&2
      echo "or export FPS_GITHUB_TOKEN for a private repository." >&2
      exit 1
    fi
    chmod +x "${BUNDLE_DIR}/${f}" 2>/dev/null || true
  done
}

github_fetch_file() {
  local path="$1"
  local url="${FPS_RAW_BASE}/${path}"
  local args=(curl -fsSL --retry 3 --retry-delay 2)
  if [[ -n "${FPS_GITHUB_TOKEN:-}" ]]; then
    args+=(-H "Authorization: Bearer ${FPS_GITHUB_TOKEN}" -H "Accept: application/vnd.github.raw")
  fi
  "${args[@]}" "${url}"
}

resolve_bundle
# shellcheck disable=SC1091
source "${BUNDLE_DIR}/lib.sh"
export DRY_RUN YES ASSUME_PROXMOX EXISTING_VMIDS

pick_role() {
  if [[ -n "${ROLE}" ]]; then
    ROLE="$(normalize_role "${ROLE}")"
    return 0
  fi
  if [[ ! -t 0 ]]; then
    die "no TTY: pass --role control-plane or --role game-host"
  fi
  cat <<'EOF'

What should this Proxmox host build?

  1) Control plane   Web UI + API + MariaDB     (Fry — LXC by default)
  2) Game host       Docker Engine + node agent (Homer — full VM, never LXC)

EOF
  local choice=""
  printf 'Select 1 or 2: '
  read -r choice
  ROLE="$(normalize_role "${choice}")"
}

apply_role_defaults() {
  if [[ "${ROLE}" == "control-plane" ]]; then
    GUEST_TYPE="${GUEST_TYPE:-lxc}"
    VMID="${VMID:-101}"
    HOSTNAME_GUEST="${HOSTNAME_GUEST:-fry}"
    CORES="${CORES:-4}"
    MEMORY="${MEMORY:-8192}"
    DISK="${DISK:-32}"
  else
    GUEST_TYPE="${GUEST_TYPE:-vm}"
    VMID="${VMID:-201}"
    HOSTNAME_GUEST="${HOSTNAME_GUEST:-homer}"
    CORES="${CORES:-4}"
    MEMORY="${MEMORY:-8192}"
    DISK="${DISK:-64}"
  fi
  GUEST_TYPE="$(echo "${GUEST_TYPE}" | tr '[:upper:]' '[:lower:]')"
  case "${GUEST_TYPE}" in
    lxc | ct | container) GUEST_TYPE=lxc ;;
    vm | qemu | kvm) GUEST_TYPE=vm ;;
    *) die "unknown --guest-type ${GUEST_TYPE} (lxc or vm)" ;;
  esac
  if [[ "${ROLE}" == "game-host" && "${GUEST_TYPE}" == "lxc" ]]; then
    die "Game hosts must be full VMs. LXC is not supported for the Docker game runtime."
  fi
  BRIDGE="${BRIDGE:-vmbr0}"
  IP_CIDR="${IP_CIDR:-dhcp}"
  DNS="${DNS:-1.1.1.1}"
  TEMPLATE_STORAGE="${TEMPLATE_STORAGE:-$(default_template_storage)}"
  DISK_STORAGE="${DISK_STORAGE:-$(default_disk_storage)}"
}

prompt_missing() {
  prompt_value VMID "VMID" "${VMID}"
  prompt_value HOSTNAME_GUEST "Hostname" "${HOSTNAME_GUEST}"
  prompt_value CORES "CPU cores" "${CORES}"
  prompt_value MEMORY "Memory (MiB)" "${MEMORY}"
  prompt_value DISK "Disk (GiB)" "${DISK}"
  prompt_value DISK_STORAGE "Disk storage" "${DISK_STORAGE}"
  prompt_value TEMPLATE_STORAGE "Template / snippet storage" "${TEMPLATE_STORAGE}"
  prompt_value BRIDGE "Bridge" "${BRIDGE}"
  prompt_value IP_CIDR "IPv4 (CIDR or dhcp)" "${IP_CIDR}"
  if [[ "${IP_CIDR}" != "dhcp" ]]; then
    prompt_value GATEWAY "Gateway" "${GATEWAY:-}"
    [[ -n "${GATEWAY}" ]] || die "--gateway is required when --ip is not dhcp"
  fi
  prompt_value DNS "DNS" "${DNS}"
  if [[ "${GUEST_TYPE}" == "lxc" ]]; then
    prompt_secret PASSWORD "LXC root password"
  fi
  if [[ "${ROLE}" == "game-host" ]]; then
    prompt_value CONTROL_PLANE_URL "Control plane URL (empty = enroll later)" "${CONTROL_PLANE_URL:-}"
    if [[ -n "${CONTROL_PLANE_URL}" ]]; then
      prompt_secret ENROLL_TOKEN "Enrollment token"
      ENROLL_TOKEN="${ENROLL_TOKEN:-}"
    fi
  fi
}

net0_arg() {
  if [[ "${IP_CIDR}" == "dhcp" ]]; then
    if [[ "${GUEST_TYPE}" == "lxc" ]]; then
      echo "name=eth0,bridge=${BRIDGE},ip=dhcp"
    else
      echo "virtio,bridge=${BRIDGE}"
    fi
  else
    if [[ "${GUEST_TYPE}" == "lxc" ]]; then
      echo "name=eth0,bridge=${BRIDGE},ip=${IP_CIDR},gw=${GATEWAY}"
    else
      echo "virtio,bridge=${BRIDGE}"
    fi
  fi
}

ipconfig0_arg() {
  if [[ "${IP_CIDR}" == "dhcp" ]]; then
    echo "ip=dhcp"
  else
    echo "ip=${IP_CIDR},gw=${GATEWAY}"
  fi
}

ensure_lxc_template() {
  local tmpl="${LXC_TEMPLATE:-}"
  if [[ -z "${tmpl}" ]]; then
    if [[ "${DRY_RUN}" -eq 1 ]]; then
      tmpl="debian-12-standard_12.7-1_amd64.tar.zst"
      echo "+ pveam update"
      echo "+ pveam download ${TEMPLATE_STORAGE} ${tmpl}"
      LXC_TEMPLATE_VOL="${TEMPLATE_STORAGE}:vztmpl/${tmpl}"
      return 0
    fi
    info "Refreshing LXC template catalogue"
    pveam update >/dev/null || true
    tmpl="$(pveam available -section system 2>/dev/null | awk '/debian-12-standard.*amd64/ {print $2}' | tail -n1)"
    [[ -n "${tmpl}" ]] || die "could not find debian-12-standard in pveam available"
    if ! pveam list "${TEMPLATE_STORAGE}" 2>/dev/null | grep -q "${tmpl}"; then
      info "Downloading ${tmpl} to ${TEMPLATE_STORAGE}"
      pveam download "${TEMPLATE_STORAGE}" "${tmpl}"
    fi
  fi
  LXC_TEMPLATE_VOL="${TEMPLATE_STORAGE}:vztmpl/${tmpl}"
}

create_lxc() {
  ensure_lxc_template
  if [[ -z "${PASSWORD}" ]]; then
    PASSWORD="$(random_password)"
  fi
  info "Creating LXC ${VMID} (${HOSTNAME_GUEST})"
  run pct create "${VMID}" "${LXC_TEMPLATE_VOL}" \
    --hostname "${HOSTNAME_GUEST}" \
    --cores "${CORES}" \
    --memory "${MEMORY}" \
    --swap 512 \
    --rootfs "${DISK_STORAGE}:${DISK}" \
    --net0 "$(net0_arg)" \
    --nameserver "${DNS}" \
    --unprivileged 1 \
    --features nesting=1,keyctl=1 \
    --password "${PASSWORD}" \
    --onboot 1 \
    --ostype debian \
    --start 0
  if [[ -n "${SSH_PUBKEY}" ]]; then
    local keyfile="${SSH_PUBKEY}"
    if [[ ! -f "${keyfile}" ]]; then
      keyfile="$(mktemp /tmp/fps-ssh-XXXXXX.pub)"
      printf '%s\n' "${SSH_PUBKEY}" >"${keyfile}"
    fi
    run pct set "${VMID}" --ssh-public-keys "${keyfile}"
  fi
  run pct start "${VMID}"
}

wait_lxc_network() {
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    info "dry-run: would wait for network in CT ${VMID}"
    return 0
  fi
  info "Waiting for network in CT ${VMID}…"
  local i
  for i in $(seq 1 60); do
    if pct exec "${VMID}" -- ping -c1 -W1 1.1.1.1 >/dev/null 2>&1 \
      || pct exec "${VMID}" -- ping -c1 -W1 8.8.8.8 >/dev/null 2>&1; then
      ok "CT ${VMID} can reach the internet"
      return 0
    fi
    sleep 2
  done
  die "CT ${VMID} has no network. Check bridge ${BRIDGE} and IP settings."
}

provision_lxc() {
  local guest="${BUNDLE_DIR}/guest-control-plane.sh"
  info "Pushing control-plane bootstrap into CT ${VMID}"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ pct push ${VMID} ${guest} /root/fps-guest-bootstrap.sh"
    echo "+ pct exec ${VMID} -- env FPS_GIT_URL=… bash /root/fps-guest-bootstrap.sh"
    return 0
  fi
  pct push "${VMID}" "${guest}" /root/fps-guest-bootstrap.sh --perms 0755
  local token_file=""
  if [[ -n "${FPS_GITHUB_TOKEN:-}" ]]; then
    token_file="$(mktemp /tmp/fps-token-XXXXXX)"
    printf '%s' "${FPS_GITHUB_TOKEN}" >"${token_file}"
    chmod 600 "${token_file}"
    pct push "${VMID}" "${token_file}" /root/.fps-github-token --perms 0600
    rm -f "${token_file}"
  fi
  pct exec "${VMID}" -- env \
    FPS_GIT_URL="${FPS_GIT_URL:-https://github.com/${FPS_GIT_OWNER}/${FPS_GIT_REPO}.git}" \
    FPS_GIT_REF="${FPS_GIT_REF}" \
    FPS_GITHUB_TOKEN="${FPS_GITHUB_TOKEN:-}" \
    bash /root/fps-guest-bootstrap.sh
}

cloud_image_path() {
  local dest="/var/tmp/fps-debian-12-genericcloud-amd64.qcow2"
  local url="${FPS_CLOUD_IMAGE_URL:-https://cloud.debian.org/images/cloud/bookworm/latest/debian-12-genericcloud-amd64.qcow2}"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    info "dry-run: would download ${url} → ${dest}"
    echo "${dest}"
    return 0
  fi
  if [[ ! -s "${dest}" ]]; then
    info "Downloading Debian 12 cloud image"
    curl -fL --retry 3 -o "${dest}.partial" "${url}"
    mv "${dest}.partial" "${dest}"
  fi
  echo "${dest}"
}

write_cloudinit() {
  local snippet_dir="/var/lib/vz/snippets"
  local snippet_name="fps-${VMID}-user.yaml"
  local guest_b64 env_b64
  guest_b64="$(base64 -w0 "${BUNDLE_DIR}/guest-game-host.sh" 2>/dev/null || base64 "${BUNDLE_DIR}/guest-game-host.sh" | tr -d '\n')"
  local env_body=""
  env_body+="FPS_GIT_URL=${FPS_GIT_URL:-https://github.com/${FPS_GIT_OWNER}/${FPS_GIT_REPO}.git}"$'\n'
  env_body+="FPS_GIT_REF=${FPS_GIT_REF}"$'\n'
  env_body+="FPS_GITHUB_TOKEN=${FPS_GITHUB_TOKEN:-}"$'\n'
  env_body+="FPS_CONTROL_PLANE_URL=${CONTROL_PLANE_URL:-}"$'\n'
  env_body+="FPS_ENROLL_TOKEN=${ENROLL_TOKEN:-}"$'\n'
  env_b64="$(printf '%s' "${env_body}" | base64 -w0 2>/dev/null || printf '%s' "${env_body}" | base64 | tr -d '\n')"

  local yaml
  yaml="$(cat <<EOF
#cloud-config
hostname: ${HOSTNAME_GUEST}
manage_etc_hosts: true
package_update: true
packages:
  - qemu-guest-agent
  - curl
  - ca-certificates
  - python3
write_files:
  - path: /usr/local/sbin/fps-guest-bootstrap
    permissions: '0755'
    encoding: b64
    content: ${guest_b64}
  - path: /etc/fps-guest.env
    permissions: '0600'
    encoding: b64
    content: ${env_b64}
runcmd:
  - [systemctl, enable, --now, qemu-guest-agent]
  - [bash, -lc, "set -a; . /etc/fps-guest.env; set +a; /usr/local/sbin/fps-guest-bootstrap >> /var/log/fps-provision.log 2>&1"]
EOF
)"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ mkdir -p ${snippet_dir}"
    echo "+ write ${snippet_dir}/${snippet_name} (cloud-init first-boot provision)"
    CLOUDINIT_SNIPPET="local:snippets/${snippet_name}"
    return 0
  fi
  mkdir -p "${snippet_dir}"
  printf '%s\n' "${yaml}" >"${snippet_dir}/${snippet_name}"
  CLOUDINIT_SNIPPET="local:snippets/${snippet_name}"
}

prepare_ssh_keys() {
  local combined=""
  EPHEMERAL_KEY="$(mktemp /tmp/fps-ephemeral-XXXXXX)"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ ssh-keygen -t ed25519 -f ${EPHEMERAL_KEY} -N ''"
    SSH_KEYS_FILE="${EPHEMERAL_KEY}.pub"
    printf 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFdryrun fps@dry-run\n' >"${SSH_KEYS_FILE}"
    return 0
  fi
  ssh-keygen -t ed25519 -f "${EPHEMERAL_KEY}" -N "" -C "fps-proxmox-install-${VMID}" >/dev/null
  combined="${EPHEMERAL_KEY}.pub"
  if [[ -n "${SSH_PUBKEY}" ]]; then
    local extra="${SSH_PUBKEY}"
    if [[ ! -f "${extra}" ]]; then
      extra="$(mktemp /tmp/fps-userkey-XXXXXX.pub)"
      printf '%s\n' "${SSH_PUBKEY}" >"${extra}"
    fi
    cat "${extra}" >>"${combined}"
  fi
  SSH_KEYS_FILE="${combined}"
}

create_vm() {
  local image
  image="$(cloud_image_path)"
  prepare_ssh_keys
  write_cloudinit
  info "Creating QEMU VM ${VMID} (${HOSTNAME_GUEST})"
  run qm create "${VMID}" \
    --name "${HOSTNAME_GUEST}" \
    --ostype l26 \
    --machine q35 \
    --cpu host \
    --cores "${CORES}" \
    --memory "${MEMORY}" \
    --net0 "$(net0_arg)" \
    --scsihw virtio-scsi-single \
    --agent enabled=1,fstrim_cloned_disks=1 \
    --onboot 1 \
    --serial0 socket \
    --vga serial0
  run qm importdisk "${VMID}" "${image}" "${DISK_STORAGE}"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ qm set ${VMID} --scsi0 ${DISK_STORAGE}:vm-${VMID}-disk-0,discard=on,iothread=1"
  else
    local unused
    unused="$(qm config "${VMID}" | awk '/^unused0:/ {print $2; exit}')"
    [[ -n "${unused}" ]] || die "qm importdisk did not produce unused0 for VM ${VMID}"
    qm set "${VMID}" --scsi0 "${unused},discard=on,iothread=1"
  fi
  run qm set "${VMID}" --boot order=scsi0
  run qm set "${VMID}" --ide2 "${DISK_STORAGE}:cloudinit"
  run qm set "${VMID}" --ipconfig0 "$(ipconfig0_arg)"
  run qm set "${VMID}" --nameserver "${DNS}"
  run qm set "${VMID}" --ciuser debian
  run qm set "${VMID}" --sshkeys "${SSH_KEYS_FILE}"
  run qm set "${VMID}" --cicustom "user=${CLOUDINIT_SNIPPET}"
  # Resize after import so the cloud image is not stuck at ~3 GiB.
  run qm resize "${VMID}" scsi0 "${DISK}G" || true
  run qm start "${VMID}"
}

wait_vm_provision() {
  wait_for_guest_agent "${VMID}" 120
  local ip=""
  local i
  if [[ "${IP_CIDR}" != "dhcp" ]]; then
    ip="${IP_CIDR%%/*}"
  fi
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ wait for /var/lib/fps/.provision-complete via qemu-guest-agent"
    GUEST_IP="${ip:-10.0.0.10}"
    return 0
  fi
  for i in $(seq 1 30); do
    ip="$(guest_ipv4 "${VMID}" 2>/dev/null || true)"
    [[ -n "${ip}" ]] && break
    sleep 2
  done
  GUEST_IP="${ip:-unknown}"
  info "Guest IPv4: ${GUEST_IP}. Waiting for FPS provision (Rust build, 15–40 minutes)…"
  local tries=180
  for i in $(seq 1 "${tries}"); do
    if qm guest exec "${VMID}" -- test -f /var/lib/fps/.provision-complete >/dev/null 2>&1; then
      ok "Provision marker present on VM ${VMID}"
      return 0
    fi
    # qm guest exec returns JSON; also try a simple cat.
    if qm guest exec "${VMID}" -- cat /var/lib/fps/.provision-complete >/dev/null 2>&1; then
      ok "Provision marker present on VM ${VMID}"
      return 0
    fi
    sleep 10
  done
  warn "Timed out waiting for /var/lib/fps/.provision-complete."
  warn "Cloud-init may still be building. Check: qm guest exec ${VMID} -- tail -n 50 /var/log/fps-provision.log"
}

lxc_ipv4() {
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    if [[ "${IP_CIDR}" != "dhcp" ]]; then
      echo "${IP_CIDR%%/*}"
    else
      echo "10.0.0.10"
    fi
    return 0
  fi
  if [[ "${IP_CIDR}" != "dhcp" ]]; then
    echo "${IP_CIDR%%/*}"
    return 0
  fi
  pct exec "${VMID}" -- hostname -I 2>/dev/null | awk '{print $1}'
}

print_summary() {
  local ip="$1"
  printf '\n%b────────────────────────────────────────────────────────────%b\n' "${C_GREEN}" "${C_RESET}"
  printf '%bFPS guest is ready%b\n' "${C_BOLD}" "${C_RESET}"
  printf '  Role       %s\n' "${ROLE}"
  printf '  Guest      %s %s (%s)\n' "${GUEST_TYPE}" "${VMID}" "${HOSTNAME_GUEST}"
  printf '  Address    %s\n' "${ip}"
  if [[ "${ROLE}" == "control-plane" ]]; then
    printf '  Web UI     http://%s:47880\n' "${ip}"
    printf '  API        http://%s:47890\n' "${ip}"
    printf '  Node mTLS  %s:47891\n' "${ip}"
    printf '\nOpen the web UI and create the owner account.\n'
    printf 'Then run this installer on Homer with --role game-host.\n'
  else
    printf '  Docker     installed on this VM\n'
    printf '  Agent      /opt/fps/current/fps-node-agent\n'
    if [[ -z "${ENROLL_TOKEN}" ]]; then
      printf '\nEnroll from this VM after creating a token in the Fry UI:\n\n'
      printf '  fps-node-agent enroll --url http://FRY_IP:47890 --token TOKEN \\\n'
      printf '    --data-dir /var/lib/fps/agent --allow-insecure-http\n'
      printf '  systemctl enable --now fps-node-agent.service\n'
    fi
  fi
  if [[ "${GUEST_TYPE}" == "lxc" && -n "${PASSWORD}" && "${DRY_RUN}" -eq 0 ]]; then
    printf '\n  LXC root password was set (stored only in this session).\n'
  fi
  firewall_notes
  printf '%b────────────────────────────────────────────────────────────%b\n' "${C_GREEN}" "${C_RESET}"
}

main() {
  header
  need_root
  detect_proxmox
  pick_role
  apply_role_defaults
  prompt_missing
  if [[ "${PROVISION_ONLY}" -eq 0 ]]; then
    assert_vmid_free "${VMID}"
  fi
  local action="create and provision"
  if [[ "${PROVISION_ONLY}" -eq 1 ]]; then
    action="provision existing"
  fi
  local kind="LXC"
  [[ "${GUEST_TYPE}" == "vm" ]] && kind="QEMU VM"
  local summary
  summary="$(cat <<EOF
About to ${action} ${kind} ${VMID} (${HOSTNAME_GUEST})
  role           ${ROLE}
  cores/memory   ${CORES} / ${MEMORY} MiB
  disk/storage   ${DISK} GiB on ${DISK_STORAGE}
  network        ${BRIDGE}  ${IP_CIDR}
  source         ${FPS_GIT_URL:-https://github.com/${FPS_GIT_OWNER}/${FPS_GIT_REPO}.git} @ ${FPS_GIT_REF}

This builds FPS from source inside the guest (Rust, and Node/pnpm for the web UI).
Expect 15–40 minutes after the guest starts. Existing VMIDs are never overwritten.
EOF
)"
  confirm_or_die "${summary}"
  info "Building FPS from source inside the guest. Leave this shell open."

  if [[ "${PROVISION_ONLY}" -eq 1 ]]; then
    if [[ "${GUEST_TYPE}" == "lxc" ]]; then
      wait_lxc_network
      provision_lxc
      print_summary "$(lxc_ipv4)"
    else
      wait_vm_provision
      print_summary "${GUEST_IP:-$(guest_ipv4 "${VMID}" 2>/dev/null || echo unknown)}"
    fi
    return 0
  fi

  if [[ "${GUEST_TYPE}" == "lxc" ]]; then
    create_lxc
    wait_lxc_network
    provision_lxc
    print_summary "$(lxc_ipv4)"
  else
    create_vm
    wait_vm_provision
    print_summary "${GUEST_IP:-unknown}"
  fi
}

main
