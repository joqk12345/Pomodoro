#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

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

normalize_github_url() {
  local remote_url="$1"
  if [[ "${remote_url}" =~ ^git@github\.com:(.+)\.git$ ]]; then
    printf 'https://github.com/%s' "${BASH_REMATCH[1]}"
    return 0
  fi

  if [[ "${remote_url}" =~ ^https://github\.com/(.+)\.git$ ]]; then
    printf 'https://github.com/%s' "${BASH_REMATCH[1]}"
    return 0
  fi

  if [[ "${remote_url}" =~ ^https://github\.com/.+[^/]$ ]]; then
    printf '%s' "${remote_url}"
    return 0
  fi

  printf '%s' "${remote_url}"
}

if [[ $# -ne 1 ]]; then
  echo "Usage: ./scripts/release.sh <version>" >&2
  echo "Example: ./scripts/release.sh 0.6.0" >&2
  exit 1
fi

VERSION="$1"
VERSION_TAG="v${VERSION}"
SEMVER_PATTERN='^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$'

[[ "${VERSION}" =~ ${SEMVER_PATTERN} ]] || fail "Invalid semver version '${VERSION}'"

require_cmd git
require_cmd node
require_cmd npm
require_cmd cargo

git rev-parse --is-inside-work-tree >/dev/null 2>&1 || fail "Not inside a git repository"

CURRENT_BRANCH="$(git branch --show-current)"
[[ -n "${CURRENT_BRANCH}" ]] || fail "Unable to determine current branch"

if [[ -n "$(git status --porcelain)" ]]; then
  fail "Working tree is not clean. Commit or stash changes before running release."
fi

if git rev-parse -q --verify "refs/tags/${VERSION_TAG}" >/dev/null 2>&1; then
  fail "Tag ${VERSION_TAG} already exists locally"
fi

CURRENT_VERSION="$(node -p "require('./package.json').version")"
if [[ "${CURRENT_VERSION}" == "${VERSION}" ]]; then
  fail "Version ${VERSION} is already the current version"
fi

REMOTE_URL="$(git remote get-url origin 2>/dev/null || true)"
if [[ -z "${REMOTE_URL}" ]]; then
  fail "Remote 'origin' is not configured"
fi

REPO_URL="$(normalize_github_url "${REMOTE_URL}")"

echo -e "${GREEN}=== Pomodoro Release Script ===${NC}"
echo "Repository: ${REPO_URL}"
echo "Branch: ${CURRENT_BRANCH}"
echo "Current version: ${CURRENT_VERSION}"
echo "Next version: ${VERSION}"
echo

if [[ ! -d .github/workflows ]]; then
  warn "No .github/workflows directory found."
  warn "Pushing ${VERSION_TAG} will not trigger GitHub Actions until workflows are added."
  echo
fi

read -r -p "Do you want to continue? (y/n) " REPLY
if [[ ! "${REPLY}" =~ ^[Yy]$ ]]; then
  echo "Release cancelled"
  exit 1
fi

log "Syncing version across release files"
./scripts/sync-version.sh "${VERSION}"
ok "Version sync completed"

log "Running frontend build"
npm run build
ok "Frontend build passed"

log "Running cargo check"
(
  cd src-tauri
  cargo check
)
ok "Cargo check passed"

log "Committing version changes"
git add package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock

if [[ -f CHANGELOG.md ]]; then
  if ! git diff --quiet -- CHANGELOG.md || ! git diff --cached --quiet -- CHANGELOG.md; then
    git add CHANGELOG.md
  fi
fi

git commit -m "chore: bump version to ${VERSION}"
ok "Created commit for ${VERSION}"

log "Creating git tag ${VERSION_TAG}"
git tag -a "${VERSION_TAG}" -m "Release ${VERSION_TAG}"
ok "Created tag ${VERSION_TAG}"

log "Pushing branch ${CURRENT_BRANCH}"
git push origin "${CURRENT_BRANCH}"

log "Pushing tag ${VERSION_TAG}"
git push origin "${VERSION_TAG}"

echo
ok "Release ${VERSION_TAG} pushed successfully"
echo "Repository: ${REPO_URL}"
echo "Releases: ${REPO_URL}/releases"
echo "Actions: ${REPO_URL}/actions"
