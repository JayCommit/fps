#!/usr/bin/env bash
# FPS installer — run as root on a fresh Ubuntu or Debian machine you already created
# (VPS, dedicated, Proxmox guest, AWS, Azure, …). This script does not create VMs.
#
#   bash <(curl -fsSL https://raw.githubusercontent.com/JayCommit/fps/main/deploy/install.sh)
#
# Already cloned:
#   sudo bash deploy/install.sh
#
# Unattended:
#   sudo bash deploy/install.sh --role control-plane --yes
# Reconfigure an existing install (IP, database, HTTP) without rebuilding:
#   sudo bash deploy/install.sh --reconfigure --public-host NEW_IP
#
# Prompts read /dev/tty, so curl | bash still shows the menu.
set -euo pipefail

FPS_GIT_OWNER="${FPS_GIT_OWNER:-JayCommit}"
FPS_GIT_REPO="${FPS_GIT_REPO:-fps}"
FPS_GIT_REF="${FPS_GIT_REF:-main}"
FPS_GIT_URL="${FPS_GIT_URL:-https://github.com/${FPS_GIT_OWNER}/${FPS_GIT_REPO}.git}"
FPS_RAW_BASE="${FPS_RAW_BASE:-https://raw.githubusercontent.com/${FPS_GIT_OWNER}/${FPS_GIT_REPO}/${FPS_GIT_REF}}"
FPS_PREFIX="${FPS_PREFIX:-/opt/fps}"
FPS_WEB_ROOT="${FPS_WEB_ROOT:-/opt/fps/web}"
FPS_DATA_DIR="${FPS_DATA_DIR:-/var/lib/fps}"
FPS_AGENT_DIR="${FPS_AGENT_DIR:-/var/lib/fps/agent}"
FPS_HTTP_BIND="${FPS_HTTP_BIND:-0.0.0.0:47890}"
FPS_WEB_BIND="${FPS_WEB_BIND:-0.0.0.0:47880}"
FPS_NODE_BIND="${FPS_NODE_BIND:-0.0.0.0:47891}"
FPS_RUST_TOOLCHAIN="${FPS_RUST_TOOLCHAIN:-1.98.0}"
FPS_NODE_MAJOR="${FPS_NODE_MAJOR:-22}"
FPS_PNPM_VERSION="${FPS_PNPM_VERSION:-10.14.0}"

ROLE="${FPS_ROLE:-}"
YES=0
DRY_RUN=0
ASSUME_ROOT=0
SKIP_BUILD="${FPS_SKIP_BUILD:-0}"
SKIP_CLONE="${FPS_SKIP_CLONE:-0}"
SKIP_PACKAGES="${FPS_SKIP_PACKAGES:-0}"
SKIP_START=0
START=1
REFRESH=0
FORCE_ENV=0
INSTALL_MARIADB=1
RECONFIGURE=0
INSTALL_MODE=""
EXISTING_INSTALL=0
DB_HOST="${FPS_DB_HOST:-127.0.0.1}"
DB_PORT="${FPS_DB_PORT:-3306}"
DB_NAME="${FPS_DB_NAME:-fps}"
DB_USER="${FPS_DB_USER:-fps}"
FPS_DATABASE_URL="${FPS_DATABASE_URL:-}"
ALLOW_INSECURE="${FPS_ALLOW_INSECURE_HTTP:-true}"
PUBLIC_HOST="${FPS_PUBLIC_HOST:-}"
CONTROL_PLANE_URL="${FPS_CONTROL_PLANE_URL:-}"
ENROLL_TOKEN="${FPS_ENROLL_TOKEN:-}"
OS_RELEASE_FILE="/etc/os-release"
SRC_DIR="${FPS_SRC_DIR:-}"
LOG_FILE="${FPS_INSTALL_LOG:-}"
SELF_DIR=""
REPO_ROOT=""
OS_ID=""
OS_VERSION_ID=""
OS_CODENAME=""
OS_PRETTY=""
OS_ARCH=""
STEP_INDEX=0
STEP_TOTAL=0
LOG=""

