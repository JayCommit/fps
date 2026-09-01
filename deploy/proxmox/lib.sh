#!/usr/bin/env bash
# Shared helpers for the FPS Proxmox installer. Sourced from install.sh.
# shellcheck shell=bash

FPS_GIT_OWNER="${FPS_GIT_OWNER:-JayCommit}"
FPS_GIT_REPO="${FPS_GIT_REPO:-fps}"
FPS_GIT_REF="${FPS_GIT_REF:-main}"
FPS_GIT_URL="${FPS_GIT_URL:-https://github.com/${FPS_GIT_OWNER}/${FPS_GIT_REPO}.git}"
FPS_RAW_BASE="${FPS_RAW_BASE:-https://raw.githubusercontent.com/${FPS_GIT_OWNER}/${FPS_GIT_REPO}/${FPS_GIT_REF}}"

C_RESET='\033[0m'
C_BOLD='\033[1m'
C_CYAN='\033[36m'
C_GREEN='\033[32m'
C_YELLOW='\033[33m'
C_RED='\033[31m'
C_DIM='\033[2m'

header() {
  printf '\n%b╔══════════════════════════════════════════════════════════════╗%b\n' "${C_CYAN}" "${C_RESET}" >&2
  printf '%b║%b  %bFPS installer for Proxmox VE%b                              %b║%b\n' "${C_CYAN}" "${C_RESET}" "${C_BOLD}" "${C_RESET}" "${C_CYAN}" "${C_RESET}" >&2
  printf '%b║%b  Creates the guest and builds FPS inside it.               %b║%b\n' "${C_CYAN}" "${C_RESET}" "${C_CYAN}" "${C_RESET}" >&2
  printf '%b╚══════════════════════════════════════════════════════════════╝%b\n\n' "${C_CYAN}" "${C_RESET}" >&2
}

info() { printf '%b➜%b %s\n' "${C_CYAN}" "${C_RESET}" "$*" >&2; }
ok() { printf '%b✓%b %s\n' "${C_GREEN}" "${C_RESET}" "$*" >&2; }
warn() { printf '%b!%b %s\n' "${C_YELLOW}" "${C_RESET}" "$*" >&2; }
die() {
  printf '%b✗%b %s\n' "${C_RED}" "${C_RESET}" "$*" >&2
  exit 1
}

run() {
  if [[ "${DRY_RUN:-0}" -eq 1 ]]; then
    printf '%b+%b %s\n' "${C_DIM}" "${C_RESET}" "$*" >&2
    return 0
  fi
  "$@"
}

normalize_role() {
  local raw
  raw="$(echo "$1" | tr '[:upper:]' '[:lower:]')"
  case "$raw" in
    1 | control-plane | control_plane | controlplane | web | panel | ui | api | fry)
      echo control-plane
      ;;
    2 | game-host | game_host | gamehost | node | agent | homer)
      echo game-host
      ;;
    *)
      die "unknown role: $1 (use control-plane or game-host)"
      ;;
  esac
}

need_root() {
  if [[ "${DRY_RUN:-0}" -eq 1 || "${ASSUME_PROXMOX:-0}" -eq 1 ]]; then
    return 0
  fi
  if [[ "${EUID}" -ne 0 ]]; then
    die "run this as root on the Proxmox host (or sudo -E bash … to keep FPS_GITHUB_TOKEN)"
  fi
}

detect_proxmox() {
  if [[ "${ASSUME_PROXMOX:-0}" -eq 1 ]]; then
    return 0
  fi
  if [[ -d /etc/pve ]] && command -v pct >/dev/null 2>&1 && command -v qm >/dev/null 2>&1; then
    return 0
  fi
  die "This installer must run on a Proxmox VE host (needs /etc/pve, pct, and qm).
Inside a guest, use the in-guest scripts under deploy/proxmox/guest-*.sh instead."
}

