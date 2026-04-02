#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: ./scripts/sync-version.sh <version>" >&2
  exit 1
fi

VERSION="$1"
SEMVER_PATTERN='^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$'
if [[ ! "${VERSION}" =~ ${SEMVER_PATTERN} ]]; then
  echo "Invalid semver format: ${VERSION}" >&2
  echo "Examples: 0.6.0, 0.6.0-rc.1, 0.6.0+build.1" >&2
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

node - "${VERSION}" <<'NODE'
const fs = require('node:fs');

const version = process.argv[2];
const updateJsonVersion = (file, updateRootPackage = false) => {
  const data = JSON.parse(fs.readFileSync(file, 'utf8'));
  data.version = version;
  if (updateRootPackage && data.packages && data.packages['']) {
    data.packages[''].version = version;
  }
  fs.writeFileSync(file, `${JSON.stringify(data, null, 2)}\n`);
};

updateJsonVersion('package.json');
updateJsonVersion('src-tauri/tauri.conf.json');
updateJsonVersion('package-lock.json', true);
NODE

awk -v version="${VERSION}" '
  BEGIN { in_package = 0; updated = 0 }
  /^\[package\]$/ { in_package = 1; print; next }
  /^\[/ && in_package { in_package = 0 }
  in_package && !updated && /^version = "/ {
    print "version = \"" version "\""
    updated = 1
    next
  }
  { print }
  END {
    if (!updated) {
      print "failed to update src-tauri/Cargo.toml [package].version" > "/dev/stderr"
      exit 1
    }
  }
' src-tauri/Cargo.toml > src-tauri/Cargo.toml.tmp
mv src-tauri/Cargo.toml.tmp src-tauri/Cargo.toml

awk -v version="${VERSION}" '
  BEGIN { in_pomodoro = 0; updated = 0 }
  /^\[\[package\]\]$/ { in_pomodoro = 0; print; next }
  /^name = "pomodoro"$/ { in_pomodoro = 1; print; next }
  in_pomodoro && !updated && /^version = "/ {
    print "version = \"" version "\""
    updated = 1
    in_pomodoro = 0
    next
  }
  { print }
  END {
    if (!updated) {
      print "failed to update src-tauri/Cargo.lock pomodoro package version" > "/dev/stderr"
      exit 1
    }
  }
' src-tauri/Cargo.lock > src-tauri/Cargo.lock.tmp
mv src-tauri/Cargo.lock.tmp src-tauri/Cargo.lock

./scripts/check-version.sh "${VERSION}"
