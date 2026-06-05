#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR=$(cd -- "$(dirname "$0")/.." && pwd)
PACKAGE_DIR="${1:-$ROOT_DIR/target/component-packages}"
if ! command -v oras >/dev/null 2>&1; then
  echo "oras is required only for publishing component packages" >&2
  exit 1
fi
if [ ! -f "$PACKAGE_DIR/index.json" ]; then
  echo "missing package index: $PACKAGE_DIR/index.json" >&2
  echo "run: bash scripts/build_components.sh && bash scripts/package_components.sh" >&2
  exit 1
fi
python3 - "$PACKAGE_DIR/index.json" <<'PYREFS' | while IFS=$'\t' read -r package_id oci_ref artifact_type wasm_media manifest_media; do
import json
import sys
from pathlib import Path
index = json.loads(Path(sys.argv[1]).read_text())
for package in index.get("packages", []):
    print("\t".join([
        package["package_id"],
        package["oci_ref"].removeprefix("oci://"),
        package["artifact_type"],
        package["layers"]["component_wasm"]["media_type"],
        package["layers"]["component_manifest"]["media_type"],
    ]))
PYREFS
  component_dir="$PACKAGE_DIR/$package_id"
  echo "Publishing $package_id -> $oci_ref"
  (
    cd "$component_dir"
    oras push \
      --artifact-type "$artifact_type" \
      "$oci_ref" \
      "component.wasm:$wasm_media" \
      "component.manifest.json:$manifest_media" \
      "package.json:application/vnd.greentic.component.package.v1+json"
  )
done
