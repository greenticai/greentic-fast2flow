#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR=$(cd -- "$(dirname "$0")/.." && pwd)
cd "$ROOT_DIR"
cargo build -p fast2flow-component-router --target wasm32-wasip2 --release
