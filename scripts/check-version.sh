#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

EXPECTED_VERSION="${1:-}"
SEMVER_PATTERN='^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$'

validate_semver() {
  local source_name="$1"
  local source_version="$2"
  if [[ ! "${source_version}" =~ ${SEMVER_PATTERN} ]]; then
    echo "invalid semver: ${source_name}=${source_version}" >&2
    return 1
  fi
  return 0
}

read_cargo_package_version() {
  awk '
    BEGIN { in_package = 0 }
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ && in_package { in_package = 0 }
    in_package && /^version = "/ {
      gsub(/^version = "/, "", $0)
      gsub(/"$/, "", $0)
      print
      exit
    }
  ' src-tauri/Cargo.toml
}

read_cargo_lock_pomodoro_version() {
  awk '
    BEGIN { pending = 0 }
    /^\[\[package\]\]$/ { pending = 0; next }
    /^name = "pomodoro"$/ { pending = 1; next }
    pending && /^version = "/ {
      gsub(/^version = "/, "", $0)
      gsub(/"$/, "", $0)
      print
      exit
    }
  ' src-tauri/Cargo.lock
}

PACKAGE_JSON_VERSION="$(node -p "require('./package.json').version")"
TAURI_CONF_VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
PACKAGE_LOCK_VERSION="$(node -p "require('./package-lock.json').version")"
PACKAGE_LOCK_ROOT_VERSION="$(node -p "const l=require('./package-lock.json'); (l.packages && l.packages[''] && l.packages[''].version) || ''")"
CARGO_TOML_VERSION="$(read_cargo_package_version)"
CARGO_LOCK_VERSION="$(read_cargo_lock_pomodoro_version)"

declare -a VERSION_SOURCES=(
  "package.json:${PACKAGE_JSON_VERSION}"
  "src-tauri/tauri.conf.json:${TAURI_CONF_VERSION}"
  "package-lock.json:${PACKAGE_LOCK_VERSION}"
  "package-lock.json#packages[\"\"]:${PACKAGE_LOCK_ROOT_VERSION}"
  "src-tauri/Cargo.toml:${CARGO_TOML_VERSION}"
  "src-tauri/Cargo.lock(pomodoro):${CARGO_LOCK_VERSION}"
)

REFERENCE_VERSION="${PACKAGE_JSON_VERSION}"
if [[ -n "${EXPECTED_VERSION}" ]]; then
  if ! validate_semver "expected" "${EXPECTED_VERSION}"; then
    exit 1
  fi
  REFERENCE_VERSION="${EXPECTED_VERSION}"
fi

HAS_MISMATCH=0
for entry in "${VERSION_SOURCES[@]}"; do
  source_name="${entry%%:*}"
  source_version="${entry#*:}"
  if ! validate_semver "${source_name}" "${source_version}"; then
    HAS_MISMATCH=1
    continue
  fi
  if [[ "${source_version}" != "${REFERENCE_VERSION}" ]]; then
    echo "version mismatch: ${source_name}=${source_version}, expected=${REFERENCE_VERSION}" >&2
    HAS_MISMATCH=1
  fi
done

if [[ "${HAS_MISMATCH}" -ne 0 ]]; then
  exit 1
fi

echo "version check passed: ${REFERENCE_VERSION}"
