#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

step() {
  printf '\n==> %s\n' "$1"
}

step "cargo fmt --all -- --check"
cargo fmt --all -- --check

step "cargo clippy --all-targets --all-features -- -D warnings"
cargo clippy --all-targets --all-features -- -D warnings

step "cargo test --all-features"
cargo test --all-features

step "cargo build --all-features"
cargo build --all-features

step "cargo doc --no-deps --all-features"
cargo doc --no-deps --all-features

step "cargo package + publish dry-run checks"
if [ "${CI:-}" = "true" ]; then
  bash ci/publish_dry_run.sh
else
  bash ci/publish_dry_run.sh --allow-dirty
fi

step "local checks passed"
