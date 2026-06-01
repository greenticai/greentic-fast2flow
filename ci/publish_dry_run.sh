#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# Crates that are intentionally published to crates.io as standalone
# primitives. Each MUST override the workspace license + carry an
# explicit `publish = true`. Adding to this list is a deliberate
# release-policy decision.
ALLOWED_PUBLISHABLE_CRATES=(
  greentic-intent
)

list_violating_crates() {
  local allowlist
  allowlist=$(printf '%s\n' "${ALLOWED_PUBLISHABLE_CRATES[@]}")
  cargo metadata --no-deps --format-version 1 | ALLOWLIST="$allowlist" python3 -c '
import json,os,sys

allowlist={name for name in os.environ.get("ALLOWLIST","").splitlines() if name}
data=json.load(sys.stdin)
packages={pkg["id"]:pkg for pkg in data.get("packages",[])}
for pkg_id in data.get("workspace_members",[]):
    pkg=packages.get(pkg_id)
    if not pkg:
        continue
    publish = pkg.get("publish")
    if publish is False or publish == []:
        continue
    if pkg["name"] in allowlist:
        continue
    print(pkg["name"])
'
}

header() {
  printf '\n==> %s\n' "$1"
}

if ! crates="$(list_violating_crates)"; then
  echo "Failed to determine publishable crates"
  exit 1
fi

if [ -n "$crates" ]; then
  header "unexpected publishable crates outside the allowlist"
  echo "$crates"
  echo
  echo "Set publish = false for these crates, or add them to"
  echo "ALLOWED_PUBLISHABLE_CRATES at the top of this script if their"
  echo "release policy was intentionally changed."
  exit 1
fi

if [ ${#ALLOWED_PUBLISHABLE_CRATES[@]} -gt 0 ]; then
  header "Allow-listed crates published to crates.io"
  printf '  - %s\n' "${ALLOWED_PUBLISHABLE_CRATES[@]}"
else
  header "No publishable crates detected (crates.io publishing disabled)"
fi