vmid_in_use() {
  local id="$1"
  if [[ -n "${EXISTING_VMIDS:-}" ]]; then
    [[ ",${EXISTING_VMIDS}," == *",${id},"* ]]
    return $?
  fi
  if [[ "${DRY_RUN:-0}" -eq 1 ]]; then
    return 1
  fi
  [[ -e "/etc/pve/lxc/${id}.conf" || -e "/etc/pve/qemu-server/${id}.conf" ]] && return 0
  if command -v pct >/dev/null 2>&1 && pct status "${id}" >/dev/null 2>&1; then
    return 0
  fi
  if command -v qm >/dev/null 2>&1 && qm status "${id}" >/dev/null 2>&1; then
    return 0
  fi
  return 1
}

assert_vmid_free() {
  local id="$1"
  if vmid_in_use "${id}"; then
    die "VMID ${id} already exists. FPS never overwrites an existing LXC or VM. Pick another ID."
  fi
}

# curl | bash has no TTY on stdin. Talk to the real terminal instead so menus work.
can_prompt() {
  if [[ "${FPS_FORCE_NO_TTY:-0}" == "1" ]]; then
    return 1
  fi
  if [[ -t 0 || -t 1 || -t 2 ]]; then
    return 0
  fi
  [[ -r /dev/tty && -w /dev/tty ]]
}

# Read operator input without consuming a script piped on stdin.
read_prompt() {
  local silent=0
  if [[ "${1:-}" == "-s" ]]; then
    silent=1
    shift
  fi
  local var="$1"
  if [[ -t 0 ]]; then
    if [[ "${silent}" -eq 1 ]]; then
      read -r -s "${var}" || true
    else
      read -r "${var}" || true
    fi
    return 0
  fi
  if [[ -r /dev/tty ]]; then
    if [[ "${silent}" -eq 1 ]]; then
      read -r -s "${var}" </dev/tty || true
    else
      read -r "${var}" </dev/tty || true
    fi
    return 0
  fi
  return 1
}

prompt_value() {
  # prompt_value VAR "Question" "default"
  local var="$1" msg="$2" default="$3"
  local current="${!var:-}"
  if [[ -n "${current}" ]]; then
    return 0
  fi
  if [[ "${YES:-0}" -eq 1 ]]; then
    printf -v "${var}" '%s' "${default}"
    return 0
  fi
  if ! can_prompt; then
    printf -v "${var}" '%s' "${default}"
    return 0
  fi
  local val=""
  printf '%s [%s]: ' "${msg}" "${default}" >&2
  read_prompt val || true
  printf -v "${var}" '%s' "${val:-${default}}"
}

prompt_secret() {
  local var="$1" msg="$2"
  local current="${!var:-}"
  if [[ -n "${current}" ]]; then
    return 0
  fi
  if [[ "${YES:-0}" -eq 1 ]] || ! can_prompt; then
    printf -v "${var}" '%s' ""
    return 0
  fi
  local val=""
  printf '%s (empty to generate): ' "${msg}" >&2
  read_prompt -s val || true
  printf '\n' >&2
  printf -v "${var}" '%s' "${val}"
}

github_get() {
  # github_get <path-in-repo>  → stdout
  local path="$1"
  local url="${FPS_RAW_BASE}/${path}"
  local args=(curl -fsSL --retry 3 --retry-delay 2)
  if [[ -n "${FPS_GITHUB_TOKEN:-}" ]]; then
    args+=(-H "Authorization: Bearer ${FPS_GITHUB_TOKEN}" -H "Accept: application/vnd.github.raw")
  fi
  "${args[@]}" "${url}"
}

