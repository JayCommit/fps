#!/usr/bin/env bash
# Runs INSIDE the Fry control-plane guest (Debian LXC by default).
# Installs MariaDB, builds FPS from git, serves the web UI + API, starts systemd.
set -euo pipefail

FPS_GIT_URL="${FPS_GIT_URL:-https://github.com/JayCommit/fps.git}"
FPS_GIT_REF="${FPS_GIT_REF:-main}"
FPS_PREFIX="${FPS_PREFIX:-/opt/fps}"
FPS_WEB_ROOT="${FPS_WEB_ROOT:-/opt/fps/web}"
FPS_DATA_DIR="${FPS_DATA_DIR:-/var/lib/fps}"
FPS_HTTP_BIND="${FPS_HTTP_BIND:-0.0.0.0:47890}"
FPS_WEB_BIND="${FPS_WEB_BIND:-0.0.0.0:47880}"
FPS_NODE_BIND="${FPS_NODE_BIND:-0.0.0.0:47891}"
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

public_base() {
  if [[ -n "${FPS_PUBLIC_IP:-}" ]]; then
    echo "${FPS_PUBLIC_IP}"
    return
  fi
  hostname -I 2>/dev/null | awk '{print $1}'
}

install_packages() {
  info "Installing Debian packages (MariaDB, build tools, Node.js 22)"
  export DEBIAN_FRONTEND=noninteractive
  run apt-get update
  run apt-get install -y --no-install-recommends \
    ca-certificates curl git gnupg openssl \
    build-essential pkg-config libssl-dev \
    mariadb-server \
    python3
  if ! command -v node >/dev/null 2>&1 || [[ "$(node -v 2>/dev/null | sed 's/^v//' | cut -d. -f1)" -lt 22 ]]; then
    if [[ "${DRY_RUN}" -eq 1 ]]; then
      echo '+ curl -fsSL https://deb.nodesource.com/setup_22.x | bash -'
      echo '+ apt-get install -y nodejs'
    else
      curl -fsSL https://deb.nodesource.com/setup_22.x | bash -
      apt-get install -y nodejs
    fi
  fi
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo '+ corepack enable && corepack prepare pnpm@10.14.0 --activate'
  else
    corepack enable
    corepack prepare pnpm@10.14.0 --activate
  fi
}

install_rust() {
  if [[ -x /root/.cargo/bin/rustc ]]; then
    info "Rust already installed"
    # shellcheck disable=SC1091
    source /root/.cargo/env 2>/dev/null || true
    return 0
  fi
  info "Installing Rust 1.98 via rustup"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo '+ curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.98.0'
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
    # shellcheck disable=SC1091
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
    die "git clone failed. For a private repo export FPS_GITHUB_TOKEN (a GitHub PAT with contents:read)."
  fi
  git -C "${SRC_DIR}" remote set-url origin "${FPS_GIT_URL}"
  unset FPS_GITHUB_TOKEN
  rm -f /root/.fps-github-token /etc/fps-guest.env
}

build_fps() {
  if [[ "${SKIP_BUILD}" -eq 1 ]]; then
    info "Skipping cargo/pnpm build (FPS_SKIP_BUILD=1)"
    return 0
  fi
  # shellcheck disable=SC1091
  source /root/.cargo/env 2>/dev/null || true
  info "Building fps-control-plane and fps (release). This takes a while."
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ cargo build --release -p fps-control-plane -p fps"
    echo "+ pnpm install && pnpm --filter @fps/web build"
    return 0
  fi
  cd "${SRC_DIR}"
  export CARGO_TERM_COLOR=always
  # Thin LTO on a 4–8 GB LXC OOMs. The installer binary does not need it.
  export CARGO_PROFILE_RELEASE_LTO=false
  export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
  cargo build --release -p fps-control-plane -p fps
  pnpm install --frozen-lockfile || pnpm install
  pnpm --filter @fps/web build
}

install_binaries() {
  info "Installing binaries and web UI under ${FPS_PREFIX}"
  local current="${FPS_PREFIX}/current"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ install binaries → ${current}"
    echo "+ copy apps/web/dist → ${FPS_WEB_ROOT}"
    return 0
  fi
  mkdir -p "${current}" "${FPS_WEB_ROOT}" "${FPS_DATA_DIR}" /etc/fps /etc/systemd/system
  if [[ -x "${SRC_DIR}/target/release/fps-control-plane" ]]; then
    install -m 0755 "${SRC_DIR}/target/release/fps-control-plane" "${current}/fps-control-plane"
  fi
  if [[ -x "${SRC_DIR}/target/release/fps" ]]; then
    install -m 0755 "${SRC_DIR}/target/release/fps" "${current}/fps"
    ln -sfn "${current}/fps" /usr/local/bin/fps
  fi
  if [[ -d "${SRC_DIR}/apps/web/dist" ]]; then
    rm -rf "${FPS_WEB_ROOT}"
    cp -a "${SRC_DIR}/apps/web/dist" "${FPS_WEB_ROOT}"
  fi
  if [[ -f "${SRC_DIR}/deploy/systemd/fps-control-plane.service" ]]; then
    install -m 0644 "${SRC_DIR}/deploy/systemd/fps-control-plane.service" \
      /etc/systemd/system/fps-control-plane.service
  fi
  # Reclaim build tree space; rustc stays for later source updates.
  rm -rf "${SRC_DIR}/target" "${SRC_DIR}/node_modules" "${SRC_DIR}/apps/web/node_modules" || true
}

