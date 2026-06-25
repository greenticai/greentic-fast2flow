#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR=$(cd -- "$(dirname "$0")/.." && pwd)
python3 "$ROOT_DIR/scripts/package_components.py" --repo-root "$ROOT_DIR" "$@"
