#!/usr/bin/env bash
# Host install for FPS systemd units.
# Copies binaries if present, writes units and env templates.
# Does NOT start services unless --start is passed (default off).
# Does NOT SSH into Proxmox or create guests.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: install.sh [--start] [--destdir DIR] [--bin-dir DIR] [--prefix DIR]

  --start       systemctl enable --now the units (default: off; write files only)
  --destdir     prefix all install paths (for packaging / tests; never starts)
  --bin-dir     directory to search for fps-control-plane / node-agent
  --prefix      binary prefix (default /opt/fps)

This script never contacts Proxmox.
EOF
}

START=0
DESTDIR=""
BIN_DIR=""
PREFIX="/opt/fps"

while [[ $# -gt 0 ]]; do
  case "$1" in
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

mkdir -p "${CURRENT}" "${UNITDIR}" "${ENVDIR}" "${CP_DATA}" "${AGENT_DATA}"

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

copy_bin_if_present fps-control-plane
copy_bin_if_present fps-node-agent
copy_bin_if_present fps

install -D -m 0644 "${UNIT_SRC}/fps-control-plane.service" \
  "${UNITDIR}/fps-control-plane.service"
install -D -m 0644 "${UNIT_SRC}/fps-node-agent.service" \
  "${UNITDIR}/fps-node-agent.service"
echo "wrote units under ${UNITDIR}"

if [[ ! -f "${ENVDIR}/control-plane.env" ]]; then
  cat >"${ENVDIR}/control-plane.env" <<'EOF'
FPS_DATABASE_URL=mysql://fps:change-me@127.0.0.1:3306/fps
FPS_MASTER_KEY=
FPS_HTTP_BIND=0.0.0.0:47890
FPS_NODE_BIND=0.0.0.0:47891
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

if [[ ! -f "${ENVDIR}/node-agent.env" ]]; then
  cat >"${ENVDIR}/node-agent.env" <<'EOF'
FPS_LOG_FORMAT=json
EOF
  chmod 0600 "${ENVDIR}/node-agent.env" 2>/dev/null || true
  echo "wrote ${ENVDIR}/node-agent.env"
else
  echo "keep existing ${ENVDIR}/node-agent.env"
fi

if [[ -n "${DESTDIR}" ]]; then
  echo "destdir set; not invoking systemctl (START=${START})"
  exit 0
fi

if [[ "${START}" -eq 1 ]]; then
  if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload
    systemctl enable --now fps-control-plane.service
    systemctl enable --now fps-node-agent.service
    echo "started fps-control-plane and fps-node-agent"
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
