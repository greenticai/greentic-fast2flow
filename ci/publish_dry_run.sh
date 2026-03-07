#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

list_publishable_crates() {
  cargo metadata --no-deps --format-version 1 | python3 -c '
import json,sys

data=json.load(sys.stdin)
packages={pkg["id"]:pkg for pkg in data.get("packages",[])}
for pkg_id in data.get("workspace_members",[]):
    pkg=packages.get(pkg_id)
    if not pkg:
        continue
    publish = pkg.get("publish")
    if publish is False or publish == []:
        continue
    print(pkg["name"])
'
}

header() {
  printf '\n==> %s\n' "$1"
}

if ! crates="$(list_publishable_crates)"; then
  echo "Failed to determine publishable crates"
  exit 1
fi

if [ -n "$crates" ]; then
  header "publishable crates are not allowed for this closed-source repository"
  echo "$crates"
  echo "Set publish = false for all workspace crates before releasing."
  exit 1
fi

header "No publishable crates detected (crates.io publishing disabled)"