clone_url_with_token() {
  local url="${FPS_GIT_URL}"
  if [[ -n "${FPS_GITHUB_TOKEN:-}" && "${url}" == https://github.com/* ]]; then
    url="https://x-access-token:${FPS_GITHUB_TOKEN}@github.com/${url#https://github.com/}"
  fi
  printf '%s' "${url}"
}

confirm_or_die() {
  local summary="$1"
  printf '\n%s\n\n' "${summary}"
  if [[ "${YES:-0}" -eq 1 ]]; then
    return 0
  fi
  if ! can_prompt; then
    die "no terminal for confirmation. Re-run on the Proxmox console, or pass --yes."
  fi
  local ans=""
  printf 'Type yes to create and fully provision this guest: ' >&2
  read_prompt ans || true
  [[ "${ans}" == "yes" ]] || die "aborted"
}

random_password() {
  openssl rand -base64 24 | tr -d '/+=' | head -c 28
}

default_template_storage() {
  if [[ "${DRY_RUN:-0}" -eq 1 || "${ASSUME_PROXMOX:-0}" -eq 1 ]]; then
    echo "${TEMPLATE_STORAGE:-local}"
    return 0
  fi
  if pvesm status >/dev/null 2>&1; then
    if pvesm status | awk 'NR>1 && $1=="local" {found=1} END{exit found?0:1}'; then
      echo local
      return 0
    fi
    pvesm status | awk 'NR==2 {print $1; exit}'
    return 0
  fi
  echo local
}

default_disk_storage() {
  if [[ "${DRY_RUN:-0}" -eq 1 || "${ASSUME_PROXMOX:-0}" -eq 1 ]]; then
    echo "${DISK_STORAGE:-local-lvm}"
    return 0
  fi
  if command -v pvesm >/dev/null 2>&1; then
    # Prefer a storage that can hold VM/LXC disks.
    local name
    name="$(pvesm status -content rootdir 2>/dev/null | awk 'NR==2 {print $1}')"
    if [[ -n "${name}" ]]; then
      echo "${name}"
      return 0
    fi
    name="$(pvesm status -content images 2>/dev/null | awk 'NR==2 {print $1}')"
    if [[ -n "${name}" ]]; then
      echo "${name}"
      return 0
    fi
  fi
  echo local-lvm
}

wait_for_guest_agent() {
  local vmid="$1"
  local tries="${2:-90}"
  local i
  if [[ "${DRY_RUN:-0}" -eq 1 ]]; then
    info "dry-run: would wait for qemu-guest-agent on VM ${vmid}"
    return 0
  fi
  info "Waiting for qemu-guest-agent on VM ${vmid}…"
  for ((i = 1; i <= tries; i++)); do
    if qm agent "${vmid}" ping >/dev/null 2>&1; then
      ok "guest agent is up"
      return 0
    fi
    sleep 2
  done
  die "qemu-guest-agent did not come up on VM ${vmid} within $((tries * 2))s"
}

guest_ipv4() {
  local vmid="$1"
  if [[ "${DRY_RUN:-0}" -eq 1 ]]; then
    echo "${GUEST_IP_OVERRIDE:-10.0.0.10}"
    return 0
  fi
  if [[ -n "${GUEST_IP_OVERRIDE:-}" ]]; then
    echo "${GUEST_IP_OVERRIDE}"
    return 0
  fi
  python3 - "${vmid}" <<'PY'
import json, subprocess, sys
vmid = sys.argv[1]
raw = subprocess.check_output(["qm", "guest", "cmd", vmid, "network-get-interfaces"], text=True)
data = json.loads(raw)
for nic in data:
    for ip in nic.get("ip-addresses") or []:
        addr = ip.get("ip-address") or ""
        if ip.get("ip-address-type") == "ipv4" and not addr.startswith("127."):
            print(addr)
            raise SystemExit(0)
raise SystemExit("no IPv4 from guest agent")
PY
}

firewall_notes() {
  cat <<'EOF'

OPNsense is not changed by this installer. Allow:

  • Administrators → Fry TCP 47880 (web UI) and TCP 47890 (API)
  • Homer → Fry TCP 47890 (API) and TCP 47891 (node mTLS)
  • Site-to-site WireGuard must already route the guest subnets
EOF
}
