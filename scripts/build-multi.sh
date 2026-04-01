#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

HOST_OS="$(uname -s)"
HOST_ARCH="$(uname -m)"
SKIP_CHECKS=0
INSTALL_TARGETS=0
PRESET="host"
declare -a TARGETS=()
declare -a BUILT_DIRS=()

print_usage() {
  cat <<'EOF'
Usage:
  ./scripts/build-multi.sh [options]

Options:
  --preset <name>         Build preset: host | mac
  --targets <csv>         Explicit Rust target triples, comma separated
  --install-targets       Auto-run rustup target add for missing targets
  --skip-checks           Skip npm build and cargo check preflight
  -h, --help              Show this help

Examples:
  ./scripts/build-multi.sh
  ./scripts/build-multi.sh --preset mac
  ./scripts/build-multi.sh --targets aarch64-apple-darwin,x86_64-apple-darwin

Notes:
  - host: build the current host platform bundle
  - mac: on macOS, build both Apple Silicon and Intel bundles
  - Linux / Windows bundles generally need matching host runners or CI
EOF
}

log() {
  printf "${BLUE}==>${NC} %s\n" "$1"
}

ok() {
  printf "${GREEN}✓${NC} %s\n" "$1"
}

warn() {
  printf "${YELLOW}!${NC} %s\n" "$1"
}

fail() {
  printf "${RED}Error:${NC} %s\n" "$1" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "Missing required command: $1"
}

append_csv_targets() {
  local csv="$1"
  local item
  IFS=',' read -r -a parsed <<<"${csv}"
  for item in "${parsed[@]}"; do
    item="${item// /}"
    [[ -n "${item}" ]] && TARGETS+=("${item}")
  done
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --preset)
      [[ $# -ge 2 ]] || fail "--preset requires a value"
      PRESET="$2"
      shift 2
      ;;
    --targets)
      [[ $# -ge 2 ]] || fail "--targets requires a value"
      append_csv_targets "$2"
      shift 2
      ;;
    --install-targets)
      INSTALL_TARGETS=1
      shift
      ;;
    --skip-checks)
      SKIP_CHECKS=1
      shift
      ;;
    -h|--help)
      print_usage
      exit 0
      ;;
    *)
      fail "Unknown argument: $1"
      ;;
  esac
done

if [[ ${#TARGETS[@]} -eq 0 ]]; then
  case "${PRESET}" in
    host)
      ;;
    mac)
      [[ "${HOST_OS}" == "Darwin" ]] || fail "--preset mac can only run on macOS"
      TARGETS=("aarch64-apple-darwin" "x86_64-apple-darwin")
      ;;
    *)
      fail "Unsupported preset: ${PRESET}"
      ;;
  esac
fi

require_cmd npm
require_cmd cargo
require_cmd rustup
require_cmd rg

if [[ ${#TARGETS[@]} -gt 0 ]]; then
  case "${HOST_OS}" in
    Darwin)
      for target in "${TARGETS[@]}"; do
        [[ "${target}" == *apple-darwin ]] || fail "Target ${target} does not match host ${HOST_OS}. Use a matching runner or CI."
      done
      ;;
    Linux)
      for target in "${TARGETS[@]}"; do
        [[ "${target}" == *linux* ]] || fail "Target ${target} does not match host ${HOST_OS}. Use a matching runner or CI."
      done
      ;;
    MINGW*|MSYS*|CYGWIN*)
      for target in "${TARGETS[@]}"; do
        [[ "${target}" == *windows* ]] || fail "Target ${target} does not match Windows host. Use a matching runner or CI."
      done
      ;;
    *)
      fail "Unsupported host OS: ${HOST_OS}"
      ;;
  esac
fi

log "Pomodoro multi-target build"
echo "Host: ${HOST_OS} (${HOST_ARCH})"
if [[ ${#TARGETS[@]} -eq 0 ]]; then
  echo "Mode: host bundle"
else
  echo "Targets:"
  printf '  - %s\n' "${TARGETS[@]}"
fi
echo

if [[ ${SKIP_CHECKS} -eq 0 ]]; then
  log "Running frontend build"
  npm run build
  ok "Frontend build passed"

  log "Running cargo check"
  (
    cd src-tauri
    cargo check
  )
  ok "Cargo check passed"
else
  warn "Skipping preflight checks"
fi

ensure_target_installed() {
  local target="$1"
  if rustup target list --installed | rg -x "${target}" >/dev/null 2>&1; then
    return 0
  fi

  if [[ ${INSTALL_TARGETS} -eq 1 ]]; then
    log "Installing rust target ${target}"
    rustup target add "${target}"
    return 0
  fi

  fail "Rust target ${target} is not installed. Re-run with --install-targets or run: rustup target add ${target}"
}

run_tauri_build() {
  local target="${1:-}"
  if [[ -n "${target}" ]]; then
    log "Building target ${target}"
    ensure_target_installed "${target}"
    npm run tauri build -- --target "${target}"
    BUILT_DIRS+=("src-tauri/target/${target}/release/bundle")
  else
    log "Building host bundle"
    npm run tauri build
    BUILT_DIRS+=("src-tauri/target/release/bundle")
  fi
}

if [[ ${#TARGETS[@]} -eq 0 ]]; then
  run_tauri_build
else
  for target in "${TARGETS[@]}"; do
    run_tauri_build "${target}"
  done
fi

echo
ok "Build finished"
echo "Bundle output:"
printf '  - %s\n' "${BUILT_DIRS[@]}"

if [[ "${HOST_OS}" == "Darwin" && ${#TARGETS[@]} -gt 1 ]]; then
  echo
  warn "This script builds separate per-architecture macOS bundles."
  warn "If you need signed/notarized public DMGs, run the packaging/signing flow on CI or a dedicated release workflow."
fi
