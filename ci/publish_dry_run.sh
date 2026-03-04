#!/usr/bin/env bash
set -euo pipefail

ALLOW_DIRTY=0
if [ "${1:-}" = "--allow-dirty" ]; then
  ALLOW_DIRTY=1
fi

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

if [ -z "$crates" ]; then
  header "No publishable crates detected"
  exit 0
fi

for crate in $crates; do
  header "Packaging checks for $crate"

  if [ "$ALLOW_DIRTY" -eq 1 ]; then
    cargo package --no-verify -p "$crate" --allow-dirty
  else
    cargo package --no-verify -p "$crate"
  fi

  if [ "$ALLOW_DIRTY" -eq 1 ]; then
    cargo package -p "$crate" --allow-dirty
    cargo publish -p "$crate" --dry-run --allow-dirty
    package_list="$(cargo package -p "$crate" --allow-dirty --list)"
  else
    cargo package -p "$crate"
    cargo publish -p "$crate" --dry-run
    package_list="$(cargo package -p "$crate" --list)"
  fi

  echo "$package_list" | grep -Eq '^Cargo.toml$' || {
    echo "cargo package list for $crate does not include Cargo.toml"
    exit 1
  }
  echo "$package_list" | grep -Eq '^src/' || {
    echo "cargo package list for $crate does not include src/ files"
    exit 1
  }
  echo "$package_list" | grep -Eq '^README' || {
    echo "cargo package list for $crate does not include a README"
    exit 1
  }
  echo "$package_list" | grep -Eq '^LICENSE' || {
    echo "cargo package list for $crate does not include a LICENSE"
    exit 1
  }
done
