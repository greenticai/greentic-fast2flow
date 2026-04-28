#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

HOST_PACKAGES=(
  -p fast2flow-contracts
  -p fast2flow-core
  -p fast2flow-strategy
  -p fast2flow-strategy-phase1
  -p fast2flow-indexer
  -p fast2flow-hooks
  -p fast2flow-llm
  -p fast2flow-llm-openai
  -p fast2flow-llm-ollama
  -p fast2flow-routing-gtpack
  -p fast2flow-bundle
  -p greentic-fast2flow
)

COMPONENT_PACKAGES=(
  -p fast2flow-component-indexer
  -p fast2flow-component-matcher
  -p fast2flow-component-router
)

step() {
  printf '\n==> %s\n' "$1"
}

step "cargo fmt --all -- --check"
cargo fmt --all -- --check

step "cargo clippy host crates --all-targets --all-features -- -D warnings"
cargo clippy "${HOST_PACKAGES[@]}" --all-targets --all-features -- -D warnings

step "cargo clippy WASM components --target wasm32-wasip2 -- -D warnings"
cargo clippy "${COMPONENT_PACKAGES[@]}" --target wasm32-wasip2 -- -D warnings

step "cargo test host crates --all-features"
cargo test "${HOST_PACKAGES[@]}" --all-features

step "cargo build host crates --all-features"
cargo build "${HOST_PACKAGES[@]}" --all-features

step "cargo build WASM components --target wasm32-wasip2 --release"
cargo build "${COMPONENT_PACKAGES[@]}" --target wasm32-wasip2 --release

step "cargo doc host crates --no-deps --all-features"
cargo doc "${HOST_PACKAGES[@]}" --no-deps --all-features

step "build fast2flow.gtpack bundle"
dist_dir="$repo_root/dist"
bash ci/build_gtpack.sh "$dist_dir"

step "validate gtpack wizard replay + dependency metadata"
bash ci/test_gtpack_replay.sh

step "verify crates.io publishing is disabled"
bash ci/publish_dry_run.sh

step "local checks passed"
