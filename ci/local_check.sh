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

step "build fast2flow.gtpack bundle"
dist_dir="$repo_root/dist"
bash ci/build_gtpack.sh "$dist_dir"

step "validate gtpack wizard replay + dependency metadata"
bash ci/test_gtpack_replay.sh

step "verify crates.io publishing is disabled"
bash ci/publish_dry_run.sh

step "local checks passed"
