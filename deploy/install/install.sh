#!/usr/bin/env bash
# Host install for FPS systemd units (binaries already built).
# Copies binaries if present, writes units and env templates.
# Does NOT start services unless --start is passed (default off).
# Does NOT create VMs. For a fresh Ubuntu/Debian machine, use ../install.sh.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: install.sh [--role ROLE] [--start] [--destdir DIR] [--bin-dir DIR] [--prefix DIR]

  --role        control-plane | game-host | both
                aliases: web/fry, node/homer
                default: both
                If omitted and stdin is a TTY, the script asks.
  --start       systemctl enable --now the units for that role (default: off)
  --destdir     prefix all install paths (for packaging / tests; never starts)
  --bin-dir     directory to search for fps-control-plane / fps-node-agent / fps
  --prefix      binary prefix (default /opt/fps)

This script never creates VMs. For a fresh Ubuntu/Debian machine use `deploy/install.sh`.
Prefer `fps install` when the CLI is on PATH; this script is the same picker
for hosts that only have the deploy tree.
EOF
}

ROLE=""
START=0
DESTDIR=""
BIN_DIR=""
PREFIX="/opt/fps"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --role)
      ROLE="${2:?--role requires control-plane, game-host, or both}"
      shift 2
      ;;
    --start)
      START=1
      shift
      ;;
    --destdir)
      DESTDIR="${2:?--destdir requires a directory}"
      shift 2
      ;;
    --bin-dir)
      BIN_DIR="${2:?--bin-dir requires a directory}"
      shift 2
      ;;
    --prefix)
      PREFIX="${2:?--prefix requires a directory}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

normalize_role() {
  local raw
  raw="$(echo "$1" | tr '[:upper:]' '[:lower:]')"
  case "$raw" in
    1|control-plane|control_plane|controlplane|web|panel|api|fry) echo control-plane ;;
    2|game-host|game_host|gamehost|node|agent|homer) echo game-host ;;
    3|both|all|lab) echo both ;;
    *)
      echo "unknown role: $1" >&2
      echo "use control-plane, game-host, or both" >&2
      exit 2
      ;;
  esac
}

if [[ -z "${ROLE}" ]]; then
  if [[ -t 0 ]]; then
    cat <<'EOF'

FPS installer — what should this machine be?

  1) Control plane   web panel + API          (Fry)
  2) Game host       Docker + node agent      (Homer)
  3) Both            lab only, not the usual two-host split

EOF
    printf 'Select 1, 2, or 3: '
    read -r ROLE
  else
    echo "no TTY: pass --role control-plane, --role game-host, or --role both" >&2
    exit 2
  fi
fi
ROLE="$(normalize_role "${ROLE}")"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UNIT_SRC="$(cd "${SCRIPT_DIR}/../systemd" && pwd)"

root() {
  printf '%s%s' "${DESTDIR}" "$1"
}

CURRENT="$(root "${PREFIX}/current")"
UNITDIR="$(root /etc/systemd/system)"
ENVDIR="$(root /etc/fps)"
CP_DATA="$(root /var/lib/fps)"
AGENT_DATA="$(root /var/lib/fps/agent)"

mkdir -p "${CURRENT}" "${UNITDIR}" "${ENVDIR}"

copy_bin_if_present() {
  local name="$1"
  local dest="${CURRENT}/${name}"
  local src=""
  if [[ -n "${BIN_DIR}" && -f "${BIN_DIR}/${name}" ]]; then
    src="${BIN_DIR}/${name}"
  elif [[ -f "./${name}" ]]; then
    src="./${name}"
  fi
  if [[ -n "${src}" ]]; then
    install -D -m 0755 "${src}" "${dest}"
    echo "copied ${src} -> ${dest}"
  else
    echo "skip binary ${name}: not found (place it at ${CURRENT}/${name} later)"
  fi
}

copy_bin_if_present fps

if [[ "${ROLE}" == "control-plane" || "${ROLE}" == "both" ]]; then
  mkdir -p "${CP_DATA}"
  copy_bin_if_present fps-control-plane
  install -D -m 0644 "${UNIT_SRC}/fps-control-plane.service" \
    "${UNITDIR}/fps-control-plane.service"
  if [[ ! -f "${ENVDIR}/control-plane.env" ]]; then
    cat >"${ENVDIR}/control-plane.env" <<'EOF'
FPS_DATABASE_URL=mysql://fps:change-me@127.0.0.1:3306/fps
FPS_MASTER_KEY=
FPS_HTTP_BIND=0.0.0.0:47890
FPS_NODE_BIND=0.0.0.0:47891
FPS_WEB_BIND=0.0.0.0:47880
FPS_WEB_ROOT=/opt/fps/web
FPS_PUBLIC_URL=http://127.0.0.1:47890
FPS_DATA_DIR=/var/lib/fps
FPS_ALLOW_INSECURE_HTTP=false
FPS_LOG_FORMAT=json
EOF
    chmod 0600 "${ENVDIR}/control-plane.env" 2>/dev/null || true
    echo "wrote ${ENVDIR}/control-plane.env (edit before start)"
  else
    echo "keep existing ${ENVDIR}/control-plane.env"
  fi
fi

if [[ "${ROLE}" == "game-host" || "${ROLE}" == "both" ]]; then
  mkdir -p "${AGENT_DATA}"
  copy_bin_if_present fps-node-agent
  install -D -m 0644 "${UNIT_SRC}/fps-node-agent.service" \
    "${UNITDIR}/fps-node-agent.service"
  if [[ ! -f "${ENVDIR}/node-agent.env" ]]; then
    cat >"${ENVDIR}/node-agent.env" <<'EOF'
FPS_LOG_FORMAT=json
# Must match the control plane. systemd does not pass --allow-insecure-http.
FPS_ALLOW_INSECURE_HTTP=false
EOF
    chmod 0600 "${ENVDIR}/node-agent.env" 2>/dev/null || true
    echo "wrote ${ENVDIR}/node-agent.env"
  else
    echo "keep existing ${ENVDIR}/node-agent.env"
  fi
fi

echo "role=${ROLE} units under ${UNITDIR}"

if [[ -n "${DESTDIR}" ]]; then
  echo "destdir set; not invoking systemctl (START=${START})"
  exit 0
fi

if [[ "${START}" -eq 1 ]]; then
  if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload
    if [[ "${ROLE}" == "control-plane" || "${ROLE}" == "both" ]]; then
      systemctl enable --now fps-control-plane.service
    fi
    if [[ "${ROLE}" == "game-host" || "${ROLE}" == "both" ]]; then
      systemctl enable --now fps-node-agent.service
    fi
    echo "started units for role ${ROLE}"
  else
    echo "systemctl not found; units written but not started" >&2
    exit 1
  fi
else
  echo "units written; services not started (pass --start to enable --now)"
  if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload || true
  fi
fi
