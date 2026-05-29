#!/usr/bin/env bash
set -euo pipefail

# Publish every crate in the publish_dry_run.sh allowlist to crates.io.
# Idempotent: a version that's already on crates.io is skipped rather
# than fatal, so a re-run after a partial publish doesn't blow up.
#
# Local: `CARGO_REGISTRY_TOKEN=<token> ci/publish_crates.sh`
# CI:    same, with the secret wired through.
# Test:  `DRY_RUN=1 ci/publish_crates.sh` (no token required).

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# Keep in sync with ALLOWED_PUBLISHABLE_CRATES in publish_dry_run.sh.
PUBLISHABLE_CRATES=(
  greentic-intent
)

dry_run_flag=""
if [ "${DRY_RUN:-0}" = "1" ]; then
  dry_run_flag="--dry-run"
  echo "DRY_RUN=1 set; running cargo publish --dry-run (no upload)."
fi

if [ -z "${CARGO_REGISTRY_TOKEN:-}" ] && [ -z "$dry_run_flag" ]; then
  echo "CARGO_REGISTRY_TOKEN not set; refusing to attempt a real publish."
  echo "Either export the token or pass DRY_RUN=1."
  exit 1
fi

publish_one() {
  local crate="$1"
  echo
  echo "==> Publishing ${crate}"
  local output
  if output=$(cargo publish -p "$crate" $dry_run_flag 2>&1); then
    echo "$output"
    return 0
  fi
  if grep -qE "crate version .* is already uploaded" <<<"$output"; then
    echo "$output"
    echo "==> ${crate}: version already on crates.io; skipping."
    return 0
  fi
  echo "$output" >&2
  return 1
}

for crate in "${PUBLISHABLE_CRATES[@]}"; do
  publish_one "$crate"
done

echo
echo "All crates processed."