setup_mariadb() {
  info "Configuring MariaDB"
  local db_pass="${FPS_DB_PASSWORD:-}"
  if [[ -z "${db_pass}" ]]; then
    db_pass="$(openssl rand -base64 24 | tr -d '/+=' | head -c 28)"
  fi
  FPS_DB_PASSWORD="${db_pass}"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo '+ systemctl enable --now mariadb'
    echo '+ CREATE DATABASE fps; CREATE USER fps@127.0.0.1 …'
    return 0
  fi
  systemctl enable --now mariadb
  mysql --protocol=socket <<SQL
CREATE DATABASE IF NOT EXISTS fps CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
CREATE USER IF NOT EXISTS 'fps'@'127.0.0.1' IDENTIFIED BY '${db_pass}';
ALTER USER 'fps'@'127.0.0.1' IDENTIFIED BY '${db_pass}';
GRANT ALL PRIVILEGES ON fps.* TO 'fps'@'127.0.0.1';
FLUSH PRIVILEGES;
SQL
}

write_env_and_user() {
  local ip master
  ip="$(public_base)"
  ip="${ip:-127.0.0.1}"
  master="${FPS_MASTER_KEY:-}"
  if [[ -z "${master}" ]]; then
    master="$(openssl rand -hex 32)"
  fi
  info "Writing /etc/fps/control-plane.env"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "+ useradd fps; write env (PUBLIC_URL=http://${ip}:47890)"
    return 0
  fi
  if ! getent passwd fps >/dev/null; then
    useradd --system --home "${FPS_DATA_DIR}" --shell /usr/sbin/nologin fps
  fi
  chown -R fps:fps "${FPS_DATA_DIR}" "${FPS_PREFIX}" "${FPS_WEB_ROOT}"
  umask 077
  cat >/etc/fps/control-plane.env <<EOF
FPS_DATABASE_URL=mysql://fps:${FPS_DB_PASSWORD}@127.0.0.1:3306/fps
FPS_MASTER_KEY=${master}
FPS_HTTP_BIND=${FPS_HTTP_BIND}
FPS_NODE_BIND=${FPS_NODE_BIND}
FPS_WEB_BIND=${FPS_WEB_BIND}
FPS_WEB_ROOT=${FPS_WEB_ROOT}
FPS_PUBLIC_URL=http://${ip}:47890
FPS_CORS_ORIGINS=http://${ip}:47880,http://${ip}:47890
FPS_DATA_DIR=${FPS_DATA_DIR}
FPS_ALLOW_INSECURE_HTTP=true
FPS_LOG_FORMAT=json
EOF
  chmod 0600 /etc/fps/control-plane.env
  chown root:fps /etc/fps/control-plane.env
}

start_service() {
  info "Starting fps-control-plane.service"
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo '+ systemctl enable --now fps-control-plane.service'
    echo '+ touch /var/lib/fps/.provision-complete'
    return 0
  fi
  systemctl daemon-reload
  systemctl enable --now fps-control-plane.service
  mkdir -p "${FPS_DATA_DIR}"
  date -u +'%Y-%m-%dT%H:%M:%SZ' >"${FPS_DATA_DIR}/.provision-complete"
  local ip
  ip="$(public_base)"
  cat <<EOF

FPS control plane is up.

  Web UI   http://${ip:-127.0.0.1}:47880
  API      http://${ip:-127.0.0.1}:47890
  Node mTLS  ${ip:-127.0.0.1}:47891

Open the web UI and create the owner account (password at least 12 characters).
Then create an enrollment token and run the game-host installer on Homer.
EOF
}

main() {
  if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=1
    shift
  fi
  if [[ "${EUID}" -ne 0 && "${DRY_RUN}" -ne 1 ]]; then
    die "guest bootstrap must run as root"
  fi
  install_packages
  install_rust
  clone_source
  build_fps
  install_binaries
  setup_mariadb
  write_env_and_user
  start_service
}

main "$@"
