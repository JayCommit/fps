#!/usr/bin/env bash
# Runs INSIDE the Homer game-host VM (Debian cloud image, never LXC).
# Installs Docker Engine, builds fps-node-agent from git, writes systemd.
# Does not enroll unless FPS_ENROLL_TOKEN and FPS_CONTROL_PLANE_URL are set.
set -euo pipefail

FPS_GIT_URL="${FPS_GIT_URL:-https://github.com/JayCommit/fps.git}"
FPS_GIT_REF="${FPS_GIT_REF:-main}"
FPS_PREFIX="${FPS_PREFIX:-/opt/fps}"
FPS_DATA_DIR="${FPS_DATA_DIR:-/var/lib/fps/agent}"
SRC_DIR="${FPS_SRC_DIR:-/opt/fps/src}"
DRY_RUN="${FPS_DRY_RUN:-0}"
SKIP_BUILD="${FPS_SKIP_BUILD:-0}"

info() { printf '==> %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

run() {
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    printf '+ %s\n' "$*"
    return 0
  fi
  "$@"
}

install_packages() {
  info "Installing Debian packages"
  export DEBIAN_FRONTEND=noninteractive
  run apt-get update
  run apt-get install -y --no-install-recommends \
    ca-certificates curl git gnupg openssl \
    build-essential pkg-config libssl-dev \
    qemu-guest-agent python3
  run systemctl enable --now qemu-guest-agent || true
}

install_docker() {
  if command -v docker >/dev/null 2>&1; then
    info "Docker already installed"
    run systemctl enable --now docker || true
    return 0
  fi
  info "Installing Docker Engine"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo '+ add Docker apt repo and install docker-ce'
    return 0
  fi
  install -m 0755 -d /etc/apt/keyrings
  curl -fsSL https://download.docker.com/linux/debian/gpg -o /etc/apt/keyrings/docker.asc
  chmod a+r /etc/apt/keyrings/docker.asc
  # shellcheck disable=SC1091
  . /etc/os-release
  echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/debian ${VERSION_CODENAME} stable" \
    >/etc/apt/sources.list.d/docker.list
  apt-get update
  apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin
  systemctl enable --now docker
}

install_rust() {
  if [[ -x /root/.cargo/bin/rustc ]]; then
    # shellcheck disable=SC1091
    source /root/.cargo/env 2>/dev/null || true
    return 0
  fi
  info "Installing Rust 1.98 via rustup"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo '+ rustup 1.98.0'
    return 0
  fi
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.98.0
  # shellcheck disable=SC1091
  source /root/.cargo/env
}

clone_source() {
  if [[ -z "${FPS_GITHUB_TOKEN:-}" && -f /root/.fps-github-token ]]; then
    FPS_GITHUB_TOKEN="$(cat /root/.fps-github-token)"
  fi
  if [[ -z "${FPS_GITHUB_TOKEN:-}" && -f /etc/fps-guest.env ]]; then
    set -a
    # shellcheck disable=SC1091
    source /etc/fps-guest.env
    set +a
  fi
  info "Cloning ${FPS_GIT_URL} (${FPS_GIT_REF})"
  local url="${FPS_GIT_URL}"
  if [[ -n "${FPS_GITHUB_TOKEN:-}" && "${url}" == https://github.com/* ]]; then
    url="https://x-access-token:${FPS_GITHUB_TOKEN}@github.com/${url#https://github.com/}"
  fi
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ git clone --depth 1 --branch ${FPS_GIT_REF} <repo> ${SRC_DIR}"
    return 0
  fi
  rm -rf "${SRC_DIR}"
  mkdir -p "$(dirname "${SRC_DIR}")"
  if ! git clone --depth 1 --branch "${FPS_GIT_REF}" "${url}" "${SRC_DIR}"; then
    die "git clone failed. For a private repo export FPS_GITHUB_TOKEN."
  fi
  git -C "${SRC_DIR}" remote set-url origin "${FPS_GIT_URL}"
  unset FPS_GITHUB_TOKEN
  rm -f /root/.fps-github-token /etc/fps-guest.env
}

build_agent() {
  if [[ "${SKIP_BUILD}" -eq 1 ]]; then
    info "Skipping cargo build (FPS_SKIP_BUILD=1)"
    return 0
  fi
  # shellcheck disable=SC1091
  source /root/.cargo/env 2>/dev/null || true
  info "Building fps-node-agent and fps (release). This takes a while."
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo '+ cargo build --release -p fps-node-agent -p fps'
    return 0
  fi
  cd "${SRC_DIR}"
  export CARGO_TERM_COLOR=always
  export CARGO_PROFILE_RELEASE_LTO=false
  export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
  cargo build --release -p fps-node-agent -p fps
}

install_binaries() {
  info "Installing agent under ${FPS_PREFIX}"
  local current="${FPS_PREFIX}/current"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ install fps-node-agent → ${current}"
    return 0
  fi
  mkdir -p "${current}" "${FPS_DATA_DIR}" /etc/fps /etc/systemd/system
  if [[ -x "${SRC_DIR}/target/release/fps-node-agent" ]]; then
    install -m 0755 "${SRC_DIR}/target/release/fps-node-agent" "${current}/fps-node-agent"
    ln -sfn "${current}/fps-node-agent" /usr/local/bin/fps-node-agent
  fi
  if [[ -x "${SRC_DIR}/target/release/fps" ]]; then
    install -m 0755 "${SRC_DIR}/target/release/fps" "${current}/fps"
    ln -sfn "${current}/fps" /usr/local/bin/fps
  fi
  if [[ -f "${SRC_DIR}/deploy/systemd/fps-node-agent.service" ]]; then
    install -m 0644 "${SRC_DIR}/deploy/systemd/fps-node-agent.service" \
      /etc/systemd/system/fps-node-agent.service
  fi
  if [[ ! -f /etc/fps/node-agent.env ]]; then
    cat >/etc/fps/node-agent.env <<'EOF'
FPS_LOG_FORMAT=json
EOF
    chmod 0600 /etc/fps/node-agent.env
  fi
  rm -rf "${SRC_DIR}/target" || true
}

maybe_enroll() {
  if [[ -z "${FPS_ENROLL_TOKEN:-}" || -z "${FPS_CONTROL_PLANE_URL:-}" ]]; then
    info "No enrollment token provided — agent is installed but not enrolled"
    cat <<'EOF'

Next:
  1. Open the Fry web UI and create an enrollment token (Nodes).
  2. On this VM:

     fps-node-agent enroll \
       --url http://FRY_IP:47890 \
       --token PASTE_TOKEN_HERE \
       --data-dir /var/lib/fps/agent \
       --allow-insecure-http

     systemctl enable --now fps-node-agent.service
EOF
    return 0
  fi
  info "Enrolling this node with the control plane"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ fps-node-agent enroll --url ${FPS_CONTROL_PLANE_URL}"
    echo '+ systemctl enable --now fps-node-agent.service'
    return 0
  fi
  "${FPS_PREFIX}/current/fps-node-agent" enroll \
    --url "${FPS_CONTROL_PLANE_URL}" \
    --token "${FPS_ENROLL_TOKEN}" \
    --data-dir "${FPS_DATA_DIR}" \
    --allow-insecure-http
  systemctl daemon-reload
  systemctl enable --now fps-node-agent.service
}

finish() {
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo '+ touch /var/lib/fps/.provision-complete'
    return 0
  fi
  mkdir -p /var/lib/fps
  date -u +'%Y-%m-%dT%H:%M:%SZ' >/var/lib/fps/.provision-complete
  systemctl daemon-reload
  docker info >/dev/null
  info "Docker Engine is running. Game host provision complete."
}

main() {
  if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=1
    shift
  fi
  if [[ "${EUID}" -ne 0 && "${DRY_RUN}" -ne 1 ]]; then
    die "guest bootstrap must run as root"
  fi
  if [[ "${DRY_RUN}" -ne 1 ]]; then
    if grep -qa 'container=lxc' /proc/1/environ 2>/dev/null \
      || [[ "$(cat /run/systemd/container 2>/dev/null || true)" == "lxc" ]]; then
      die "FPS game hosts must be full VMs. LXC is not supported for Docker game runtime."
    fi
  fi
  install_packages
  install_docker
  install_rust
  clone_source
  build_agent
  install_binaries
  maybe_enroll
  finish
}

main "$@"