usage() {
  cat <<'EOF'
Usage: install.sh [options]

Install FPS on this Ubuntu or Debian machine (fresh OS → running).
The operator creates the VM / VPS / dedicated server; this script does not.

  --role ROLE             control-plane | game-host | both
                          aliases: web/panel, node/agent, all/lab
  --yes                   unattended: accept defaults, no prompts
  --interactive           force prompts even when --yes would apply
  --dry-run               print what would run; do not mutate
  --public-host HOST      hostname or IP used in URLs (default: first IPv4)
  --database-url URL      control-plane MariaDB URL (implies no local server)
  --db-host HOST          remote MariaDB host (default 127.0.0.1)
  --db-port PORT          remote MariaDB port (default 3306)
  --db-name NAME          database name (default fps)
  --db-user USER          database user (default fps)
  --reconfigure           existing install: change IP/database/HTTP, skip rebuild
  --control-plane-url URL game-host: enroll URL (optional)
  --enroll-token TOKEN    game-host: enroll immediately (optional)
  --skip-build            do not cargo/pnpm build (use existing binaries)
  --skip-clone            do not git clone (use --source-dir or this repo)
  --skip-packages         do not apt-get (tests / already provisioned)
  --skip-start            write units and env; do not systemctl enable --now
  --refresh               re-clone /opt/fps/src even if it exists
  --force-env             overwrite existing /etc/fps/*.env
  --no-mariadb            control-plane: do not install/configure local MariaDB
  --source-dir DIR        existing fps git checkout
  --prefix DIR            binary prefix (default /opt/fps)
  --os-release-file FILE  override /etc/os-release (tests)
  --assume-root           skip EUID check (tests)
  --log-file FILE         install log (default /var/log/fps-install.log)
  --help                  this text

Environment:
  FPS_GIT_URL / FPS_GIT_REF
  FPS_DATABASE_URL        full mysql:// URL (remote or local)
  FPS_DB_PASSWORD         MariaDB password (generated if omitted)
  FPS_MASTER_KEY          control-plane master key (generated if omitted)
  FPS_TEST_ANSWERS        comma-separated prompt answers (tests)
  FPS_FORCE_NO_TTY=1      pretend there is no terminal (tests)

Unattended example:

  sudo bash deploy/install.sh --role control-plane --yes

Interactive example (menu + y/n):

  sudo bash deploy/install.sh
EOF
}

INTERACTIVE_FORCE=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --role) ROLE="${2:?}"; shift 2 ;;
    --yes|--non-interactive) YES=1; shift ;;
    --interactive) INTERACTIVE_FORCE=1; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    --public-host) PUBLIC_HOST="${2:?}"; shift 2 ;;
    --database-url) FPS_DATABASE_URL="${2:?}"; INSTALL_MARIADB=0; shift 2 ;;
    --db-host) DB_HOST="${2:?}"; INSTALL_MARIADB=0; shift 2 ;;
    --db-port) DB_PORT="${2:?}"; shift 2 ;;
    --db-name) DB_NAME="${2:?}"; shift 2 ;;
    --db-user) DB_USER="${2:?}"; shift 2 ;;
    --reconfigure) RECONFIGURE=1; INSTALL_MODE=reconfigure; shift ;;
    --control-plane-url) CONTROL_PLANE_URL="${2:?}"; shift 2 ;;
    --enroll-token) ENROLL_TOKEN="${2:?}"; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    --skip-clone) SKIP_CLONE=1; shift ;;
    --skip-packages) SKIP_PACKAGES=1; shift ;;
    --skip-start) SKIP_START=1; START=0; shift ;;
    --refresh) REFRESH=1; shift ;;
    --force-env) FORCE_ENV=1; shift ;;
    --no-mariadb) INSTALL_MARIADB=0; shift ;;
    --source-dir) SRC_DIR="${2:?}"; shift 2 ;;
    --prefix) FPS_PREFIX="${2:?}"; FPS_WEB_ROOT="${FPS_PREFIX}/web"; shift 2 ;;
    --os-release-file) OS_RELEASE_FILE="${2:?}"; shift 2 ;;
    --assume-root) ASSUME_ROOT=1; shift ;;
    --log-file) LOG_FILE="${2:?}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ "${INTERACTIVE_FORCE}" -eq 1 ]]; then
  YES=0
fi

# --- colour / TTY -----------------------------------------------------------

C_RESET='' C_BOLD='' C_DIM='' C_CYAN='' C_GREEN='' C_YELLOW='' C_RED='' C_MAGENTA='' C_WHITE=''
if [[ -z "${NO_COLOR:-}" && "${TERM:-dumb}" != "dumb" ]]; then
  if [[ -t 1 || -t 2 || -r /dev/tty ]]; then
    C_RESET='\033[0m'
    C_BOLD='\033[1m'
    C_DIM='\033[2m'
    C_CYAN='\033[36m'
    C_GREEN='\033[32m'
    C_YELLOW='\033[33m'
    C_RED='\033[31m'
    C_MAGENTA='\033[35m'
    C_WHITE='\033[97m'
  fi
fi

can_prompt() {
  if [[ "${FPS_FORCE_NO_TTY:-0}" == "1" ]]; then
    return 1
  fi
  if [[ -n "${FPS_TEST_ANSWERS:-}" ]]; then
    return 0
  fi
  if [[ -t 0 || -t 1 || -t 2 ]]; then
    return 0
  fi
  [[ -r /dev/tty && -w /dev/tty ]]
}

is_tty() {
  [[ -t 2 || -w /dev/tty ]] && [[ "${FPS_FORCE_NO_TTY:-0}" != "1" ]]
}

IFS=',' read -r -a ANSWERS_LEFT <<<"${FPS_TEST_ANSWERS:-}"

read_prompt() {
  local silent=0
  if [[ "${1:-}" == "-s" ]]; then
    silent=1
    shift
  fi
  local var="$1"
  if [[ ${#ANSWERS_LEFT[@]} -gt 0 ]]; then
    local ans="${ANSWERS_LEFT[0]}"
    ANSWERS_LEFT=("${ANSWERS_LEFT[@]:1}")
    printf -v "${var}" '%s' "${ans}"
    if [[ "${silent}" -eq 1 ]]; then
      printf '\n' >&2
    else
      printf '%s\n' "${ans}" >&2
    fi
    return 0
  fi
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

die() {
  printf '%b✗%b %s\n' "${C_RED}" "${C_RESET}" "$*" >&2
  exit 1
}

info() { printf '%b➜%b %s\n' "${C_CYAN}" "${C_RESET}" "$*" >&2; }
ok() { printf '%b✓%b %s\n' "${C_GREEN}" "${C_RESET}" "$*" >&2; }
warn() { printf '%b!%b %s\n' "${C_YELLOW}" "${C_RESET}" "$*" >&2; }

header() {
  printf '\n' >&2
  printf '%b╭──────────────────────────────────────────────────────────────╮%b\n' "${C_CYAN}" "${C_RESET}" >&2
  printf '%b│%b  %bFPS%b                                                        %b│%b\n' "${C_CYAN}" "${C_RESET}" "${C_BOLD}${C_WHITE}" "${C_RESET}" "${C_CYAN}" "${C_RESET}" >&2
  printf '%b│%b  Game servers on machines you own                            %b│%b\n' "${C_CYAN}" "${C_RESET}" "${C_CYAN}" "${C_RESET}" >&2
  printf '%b│%b  Ubuntu / Debian installer · control plane and game hosts    %b│%b\n' "${C_CYAN}" "${C_RESET}" "${C_CYAN}" "${C_RESET}" >&2
  printf '%b╰──────────────────────────────────────────────────────────────╯%b\n\n' "${C_CYAN}" "${C_RESET}" >&2
}

progress_bar() {
  local current="$1" total="$2"
  local width=28
  local filled=0 empty=0 i
  if [[ "${total}" -le 0 ]]; then
    total=1
  fi
  filled=$((current * width / total))
  if [[ "${filled}" -gt "${width}" ]]; then
    filled="${width}"
  fi
  empty=$((width - filled))
  printf '%b[' "${C_CYAN}" >&2
  for ((i = 0; i < filled; i++)); do printf '█' >&2; done
  for ((i = 0; i < empty; i++)); do printf '%b░%b' "${C_DIM}" "${C_CYAN}" >&2; done
  printf ']%b  %d/%d\n' "${C_RESET}" "${current}" "${total}" >&2
}

begin_step() {
  STEP_INDEX=$((STEP_INDEX + 1))
  printf '\n' >&2
  progress_bar "${STEP_INDEX}" "${STEP_TOTAL}"
  printf '%b▸%b %s\n' "${C_MAGENTA}" "${C_RESET}" "$*" >&2
}

run() {
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    printf '%b+%b %s\n' "${C_DIM}" "${C_RESET}" "$*" >&2
    return 0
  fi
  "$@"
}

log_init() {
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    LOG="/dev/null"
    return 0
  fi
  if [[ -n "${LOG_FILE}" ]]; then
    LOG="${LOG_FILE}"
  elif [[ "${EUID}" -eq 0 ]]; then
    LOG="/var/log/fps-install.log"
  else
    LOG="${TMPDIR:-/tmp}/fps-install.log"
  fi
  mkdir -p "$(dirname "${LOG}")" 2>/dev/null || true
  : >>"${LOG}" 2>/dev/null || LOG="${TMPDIR:-/tmp}/fps-install.log"
  printf '\n===== FPS install %s =====\n' "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" >>"${LOG}"
}

show_log_tail() {
  if [[ -z "${LOG:-}" || "${LOG}" == "/dev/null" ]]; then
    return 0
  fi
  warn "Last 40 lines of ${LOG}:"
  tail -n 40 "${LOG}" >&2 || true
}

# Run a command with stdin attached to /dev/null (so curl|bash cannot be
# consumed by apt-get) and stdout/stderr appended to the install log.
# On failure, print the log tail and die instead of exiting silently via set -e.
log_cmd() {
  local what="$1"
  shift
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    printf '%b+%b %s\n' "${C_DIM}" "${C_RESET}" "$*" >&2
    return 0
  fi
  if "$@" </dev/null >>"${LOG}" 2>&1; then
    return 0
  fi
  show_log_tail
  die "${what} failed. See ${LOG}"
}

run_logged() {
  local title="$1"
  shift
  begin_step "${title}"
  log_cmd "${title}" "$@"
  ok "${title}"
}

normalize_role() {
  local raw
  raw="$(echo "$1" | tr '[:upper:]' '[:lower:]')"
  case "${raw}" in
    1 | control-plane | control_plane | controlplane | web | panel | ui | api | fry)
      echo control-plane
      ;;
    2 | game-host | game_host | gamehost | node | agent | homer)
      echo game-host
      ;;
    3 | both | all | lab)
      echo both
      ;;
    *)
      die "unknown role: $1 (use control-plane, game-host, or both)"
      ;;
  esac
}

role_has_cp() {
  [[ "${ROLE}" == "control-plane" || "${ROLE}" == "both" ]]
}

role_has_gh() {
  [[ "${ROLE}" == "game-host" || "${ROLE}" == "both" ]]
}

resolve_self() {
  local src="${BASH_SOURCE[0]:-}"
  if [[ -n "${src}" && -f "${src}" && "${src}" != /dev/fd/* && "${src}" != /proc/self/fd/* ]]; then
    SELF_DIR="$(cd "$(dirname "${src}")" && pwd)"
    if [[ -f "${SELF_DIR}/../Cargo.toml" && -d "${SELF_DIR}/../services" ]]; then
      REPO_ROOT="$(cd "${SELF_DIR}/.." && pwd)"
    elif [[ -f "${SELF_DIR}/Cargo.toml" && -d "${SELF_DIR}/services" ]]; then
      REPO_ROOT="${SELF_DIR}"
    fi
  fi
}

need_root() {
  if [[ "${DRY_RUN}" -eq 1 || "${ASSUME_ROOT}" -eq 1 ]]; then
    return 0
  fi
  if [[ "${EUID}" -ne 0 ]]; then
    die "run as root (sudo bash …)"
  fi
}

load_os() {
  if [[ ! -f "${OS_RELEASE_FILE}" ]]; then
    die "cannot read ${OS_RELEASE_FILE}"
  fi
  # shellcheck disable=SC1090
  OS_ID="$(. "${OS_RELEASE_FILE}" && echo "${ID:-}")"
  OS_VERSION_ID="$(. "${OS_RELEASE_FILE}" && echo "${VERSION_ID:-}")"
  OS_CODENAME="$(. "${OS_RELEASE_FILE}" && echo "${VERSION_CODENAME:-}")"
  OS_PRETTY="$(. "${OS_RELEASE_FILE}" && echo "${PRETTY_NAME:-${ID:-unknown}}")"
  OS_ARCH="$(dpkg --print-architecture 2>/dev/null || true)"
  if [[ -z "${OS_ARCH}" ]]; then
    case "$(uname -m)" in
      x86_64) OS_ARCH=amd64 ;;
      aarch64 | arm64) OS_ARCH=arm64 ;;
      *) OS_ARCH="$(uname -m)" ;;
    esac
  fi
  OS_ID="$(echo "${OS_ID}" | tr '[:upper:]' '[:lower:]')"
  case "${OS_ID}" in
    ubuntu | debian) ;;
    *)
      die "this installer supports Ubuntu and Debian only (found ID=${OS_ID:-unknown} in ${OS_RELEASE_FILE}).
Install Ubuntu 22.04+ or Debian 12+ on this VM/VPS, then re-run."
      ;;
  esac
  local major="${OS_VERSION_ID%%.*}"
  major="${major:-0}"
  if [[ "${OS_ID}" == "ubuntu" && "${major}" -lt 20 ]]; then
    die "Ubuntu ${OS_VERSION_ID} is too old. Use Ubuntu 22.04 LTS or newer."
  fi
  if [[ "${OS_ID}" == "debian" && "${major}" -lt 11 ]]; then
    die "Debian ${OS_VERSION_ID} is too old. Use Debian 12 or newer."
  fi
  if [[ "${OS_ID}" == "ubuntu" && "${major}" -lt 22 ]]; then
    warn "Ubuntu ${OS_VERSION_ID} is untested; 22.04 LTS or 24.04 LTS is recommended."
  fi
  if [[ "${OS_ID}" == "ubuntu" && "${major}" -ge 26 ]]; then
    info "Ubuntu ${OS_VERSION_ID}: Docker Engine will use the noble (24.04) apt pocket if ${OS_CODENAME} is not published yet."
  fi
  if [[ "${OS_ID}" == "debian" && "${major}" -lt 12 ]]; then
    warn "Debian ${OS_VERSION_ID} is untested; Debian 12+ is recommended."
  fi
  case "${OS_ARCH}" in
    amd64 | arm64 | aarch64 | x86_64) ;;
    *)
      die "unsupported architecture ${OS_ARCH} (need amd64 or arm64)"
      ;;
  esac
}

is_lxc() {
  if grep -qa 'container=lxc' /proc/1/environ 2>/dev/null; then
    return 0
  fi
  if [[ "$(cat /run/systemd/container 2>/dev/null || true)" == "lxc" ]]; then
    return 0
  fi
  return 1
}

public_host() {
  if [[ -n "${PUBLIC_HOST}" ]]; then
    echo "${PUBLIC_HOST}"
    return 0
  fi
  if [[ -n "${FPS_PUBLIC_IP:-}" ]]; then
    echo "${FPS_PUBLIC_IP}"
    return 0
  fi
  hostname -I 2>/dev/null | awk '{print $1}'
}

ask_value() {
  local var="$1" msg="$2" default="$3"
  local current="${!var:-}"
  if [[ -n "${current}" ]]; then
    return 0
  fi
  if [[ "${YES}" -eq 1 ]] || ! can_prompt; then
    printf -v "${var}" '%s' "${default}"
    return 0
  fi
  local val=""
  printf '%b?%b %s %b[%s]%b: ' "${C_CYAN}" "${C_RESET}" "${msg}" "${C_DIM}" "${default}" "${C_RESET}" >&2
  read_prompt val || true
  printf -v "${var}" '%s' "${val:-${default}}"
}

ask_yn() {
  # ask_yn VAR "question" default_y_or_n
  local var="$1" msg="$2" default="$3"
  local current="${!var:-}"
  if [[ "${current}" == "0" || "${current}" == "1" ]]; then
    if [[ "${YES}" -eq 1 ]] || ! can_prompt; then
      return 0
    fi
  elif [[ -n "${current}" ]]; then
    return 0
  fi
  local hint="y/N"
  [[ "${default}" == "y" || "${default}" == "Y" ]] && hint="Y/n"
  if [[ "${YES}" -eq 1 ]] || ! can_prompt; then
    if [[ "${default}" == "y" || "${default}" == "Y" ]]; then
      printf -v "${var}" '%s' "1"
    else
      printf -v "${var}" '%s' "0"
    fi
    return 0
  fi
  local val=""
  printf '%b?%b %s %b[%s]%b: ' "${C_CYAN}" "${C_RESET}" "${msg}" "${C_DIM}" "${hint}" "${C_RESET}" >&2
  read_prompt val || true
  val="$(echo "${val:-${default}}" | tr '[:upper:]' '[:lower:]')"
  case "${val}" in
    y | yes | 1) printf -v "${var}" '%s' "1" ;;
    n | no | 0) printf -v "${var}" '%s' "0" ;;
    *)
      if [[ "${default}" == "y" || "${default}" == "Y" ]]; then
        printf -v "${var}" '%s' "1"
      else
        printf -v "${var}" '%s' "0"
      fi
      ;;
  esac
}

ask_secret() {
  local var="$1" msg="$2"
  local current="${!var:-}"
  if [[ -n "${current}" ]]; then
    return 0
  fi
  if [[ "${YES}" -eq 1 ]] || ! can_prompt; then
    printf -v "${var}" '%s' ""
    return 0
  fi
  local val=""
  printf '%b?%b %s: ' "${C_CYAN}" "${C_RESET}" "${msg}" >&2
  read_prompt -s val || true
  printf '\n' >&2
  printf -v "${var}" '%s' "${val}"
}

pick_role() {
  if [[ -n "${ROLE}" ]]; then
    ROLE="$(normalize_role "${ROLE}")"
    return 0
  fi
  if [[ "${YES}" -eq 1 ]]; then
    die "unattended install requires --role control-plane, --role game-host, or --role both."
  fi
  if ! can_prompt; then
    die "no terminal to show the role menu. Re-run from a shell, or pass --role control-plane, --role game-host, or --role both."
  fi

  local choice=""
  if command -v whiptail >/dev/null 2>&1 && [[ -z "${FPS_TEST_ANSWERS:-}" ]] && [[ -r /dev/tty ]]; then
    choice="$(
      whiptail --backtitle "FPS" --title "FPS installer" \
        --menu "What should this machine be?" 20 78 3 \
        "1" "Control plane  — Web UI + API + database" \
        "2" "Game host      — Docker + node agent" \
        "3" "Both           — Single-box lab" \
        3>&1 1>&2 2>&3 </dev/tty
    )" || die "aborted"
  else
    cat <<EOF >&2

${C_BOLD}What should this machine be?${C_RESET}

  ${C_CYAN}1)${C_RESET}  ${C_BOLD}Control plane${C_RESET}
      Web UI, API, and database. Operators log into this machine.

  ${C_CYAN}2)${C_RESET}  ${C_BOLD}Game host${C_RESET}
      Docker Engine + node agent. This is where games actually run.

  ${C_CYAN}3)${C_RESET}  ${C_BOLD}Both${C_RESET}
      Single-box lab (one VPS). Fine for testing, not the usual split.

EOF
    printf '%b?%b Select 1, 2, or 3: ' "${C_CYAN}" "${C_RESET}" >&2
    read_prompt choice || true
  fi
  [[ -n "${choice}" ]] || die "no role selected"
  ROLE="$(normalize_role "${choice}")"
}

read_env_key() {
  local file="$1" key="$2"
  if [[ ! -f "${file}" ]]; then
    return 0
  fi
  awk -F= -v k="${key}" '$1 == k { sub(/^[^=]+=/, ""); print; exit }' "${file}"
}

redact_db_url() {
  local url="$1"
  if [[ -z "${url}" ]]; then
    echo "(unset)"
    return 0
  fi
  echo "${url}" | sed -E 's#://([^:/@]+):[^@]*@#://\1:***@#'
}

urlencode() {
  python3 -c 'import urllib.parse,sys; print(urllib.parse.quote(sys.argv[1], safe=""))' "$1"
}

build_database_url() {
  if [[ -n "${FPS_DATABASE_URL}" ]]; then
    return 0
  fi
  local pass="${FPS_DB_PASSWORD:-}"
  local pass_enc=""
  if [[ -n "${pass}" ]]; then
    if command -v python3 >/dev/null 2>&1; then
      pass_enc="$(urlencode "${pass}")"
    else
      pass_enc="${pass}"
    fi
    FPS_DATABASE_URL="mysql://${DB_USER}:${pass_enc}@${DB_HOST}:${DB_PORT}/${DB_NAME}"
  else
    FPS_DATABASE_URL="mysql://${DB_USER}@${DB_HOST}:${DB_PORT}/${DB_NAME}"
  fi
}

upsert_env_key() {
  local file="$1" key="$2" value="$3"
  if [[ ! -f "${file}" ]]; then
    printf '%s=%s\n' "${key}" "${value}" >"${file}"
    return 0
  fi
  if grep -q "^${key}=" "${file}"; then
    local tmp
    tmp="$(mktemp)"
    awk -F= -v k="${key}" -v v="${value}" 'BEGIN{OFS="="} $1==k {$0=k"="v} {print}' "${file}" >"${tmp}"
    cat "${tmp}" >"${file}"
    rm -f "${tmp}"
  else
    printf '%s=%s\n' "${key}" "${value}" >>"${file}"
  fi
}

detect_existing_install() {
  EXISTING_INSTALL=0
  if [[ "${ASSUME_ROOT}" -eq 1 && "${RECONFIGURE}" -eq 0 ]]; then
    return 1
  fi
  if [[ -f /etc/fps/control-plane.env || -f /etc/fps/node-agent.env || -f "${FPS_DATA_DIR}/.provision-complete" ]]; then
    EXISTING_INSTALL=1
    return 0
  fi
  return 1
}

infer_installed_role() {
  local cp=0 gh=0
  if [[ -f /etc/fps/control-plane.env ]] || systemctl list-unit-files fps-control-plane.service >/dev/null 2>&1; then
    if [[ -f /etc/fps/control-plane.env ]]; then
      cp=1
    fi
  fi
  if [[ -f /etc/fps/node-agent.env ]]; then
    gh=1
  fi
  if [[ "${cp}" -eq 1 && "${gh}" -eq 1 ]]; then
    echo both
  elif [[ "${gh}" -eq 1 ]]; then
    echo game-host
  else
    echo control-plane
  fi
}

show_existing_banner() {
  local host db role_guess
  host="$(read_env_key /etc/fps/control-plane.env FPS_PUBLIC_URL)"
  db="$(redact_db_url "$(read_env_key /etc/fps/control-plane.env FPS_DATABASE_URL)")"
  role_guess="$(infer_installed_role)"
  printf '%bFPS is already installed on this machine%b\n' "${C_BOLD}${C_GREEN}" "${C_RESET}" >&2
  printf '  Role          %s\n' "${role_guess}" >&2
  printf '  Public URL    %s\n' "${host:-unknown}" >&2
  printf '  Database      %s\n' "${db}" >&2
  if [[ -f /etc/fps/node-agent.env ]]; then
    printf '  Agent env     /etc/fps/node-agent.env\n' >&2
  fi
  printf '\n' >&2
}

pick_install_mode() {
  if [[ -n "${INSTALL_MODE}" ]]; then
    return 0
  fi
  if ! detect_existing_install; then
    INSTALL_MODE=fresh
    return 0
  fi
  show_existing_banner
  if [[ "${YES}" -eq 1 ]]; then
    INSTALL_MODE=upgrade
    info "Existing install: upgrade/rebuild. Pass --reconfigure to change IP/database without rebuilding."
    return 0
  fi
  if ! can_prompt; then
    INSTALL_MODE=upgrade
    return 0
  fi
  cat <<EOF >&2
${C_BOLD}What do you want to do?${C_RESET}

  ${C_CYAN}1)${C_RESET}  ${C_BOLD}Reconfigure${C_RESET}
      Change public IP, database URL, and HTTP settings. Skip the long rebuild.

  ${C_CYAN}2)${C_RESET}  ${C_BOLD}Upgrade${C_RESET}
      Rebuild from source and keep existing secrets unless you pass --force-env.

  ${C_CYAN}3)${C_RESET}  ${C_BOLD}Repair${C_RESET}
      Rewrite systemd units and restart services. No cargo/pnpm build.

EOF
  local choice=""
  printf '%b?%b Select 1, 2, or 3 [1]: ' "${C_CYAN}" "${C_RESET}" >&2
  read_prompt choice || true
  choice="${choice:-1}"
  case "${choice}" in
    1 | reconfigure | r) INSTALL_MODE=reconfigure ;;
    2 | upgrade | u) INSTALL_MODE=upgrade ;;
    3 | repair) INSTALL_MODE=repair ;;
    *) INSTALL_MODE=reconfigure ;;
  esac
}

apply_install_mode() {
  case "${INSTALL_MODE}" in
    reconfigure | repair)
      SKIP_PACKAGES=1
      SKIP_BUILD=1
      SKIP_CLONE=1
      if [[ -z "${ROLE}" ]]; then
        ROLE="$(infer_installed_role)"
      fi
      ;;
  esac
}

prompt_remote_db() {
  if [[ -n "${FPS_DATABASE_URL}" ]]; then
    INSTALL_MARIADB=0
    return 0
  fi
  ask_value DB_HOST "MariaDB host" "${DB_HOST}"
  ask_value DB_PORT "MariaDB port" "${DB_PORT}"
  ask_value DB_NAME "Database name" "${DB_NAME}"
  ask_value DB_USER "Database user" "${DB_USER}"
  if [[ -z "${FPS_DB_PASSWORD:-}" ]]; then
    ask_secret FPS_DB_PASSWORD "Database password"
  fi
  build_database_url
}

prompt_missing() {
  local default_host
  default_host="$(public_host)"
  default_host="${default_host:-127.0.0.1}"
  if [[ -z "${PUBLIC_HOST}" && -f /etc/fps/control-plane.env ]]; then
    local cur
    cur="$(read_env_key /etc/fps/control-plane.env FPS_PUBLIC_URL)"
    cur="${cur#http://}"
    cur="${cur#https://}"
    cur="${cur%%[:/]*}"
    if [[ -n "${cur}" ]]; then
      default_host="${cur}"
    fi
  fi
  ask_value PUBLIC_HOST "Public hostname or IP for URLs" "${default_host}"

  if role_has_cp; then
    local mariadb_ans="${INSTALL_MARIADB}"
    local mariadb_default="y"
    [[ "${INSTALL_MARIADB}" -eq 0 || -n "${FPS_DATABASE_URL}" ]] && mariadb_default="n"
    if [[ "${INSTALL_MODE}" == "reconfigure" ]]; then
      ask_yn mariadb_ans "Keep/use MariaDB on this machine? (n = remote database)" "${mariadb_default}"
    else
      ask_yn mariadb_ans "Install and configure MariaDB on this machine?" "${mariadb_default}"
    fi
    INSTALL_MARIADB="${mariadb_ans}"
    if [[ "${INSTALL_MARIADB}" -eq 0 ]]; then
      prompt_remote_db
    fi
  else
    INSTALL_MARIADB=0
  fi

  local insecure_default="y"
  [[ "${ALLOW_INSECURE}" == "false" || "${ALLOW_INSECURE}" == "0" ]] && insecure_default="n"
  local insecure_ans=""
  ask_yn insecure_ans "Allow unencrypted HTTP? (alpha / private LAN)" "${insecure_default}"
  if [[ "${insecure_ans}" -eq 1 ]]; then
    ALLOW_INSECURE="true"
  else
    ALLOW_INSECURE="false"
  fi

  if role_has_gh; then
    local enroll_ans="n"
    if [[ -n "${ENROLL_TOKEN}" && -n "${CONTROL_PLANE_URL}" ]]; then
      enroll_ans="y"
    fi
    local want_enroll=""
    ask_yn want_enroll "Enroll this game host with a control plane now?" "${enroll_ans}"
    if [[ "${want_enroll}" -eq 1 ]]; then
      ask_value CONTROL_PLANE_URL "Control plane URL (http://HOST:47890)" "${CONTROL_PLANE_URL:-}"
      [[ -n "${CONTROL_PLANE_URL}" ]] || die "control plane URL is required to enroll"
      ask_secret ENROLL_TOKEN "Enrollment token"
      [[ -n "${ENROLL_TOKEN}" ]] || die "enrollment token is required to enroll"
    else
      ENROLL_TOKEN=""
    fi
  fi

  local start_default="y"
  [[ "${SKIP_START}" -eq 1 ]] && start_default="n"
  if role_has_gh && ! role_has_cp && [[ -z "${ENROLL_TOKEN}" ]]; then
    start_default="n"
  fi
  local start_ans=""
  ask_yn start_ans "Start systemd services when the install finishes?" "${start_default}"
  START="${start_ans}"
}

confirm_plan() {
  local host web api
  host="${PUBLIC_HOST:-$(public_host)}"
  host="${host:-127.0.0.1}"
  web="http://${host}:47880"
  api="http://${host}:47890"

  printf '\n%b──────────────────────────────────────────────────────────────%b\n' "${C_CYAN}" "${C_RESET}" >&2
  printf '%bInstall plan%b\n' "${C_BOLD}" "${C_RESET}" >&2
  printf '  Mode          %s\n' "${INSTALL_MODE:-fresh}" >&2
  printf '  OS            %s (%s) %s\n' "${OS_PRETTY}" "${OS_CODENAME}" "${OS_ARCH}" >&2
  printf '  Role          %s\n' "${ROLE}" >&2
  if [[ "${INSTALL_MODE}" != "reconfigure" && "${INSTALL_MODE}" != "repair" ]]; then
    printf '  Source        %s\n' "${SRC_DIR:-${FPS_GIT_URL} @ ${FPS_GIT_REF}}" >&2
  fi
  if role_has_cp; then
    printf '  Web UI        %s\n' "${web}" >&2
    printf '  API           %s\n' "${api}" >&2
    if [[ "${INSTALL_MARIADB}" -eq 1 ]]; then
      printf '  MariaDB       local on this machine\n' >&2
    elif [[ -n "${FPS_DATABASE_URL}" ]]; then
      printf '  MariaDB       remote %s\n' "$(redact_db_url "${FPS_DATABASE_URL}")" >&2
    else
      printf '  MariaDB       remote %s:%s/%s (user %s)\n' "${DB_HOST}" "${DB_PORT}" "${DB_NAME}" "${DB_USER}" >&2
    fi
  fi
  if role_has_gh; then
    printf '  Docker        install Engine from docker.com/%s\n' "${OS_ID}" >&2
    if [[ -n "${ENROLL_TOKEN}" ]]; then
      printf '  Enroll        %s\n' "${CONTROL_PLANE_URL}" >&2
    else
      printf '  Enroll        later (token from the panel)\n' >&2
    fi
  fi
  printf '  HTTP TLS      ALLOW_INSECURE=%s\n' "${ALLOW_INSECURE}" >&2
  printf '  Start units   %s\n' "$([[ "${START}" -eq 1 ]] && echo yes || echo no)" >&2
  if [[ "${SKIP_BUILD}" -eq 0 ]]; then
    printf '\n  Builds from source (Rust %s' "${FPS_RUST_TOOLCHAIN}" >&2
    if role_has_cp; then
      printf ', Node %s, pnpm %s' "${FPS_NODE_MAJOR}" "${FPS_PNPM_VERSION}" >&2
    fi
    printf '). Expect 15–40 minutes.\n' >&2
  fi
  printf '%b──────────────────────────────────────────────────────────────%b\n\n' "${C_CYAN}" "${C_RESET}" >&2

  if [[ "${YES}" -eq 1 ]]; then
    return 0
  fi
  if ! can_prompt; then
    die "no terminal for confirmation. Re-run from a shell, or pass --yes."
  fi
  local go=""
  ask_yn go "Proceed with this plan?" "y"
  [[ "${go}" -eq 1 ]] || die "aborted"
}

count_steps() {
  STEP_TOTAL=1 # system check
  if [[ "${SKIP_PACKAGES}" -eq 0 ]]; then
    STEP_TOTAL=$((STEP_TOTAL + 1))
    role_has_cp && STEP_TOTAL=$((STEP_TOTAL + 1)) # node
    role_has_gh && STEP_TOTAL=$((STEP_TOTAL + 1)) # docker
  fi
  if [[ "${SKIP_BUILD}" -eq 0 ]]; then
    STEP_TOTAL=$((STEP_TOTAL + 1)) # rust
  fi
  if [[ "${SKIP_CLONE}" -eq 0 ]]; then
    STEP_TOTAL=$((STEP_TOTAL + 1))
  fi
  if [[ "${SKIP_BUILD}" -eq 0 ]]; then
    STEP_TOTAL=$((STEP_TOTAL + 1))
  fi
  STEP_TOTAL=$((STEP_TOTAL + 1)) # install files
  if role_has_cp && [[ "${INSTALL_MARIADB}" -eq 1 ]]; then
    STEP_TOTAL=$((STEP_TOTAL + 1))
  fi
  STEP_TOTAL=$((STEP_TOTAL + 1)) # services / finish
  if role_has_gh && [[ -n "${ENROLL_TOKEN}" ]]; then
    STEP_TOTAL=$((STEP_TOTAL + 1))
  fi
}

mysql_cli() {
  if command -v mysql >/dev/null 2>&1; then
    mysql "$@"
  else
    mariadb "$@"
  fi
}

random_secret() {
  openssl rand -base64 24 | tr -d '/+=' | head -c 28
}

random_master() {
  openssl rand -hex 32
}

install_packages() {
  if [[ "${SKIP_PACKAGES}" -eq 1 ]]; then
    info "Skipping apt packages (--skip-packages)"
    return 0
  fi
  begin_step "Installing OS packages"
  export DEBIAN_FRONTEND=noninteractive
  export NEEDRESTART_MODE="${NEEDRESTART_MODE:-a}"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ apt-get update" >&2
    echo "+ apt-get install -y ca-certificates curl git gnupg openssl build-essential pkg-config libssl-dev python3" >&2
    if role_has_cp && [[ "${INSTALL_MARIADB}" -eq 1 ]]; then
      echo "+ apt-get install -y mariadb-server" >&2
    fi
    ok "OS packages"
    return 0
  fi
  log_cmd "apt-get update" apt-get update
  log_cmd "install OS packages" apt-get install -y --no-install-recommends \
    ca-certificates curl git gnupg openssl \
    build-essential pkg-config libssl-dev \
    python3
  if role_has_cp && [[ "${INSTALL_MARIADB}" -eq 1 ]]; then
    log_cmd "install MariaDB" apt-get install -y --no-install-recommends mariadb-server
  fi
  ok "OS packages"
}

install_node() {
  if ! role_has_cp; then
    return 0
  fi
  if [[ "${SKIP_PACKAGES}" -eq 1 ]]; then
    return 0
  fi
  begin_step "Installing Node.js ${FPS_NODE_MAJOR} and pnpm"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ curl -fsSL https://deb.nodesource.com/setup_${FPS_NODE_MAJOR}.x | bash -" >&2
    echo "+ apt-get install -y nodejs" >&2
    echo "+ corepack enable && corepack prepare pnpm@${FPS_PNPM_VERSION} --activate" >&2
    ok "Node.js ${FPS_NODE_MAJOR} + pnpm ${FPS_PNPM_VERSION}"
    return 0
  fi
  local major=0
  if command -v node >/dev/null 2>&1; then
    major="$(node -v 2>/dev/null | sed 's/^v//' | cut -d. -f1)"
  fi
  if [[ "${major}" -lt "${FPS_NODE_MAJOR}" ]]; then
    if ! curl -fsSL "https://deb.nodesource.com/setup_${FPS_NODE_MAJOR}.x" | bash - >>"${LOG}" 2>&1; then
      show_log_tail
      die "NodeSource setup failed. See ${LOG}"
    fi
    log_cmd "install Node.js" apt-get install -y nodejs
  fi
  log_cmd "corepack enable" corepack enable
  log_cmd "activate pnpm ${FPS_PNPM_VERSION}" corepack prepare "pnpm@${FPS_PNPM_VERSION}" --activate
  ok "Node.js $(node -v 2>/dev/null || echo ${FPS_NODE_MAJOR}) + pnpm"
}

install_docker() {
  if ! role_has_gh; then
    return 0
  fi
  if [[ "${SKIP_PACKAGES}" -eq 1 ]]; then
    return 0
  fi
  begin_step "Installing Docker Engine (${OS_ID})"
  if command -v docker >/dev/null 2>&1 && [[ "${DRY_RUN}" -eq 0 ]]; then
    systemctl enable --now docker </dev/null >>"${LOG}" 2>&1 || true
    ok "Docker already installed"
    return 0
  fi
  local gpg_url="https://download.docker.com/linux/${OS_ID}/gpg"
  local docker_codename
  docker_codename="$(docker_apt_codename)"
  local repo="deb [arch=${OS_ARCH} signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/${OS_ID} ${docker_codename} stable"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ curl -fsSL ${gpg_url} -o /etc/apt/keyrings/docker.asc" >&2
    echo "+ echo ${repo} > /etc/apt/sources.list.d/docker.list" >&2
    echo "+ apt-get update && apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin" >&2
    echo "+ systemctl enable --now docker" >&2
    ok "Docker Engine (${OS_ID}/${docker_codename})"
    return 0
  fi
  install -m 0755 -d /etc/apt/keyrings
  curl -fsSL "${gpg_url}" -o /etc/apt/keyrings/docker.asc </dev/null
  chmod a+r /etc/apt/keyrings/docker.asc
  echo "${repo}" >/etc/apt/sources.list.d/docker.list
  log_cmd "apt-get update (Docker repo)" apt-get update
  if ! apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin </dev/null >>"${LOG}" 2>&1; then
    local fallback
    fallback="$(docker_apt_fallback_codename)"
    if [[ -n "${fallback}" && "${fallback}" != "${docker_codename}" ]]; then
      warn "Docker packages for ${docker_codename} were missing; retrying with ${fallback}."
      repo="deb [arch=${OS_ARCH} signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/${OS_ID} ${fallback} stable"
      echo "${repo}" >/etc/apt/sources.list.d/docker.list
      log_cmd "apt-get update (Docker ${fallback})" apt-get update
      log_cmd "install Docker Engine (${fallback})" apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin
    else
      show_log_tail
      die "Docker Engine install failed. See ${LOG}"
    fi
  fi
  log_cmd "enable docker" systemctl enable --now docker
  ok "Docker Engine"
}

# Ubuntu 25+/Debian testing may not have a Docker apt pocket yet. Map to the
# last known LTS/stable codename so a fresh 26.04 box still installs Engine.
docker_apt_codename() {
  local fallback
  fallback="$(docker_apt_fallback_codename)"
  if [[ -n "${fallback}" ]]; then
    echo "${fallback}"
    return 0
  fi
  echo "${OS_CODENAME}"
}

docker_apt_fallback_codename() {
  case "${OS_ID}" in
    ubuntu)
      case "${OS_CODENAME}" in
        noble | jammy | focal) echo "${OS_CODENAME}" ;;
        resolute | questing | plucky | oracular) echo noble ;;
        *)
          local major="${OS_VERSION_ID%%.*}"
          if [[ "${major}" -ge 26 ]]; then
            echo noble
          fi
          ;;
      esac
      ;;
    debian)
      case "${OS_CODENAME}" in
        bookworm | bullseye) echo "${OS_CODENAME}" ;;
        trixie | forky | sid) echo bookworm ;;
      esac
      ;;
  esac
}

install_rust() {
  if [[ "${SKIP_BUILD}" -eq 1 ]]; then
    info "Skipping Rust toolchain"
    return 0
  fi
  begin_step "Installing Rust ${FPS_RUST_TOOLCHAIN}"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${FPS_RUST_TOOLCHAIN}" >&2
    ok "Rust ${FPS_RUST_TOOLCHAIN}"
    return 0
  fi
  # shellcheck disable=SC1091
  source /root/.cargo/env 2>/dev/null || source "${HOME}/.cargo/env" 2>/dev/null || true
  local rustup_bin=""
  if [[ -x /root/.cargo/bin/rustup ]]; then
    rustup_bin=/root/.cargo/bin/rustup
  elif command -v rustup >/dev/null 2>&1; then
    rustup_bin="$(command -v rustup)"
  fi
  if [[ -n "${rustup_bin}" ]]; then
    log_cmd "rustup toolchain install ${FPS_RUST_TOOLCHAIN}" \
      "${rustup_bin}" toolchain install "${FPS_RUST_TOOLCHAIN}" --profile minimal
    log_cmd "rustup default ${FPS_RUST_TOOLCHAIN}" \
      "${rustup_bin}" default "${FPS_RUST_TOOLCHAIN}"
    ok "Rust $(rustc --version 2>/dev/null || echo "${FPS_RUST_TOOLCHAIN}")"
    return 0
  fi
  if ! curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain "${FPS_RUST_TOOLCHAIN}" >>"${LOG}" 2>&1; then
    show_log_tail
    die "rustup install failed. See ${LOG}"
  fi
  # shellcheck disable=SC1091
  source /root/.cargo/env 2>/dev/null || source "${HOME}/.cargo/env"
  ok "Rust ${FPS_RUST_TOOLCHAIN}"
}

clone_source() {
  if [[ "${SKIP_CLONE}" -eq 1 ]]; then
    if [[ "${INSTALL_MODE}" == "reconfigure" || "${INSTALL_MODE}" == "repair" ]]; then
      info "Keeping installed binaries (no source fetch)"
      return 0
    fi
    [[ -n "${SRC_DIR}" ]] || die "--skip-clone requires --source-dir or a git checkout of this repo"
    info "Using source at ${SRC_DIR}"
    return 0
  fi
  begin_step "Fetching FPS source"
  if [[ -n "${REPO_ROOT}" && "${REFRESH}" -eq 0 ]]; then
    SRC_DIR="${REPO_ROOT}"
    info "Using local checkout ${SRC_DIR}"
    ok "Source (local checkout)"
    return 0
  fi
  SRC_DIR="${SRC_DIR:-${FPS_PREFIX}/src}"
  local url="${FPS_GIT_URL}"
  if [[ -n "${FPS_GITHUB_TOKEN:-}" && "${url}" == https://github.com/* ]]; then
    url="https://x-access-token:${FPS_GITHUB_TOKEN}@github.com/${url#https://github.com/}"
  fi
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ git clone --depth 1 --branch ${FPS_GIT_REF} <repo> ${SRC_DIR}" >&2
    ok "Source ${FPS_GIT_REF}"
    return 0
  fi
  if [[ -d "${SRC_DIR}/.git" && "${REFRESH}" -eq 0 ]]; then
    git -C "${SRC_DIR}" fetch --depth 1 origin "${FPS_GIT_REF}" </dev/null >>"${LOG}" 2>&1 || true
    git -C "${SRC_DIR}" checkout "${FPS_GIT_REF}" </dev/null >>"${LOG}" 2>&1 || true
    ok "Source (existing ${SRC_DIR})"
    return 0
  fi
  rm -rf "${SRC_DIR}"
  mkdir -p "$(dirname "${SRC_DIR}")"
  if ! git clone --depth 1 --branch "${FPS_GIT_REF}" "${url}" "${SRC_DIR}" </dev/null >>"${LOG}" 2>&1; then
    show_log_tail
    die "git clone failed. Check network access to ${FPS_GIT_URL}."
  fi
  git -C "${SRC_DIR}" remote set-url origin "${FPS_GIT_URL}"
  ok "Cloned ${FPS_GIT_REF}"
}

build_fps() {
  if [[ "${SKIP_BUILD}" -eq 1 ]]; then
    info "Skipping cargo/pnpm build (--skip-build)"
    return 0
  fi
  begin_step "Building FPS from source (this takes a while)"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    # Package name is fps-bootstrap; the installed binary is named `fps`.
    local pkgs="-p fps-bootstrap"
    role_has_cp && pkgs+=" -p fps-control-plane"
    role_has_gh && pkgs+=" -p fps-node-agent"
    echo "+ cargo build --release ${pkgs}" >&2
    if role_has_cp; then
      echo "+ pnpm install && pnpm --filter @fps/web build" >&2
    fi
    ok "Build (dry-run)"
    return 0
  fi
  # shellcheck disable=SC1091
  source /root/.cargo/env 2>/dev/null || true
  cd "${SRC_DIR}"
  export CARGO_TERM_COLOR=always
  export CARGO_PROFILE_RELEASE_LTO=false
  export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
  local pkgs=(-p fps-bootstrap)
  role_has_cp && pkgs+=(-p fps-control-plane)
  role_has_gh && pkgs+=(-p fps-node-agent)
  local locked=()
  if [[ -f "${SRC_DIR}/Cargo.lock" ]]; then
    locked=(--locked)
  fi
  cargo build --release "${locked[@]}" "${pkgs[@]}" 2>&1 | tee -a "${LOG}"
  if role_has_cp; then
    if [[ -f "${SRC_DIR}/pnpm-lock.yaml" ]]; then
      pnpm install --frozen-lockfile 2>&1 | tee -a "${LOG}" || pnpm install 2>&1 | tee -a "${LOG}"
    else
      pnpm install 2>&1 | tee -a "${LOG}"
    fi
    pnpm --filter @fps/web build 2>&1 | tee -a "${LOG}"
  fi
  ok "Build complete"
}

install_binaries() {
  if [[ "${INSTALL_MODE}" == "reconfigure" ]]; then
    begin_step "Keeping binaries; applying settings"
    ok "Reconfigure (no rebuild)"
    return 0
  fi
  begin_step "Installing binaries, units, and env files"
  local current="${FPS_PREFIX}/current"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ install binaries → ${current}" >&2
    role_has_cp && echo "+ copy apps/web/dist → ${FPS_WEB_ROOT}" >&2
    role_has_cp && echo "+ install fps-control-plane.service" >&2
    role_has_gh && echo "+ install fps-node-agent.service" >&2
    ok "Files staged"
    return 0
  fi
  mkdir -p "${current}" "${FPS_DATA_DIR}" /etc/fps /etc/systemd/system
  role_has_gh && mkdir -p "${FPS_AGENT_DIR}"
  role_has_cp && mkdir -p "${FPS_WEB_ROOT}"

  local src_bin="${SRC_DIR}/target/release"
  if [[ -x "${src_bin}/fps" ]]; then
    install -m 0755 "${src_bin}/fps" "${current}/fps"
    ln -sfn "${current}/fps" /usr/local/bin/fps
  fi
  if role_has_cp && [[ -x "${src_bin}/fps-control-plane" ]]; then
    install -m 0755 "${src_bin}/fps-control-plane" "${current}/fps-control-plane"
  fi
  if role_has_gh && [[ -x "${src_bin}/fps-node-agent" ]]; then
    install -m 0755 "${src_bin}/fps-node-agent" "${current}/fps-node-agent"
    ln -sfn "${current}/fps-node-agent" /usr/local/bin/fps-node-agent
  fi
  if role_has_cp && [[ -d "${SRC_DIR}/apps/web/dist" ]]; then
    rm -rf "${FPS_WEB_ROOT}"
    cp -a "${SRC_DIR}/apps/web/dist" "${FPS_WEB_ROOT}"
  fi
  if role_has_cp && [[ -f "${SRC_DIR}/deploy/systemd/fps-control-plane.service" ]]; then
    install -m 0644 "${SRC_DIR}/deploy/systemd/fps-control-plane.service" \
      /etc/systemd/system/fps-control-plane.service
  fi
  if role_has_gh && [[ -f "${SRC_DIR}/deploy/systemd/fps-node-agent.service" ]]; then
    install -m 0644 "${SRC_DIR}/deploy/systemd/fps-node-agent.service" \
      /etc/systemd/system/fps-node-agent.service
  fi
  if [[ "${SKIP_BUILD}" -eq 0 ]]; then
    rm -rf "${SRC_DIR}/target" "${SRC_DIR}/node_modules" "${SRC_DIR}/apps/web/node_modules" || true
  fi
  ok "Binaries and units"
}

setup_mariadb() {
  if ! role_has_cp || [[ "${INSTALL_MARIADB}" -eq 0 ]]; then
    if role_has_cp && [[ "${INSTALL_MARIADB}" -eq 0 ]]; then
      info "Using remote/existing MariaDB ($(redact_db_url "${FPS_DATABASE_URL:-mysql://${DB_USER}@${DB_HOST}:${DB_PORT}/${DB_NAME}}"))"
    fi
    return 0
  fi
  if [[ "${INSTALL_MODE}" == "reconfigure" || "${INSTALL_MODE}" == "repair" ]]; then
    info "Keeping existing MariaDB (reconfigure does not recreate the database)"
    return 0
  fi
  begin_step "Configuring MariaDB"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ systemctl enable --now mariadb" >&2
    echo "+ CREATE DATABASE fps; CREATE USER fps@127.0.0.1 …" >&2
    ok "MariaDB"
    return 0
  fi
  local db_pass="${FPS_DB_PASSWORD:-}"
  if [[ -z "${db_pass}" && -f /etc/fps/control-plane.env && "${FORCE_ENV}" -eq 0 ]]; then
    db_pass="$(awk -F= '/^FPS_DATABASE_URL=/ {print $2}' /etc/fps/control-plane.env | sed -n 's#^mysql://fps:\([^@]*\)@.*#\1#p' | tail -n1)"
  fi
  if [[ -z "${db_pass}" ]]; then
    db_pass="$(random_secret)"
  fi
  FPS_DB_PASSWORD="${db_pass}"
  if systemctl enable --now mariadb </dev/null >>"${LOG}" 2>&1; then
    :
  elif systemctl enable --now mysql </dev/null >>"${LOG}" 2>&1; then
    :
  else
    show_log_tail
    die "failed to start MariaDB/MySQL. See ${LOG}"
  fi
  local i
  for i in $(seq 1 30); do
    if mysql_cli --protocol=socket -e 'SELECT 1' >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
  mysql_cli --protocol=socket <<SQL
CREATE DATABASE IF NOT EXISTS fps CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
CREATE USER IF NOT EXISTS 'fps'@'127.0.0.1' IDENTIFIED BY '${db_pass}';
ALTER USER 'fps'@'127.0.0.1' IDENTIFIED BY '${db_pass}';
GRANT ALL PRIVILEGES ON fps.* TO 'fps'@'127.0.0.1';
FLUSH PRIVILEGES;
SQL
  ok "MariaDB database fps"
}

write_env_and_user() {
  local host
  host="$(public_host)"
  host="${host:-127.0.0.1}"
  if role_has_cp && [[ -z "${FPS_DATABASE_URL}" ]]; then
    if [[ "${INSTALL_MARIADB}" -eq 1 && -n "${FPS_DB_PASSWORD:-}" ]]; then
      DB_HOST="${DB_HOST:-127.0.0.1}"
      build_database_url
    elif [[ "${INSTALL_MARIADB}" -eq 0 ]]; then
      build_database_url
    fi
  fi
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ FPS_PUBLIC_URL=http://${host}:47890" >&2
    echo "+ FPS_CORS_ORIGINS=http://${host}:47880,http://${host}:47890" >&2
    echo "+ FPS_ALLOW_INSECURE_HTTP=${ALLOW_INSECURE}" >&2
    if role_has_gh; then
      echo "+ /etc/fps/node-agent.env FPS_ALLOW_INSECURE_HTTP=${ALLOW_INSECURE}" >&2
    fi
    if role_has_cp; then
      if [[ -n "${FPS_DATABASE_URL}" ]]; then
        echo "+ FPS_DATABASE_URL=$(redact_db_url "${FPS_DATABASE_URL}")" >&2
      elif [[ "${INSTALL_MARIADB}" -eq 0 ]]; then
        echo "+ FPS_DATABASE_URL=mysql://${DB_USER}:***@${DB_HOST}:${DB_PORT}/${DB_NAME}" >&2
      else
        echo "+ FPS_DATABASE_URL=mysql://fps:***@127.0.0.1:3306/fps" >&2
      fi
    fi
    return 0
  fi

  if role_has_cp; then
    if ! getent passwd fps >/dev/null; then
      useradd --system --home "${FPS_DATA_DIR}" --shell /usr/sbin/nologin fps
    fi
    chown -R fps:fps "${FPS_DATA_DIR}" "${FPS_PREFIX}" 2>/dev/null || true
    [[ -d "${FPS_WEB_ROOT}" ]] && chown -R fps:fps "${FPS_WEB_ROOT}"
    mkdir -p /etc/fps
    if [[ -f /etc/fps/control-plane.env && "${FORCE_ENV}" -eq 0 ]]; then
      upsert_env_key /etc/fps/control-plane.env FPS_PUBLIC_URL "http://${host}:47890"
      upsert_env_key /etc/fps/control-plane.env FPS_CORS_ORIGINS "http://${host}:47880,http://${host}:47890"
      upsert_env_key /etc/fps/control-plane.env FPS_ALLOW_INSECURE_HTTP "${ALLOW_INSECURE}"
      upsert_env_key /etc/fps/control-plane.env FPS_HTTP_BIND "${FPS_HTTP_BIND}"
      upsert_env_key /etc/fps/control-plane.env FPS_WEB_BIND "${FPS_WEB_BIND}"
      upsert_env_key /etc/fps/control-plane.env FPS_WEB_ROOT "${FPS_WEB_ROOT}"
      if [[ -n "${FPS_DATABASE_URL}" ]]; then
        upsert_env_key /etc/fps/control-plane.env FPS_DATABASE_URL "${FPS_DATABASE_URL}"
      fi
      chmod 0600 /etc/fps/control-plane.env
      chown root:fps /etc/fps/control-plane.env 2>/dev/null || true
      ok "Updated public URL / CORS in /etc/fps/control-plane.env"
    else
      local master="${FPS_MASTER_KEY:-}"
      [[ -n "${master}" ]] || master="$(random_master)"
      local db_url="${FPS_DATABASE_URL:-mysql://fps:${FPS_DB_PASSWORD}@127.0.0.1:3306/fps}"
      umask 077
      cat >/etc/fps/control-plane.env <<EOF
FPS_DATABASE_URL=${db_url}
FPS_MASTER_KEY=${master}
FPS_HTTP_BIND=${FPS_HTTP_BIND}
FPS_NODE_BIND=${FPS_NODE_BIND}
FPS_WEB_BIND=${FPS_WEB_BIND}
FPS_WEB_ROOT=${FPS_WEB_ROOT}
FPS_PUBLIC_URL=http://${host}:47890
FPS_CORS_ORIGINS=http://${host}:47880,http://${host}:47890
FPS_DATA_DIR=${FPS_DATA_DIR}
FPS_ALLOW_INSECURE_HTTP=${ALLOW_INSECURE}
FPS_LOG_FORMAT=json
EOF
      chmod 0600 /etc/fps/control-plane.env
      chown root:fps /etc/fps/control-plane.env
      ok "Wrote /etc/fps/control-plane.env"
    fi
  fi

  if role_has_gh; then
    mkdir -p /etc/fps
    if [[ -f /etc/fps/node-agent.env && "${FORCE_ENV}" -eq 0 ]]; then
      upsert_env_key /etc/fps/node-agent.env FPS_ALLOW_INSECURE_HTTP "${ALLOW_INSECURE}"
      chmod 0600 /etc/fps/node-agent.env
      ok "Updated FPS_ALLOW_INSECURE_HTTP in /etc/fps/node-agent.env"
    else
      umask 077
      cat >/etc/fps/node-agent.env <<EOF
FPS_LOG_FORMAT=json
FPS_ALLOW_INSECURE_HTTP=${ALLOW_INSECURE}
EOF
      chmod 0600 /etc/fps/node-agent.env
      ok "Wrote /etc/fps/node-agent.env"
    fi
  fi
}

maybe_enroll() {
  if ! role_has_gh; then
    return 0
  fi
  if [[ -z "${ENROLL_TOKEN}" || -z "${CONTROL_PLANE_URL}" ]]; then
    return 0
  fi
  begin_step "Enrolling this node with the control plane"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    if [[ "${ALLOW_INSECURE}" == "true" ]]; then
      echo "+ fps-node-agent enroll --url ${CONTROL_PLANE_URL} --allow-insecure-http" >&2
    else
      echo "+ fps-node-agent enroll --url ${CONTROL_PLANE_URL}" >&2
    fi
    ok "Enroll (dry-run)"
    return 0
  fi
  local enroll_args=(
    enroll
    --url "${CONTROL_PLANE_URL}"
    --token "${ENROLL_TOKEN}"
    --data-dir "${FPS_AGENT_DIR}"
  )
  if [[ "${ALLOW_INSECURE}" == "true" ]]; then
    enroll_args+=(--allow-insecure-http)
  fi
  "${FPS_PREFIX}/current/fps-node-agent" "${enroll_args[@]}"
  ok "Node enrolled"
}

start_services() {
  begin_step "Enabling services"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    if [[ "${START}" -eq 1 ]]; then
      role_has_cp && echo "+ systemctl enable --now fps-control-plane.service" >&2
      if role_has_gh && [[ -n "${ENROLL_TOKEN}" ]]; then
        echo "+ systemctl enable --now fps-node-agent.service" >&2
      elif role_has_gh; then
        echo "+ systemctl daemon-reload (agent not started until enrolled)" >&2
      fi
    else
      echo "+ systemctl daemon-reload (not starting)" >&2
    fi
    echo "+ touch ${FPS_DATA_DIR}/.provision-complete" >&2
    ok "Services (dry-run)"
    return 0
  fi
  systemctl daemon-reload
  if [[ "${START}" -eq 1 ]]; then
    if role_has_cp; then
      systemctl enable --now fps-control-plane.service
      ok "fps-control-plane.service"
    fi
    if role_has_gh && [[ -n "${ENROLL_TOKEN}" ]]; then
      systemctl enable --now fps-node-agent.service
      ok "fps-node-agent.service"
    elif role_has_gh; then
      info "Agent installed but not started (enroll first)"
    fi
  else
    info "Units written; not started (--skip-start)"
  fi
  mkdir -p "${FPS_DATA_DIR}"
  date -u +'%Y-%m-%dT%H:%M:%SZ' >"${FPS_DATA_DIR}/.provision-complete"
}

print_summary() {
  local host
  host="$(public_host)"
  host="${host:-127.0.0.1}"
  printf '\n%b╭──────────────────────────────────────────────────────────────╮%b\n' "${C_GREEN}" "${C_RESET}"
  printf '%b│%b  %bFPS is ready on this machine%b                               %b│%b\n' "${C_GREEN}" "${C_RESET}" "${C_BOLD}" "${C_RESET}" "${C_GREEN}" "${C_RESET}"
  printf '%b╰──────────────────────────────────────────────────────────────╯%b\n' "${C_GREEN}" "${C_RESET}"
  printf '  Mode       %s\n' "${INSTALL_MODE:-fresh}"
  printf '  Role       %s\n' "${ROLE}"
  printf '  OS         %s\n' "${OS_PRETTY}"
  if role_has_cp; then
    printf '  Web UI     http://%s:47880\n' "${host}"
    printf '  API        http://%s:47890\n' "${host}"
    printf '  Node mTLS  %s:47891\n' "${host}"
    printf '\nOpen the web UI and create the owner account (password ≥ 12 characters).\n'
  fi
  if role_has_gh; then
    printf '  Docker     Engine installed for %s\n' "${OS_ID}"
    printf '  Agent      %s/current/fps-node-agent\n' "${FPS_PREFIX}"
    if [[ -z "${ENROLL_TOKEN}" ]]; then
      printf '\nEnroll after creating a token in the panel:\n\n'
      printf '  fps-node-agent enroll --url http://PANEL_IP:47890 --token TOKEN \\\n'
      if [[ "${ALLOW_INSECURE}" == "true" ]]; then
        printf '    --data-dir %s --allow-insecure-http\n' "${FPS_AGENT_DIR}"
      else
        printf '    --data-dir %s\n' "${FPS_AGENT_DIR}"
      fi
      printf '  systemctl enable --now fps-node-agent.service\n'
    fi
  fi
  printf '\nOpen firewall / security group:\n'
  role_has_cp && printf '  • TCP 47880 (web UI) and TCP 47890 (API) for administrators\n'
  role_has_cp && printf '  • TCP 47890 and TCP 47891 from game hosts\n'
  role_has_gh && printf '  • game ports you allocate on this host, plus outbound to the panel\n'
  if [[ "${DRY_RUN}" -eq 0 && -n "${LOG}" && "${LOG}" != "/dev/null" ]]; then
    printf '\n  Log        %s\n' "${LOG}"
  fi
  printf '\n'
}

main() {
  resolve_self
  header
  need_root
  load_os
  if [[ -n "${REPO_ROOT}" && -z "${SRC_DIR}" && "${REFRESH}" -eq 0 ]]; then
    SRC_DIR="${REPO_ROOT}"
    SKIP_CLONE=1
  fi
  if [[ -n "${SRC_DIR}" ]]; then
    SKIP_CLONE=1
  fi

  pick_install_mode
  apply_install_mode
  if [[ -n "${ROLE}" ]]; then
    ROLE="$(normalize_role "${ROLE}")"
  else
    pick_role
  fi
  if role_has_gh && is_lxc && [[ "${DRY_RUN}" -eq 0 && "${ASSUME_ROOT}" -eq 0 ]]; then
    die "Game hosts must be full VMs (or dedicated/VPS). LXC is not supported for the Docker game runtime."
  fi
  if role_has_gh && is_lxc && [[ "${DRY_RUN}" -eq 1 ]]; then
    warn "Game hosts must be full VMs. LXC would be refused on a real run."
  fi

  prompt_missing
  log_init
  count_steps
  confirm_plan

  begin_step "Checking this machine"
  info "${OS_PRETTY} (${OS_ID} ${OS_VERSION_ID} ${OS_CODENAME}) ${OS_ARCH}"
  if [[ "${DRY_RUN}" -eq 0 ]]; then
    local mem_kib
    mem_kib="$(awk '/MemTotal:/ {print $2}' /proc/meminfo 2>/dev/null || echo 0)"
    if [[ "${mem_kib}" -gt 0 && "${mem_kib}" -lt 1500000 ]]; then
      warn "Less than ~1.5 GiB RAM. The Rust build may OOM. Give the VM 4 GiB+."
    fi
  fi
  ok "System looks like a supported ${OS_ID} host"

  install_packages
  install_node
  install_docker
  install_rust
  clone_source
  build_fps
  install_binaries
  setup_mariadb
  write_env_and_user
  maybe_enroll
  start_services
  print_summary
}

main "$@"
