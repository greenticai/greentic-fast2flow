#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

out_dir="${1:-$repo_root/dist}"
mkdir -p "$out_dir"

version="$(python3 - <<'PY'
import tomllib
with open("Cargo.toml", "rb") as f:
    root = tomllib.load(f)
print(root["workspace"]["package"]["version"])
PY
)"

work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

pack_id="fast2flow"
pack_dir="$work_dir"
wizard_answers="$repo_root/ci/wizard/finalize.answers.json"
flow_wizard_answers="$repo_root/ci/wizard/flow.answers.json"
schema_template="$repo_root/ci/templates/routing-hook-fast2flow-config.schema.json"
component_wasm_src="${FAST2FLOW_COMPONENT_WASM:-}"
allow_placeholder="${FAST2FLOW_ALLOW_PLACEHOLDER_WASM:-0}"
export_pack_source="${FAST2FLOW_EXPORT_PACK_SOURCE:-}"

greentic-pack new --dir "$work_dir" "$pack_id"
sed -i "s/^version: .*/version: ${version}/" "$pack_dir/pack.yaml"
# Declare control-chain capability dependency so deployment fails clearly if missing.
awk '
BEGIN { replaced = 0 }
$0 == "dependencies: []" {
  print "dependencies:"
  print "- alias: control_chain"
  print "  pack_id: routing.ingress.control.chain"
  print "  version_req: ^0.1.0"
  print "  required_capabilities:"
  print "  - greentic.cap.ingress.control.v1"
  replaced = 1
  next
}
{ print }
END {
  if (replaced == 0) {
    exit 2
  }
}
' "$pack_dir/pack.yaml" > "$pack_dir/pack.yaml.tmp"
mv "$pack_dir/pack.yaml.tmp" "$pack_dir/pack.yaml"

(
  cd "$pack_dir"
  greentic-pack wizard run \
    --answers "$wizard_answers" \
    --emit-answers "$work_dir/wizard.finalize.applied.answers.json"
  greentic-flow wizard . --answers-file "$flow_wizard_answers"
)

mkdir -p "$pack_dir/components"
if [ -n "$component_wasm_src" ] && [ ! -f "$component_wasm_src" ]; then
  echo "FAST2FLOW_COMPONENT_WASM points to a missing file: $component_wasm_src" >&2
  exit 1
fi

if [ -z "$component_wasm_src" ]; then
  echo "FAST2FLOW_COMPONENT_WASM not set; building fast2flow-routing-gtpack wasm" >&2
  cargo build -p fast2flow-routing-gtpack --lib --target wasm32-wasip2 --release
  candidate="$repo_root/target/wasm32-wasip2/release/fast2flow_routing_gtpack.wasm"
  if [ -f "$candidate" ]; then
    component_wasm_src="$candidate"
  fi
fi

if [ -z "$component_wasm_src" ]; then
  if [ "$allow_placeholder" = "1" ] || [ "$allow_placeholder" = "true" ]; then
    echo "warning: FAST2FLOW_COMPONENT_WASM not set; using placeholder wasm" >&2
    printf '\x00\x61\x73\x6d\x01\x00\x00\x00' > "$pack_dir/components/fast2flow.wasm"
  else
    cat >&2 <<'EOF'
missing FAST2FLOW_COMPONENT_WASM for pack build.

Auto-build failed to produce a wasm and no explicit path was provided.

Provide a real wasm file explicitly, for example:
  FAST2FLOW_COMPONENT_WASM=/abs/path/to/fast2flow.wasm bash ci/build_gtpack.sh dist

For local-only fallback (non-production), set:
  FAST2FLOW_ALLOW_PLACEHOLDER_WASM=1
EOF
    exit 1
  fi
else
  cp "$component_wasm_src" "$pack_dir/components/fast2flow.wasm"
fi

greentic-pack components --in "$pack_dir"

# `greentic-pack components` currently defaults world/version for this wasm to
# root/component placeholders. Normalize to the Fast2Flow routing component identity.
awk '
BEGIN { in_comp = 0; saw_version = 0; saw_world = 0 }
$0 == "- id: fast2flow" { in_comp = 1; print; next }
in_comp && $0 ~ /^- id: / { in_comp = 0 }
in_comp && $0 ~ /^  version: / {
  print "  version: " VERSION
  saw_version = 1
  next
}
in_comp && $0 ~ /^  world: / {
  print "  world: greentic:fast2flow/fast2flow-routing"
  saw_world = 1
  next
}
{ print }
END {
  if (saw_version == 0 || saw_world == 0) {
    exit 2
  }
}
' VERSION="$version" "$pack_dir/pack.yaml" > "$pack_dir/pack.yaml.tmp"
mv "$pack_dir/pack.yaml.tmp" "$pack_dir/pack.yaml"

greentic-pack add-extension provider --pack-dir "$pack_dir" --id "$pack_id" --kind routing-hook
greentic-pack add-extension capability \
  --pack-dir "$pack_dir" \
  --offer-id routing-hook-default \
  --cap-id routing-hook \
  --component-ref fast2flow \
  --op run

mkdir -p "$pack_dir/schemas/routing-hook/$pack_id"
cp "$schema_template" "$pack_dir/schemas/routing-hook/$pack_id/config.schema.json"

(
  cd "$pack_dir"
  greentic-pack build --in . --allow-pack-schema
)

built_gtpack="$(find "$pack_dir/dist" -maxdepth 1 -type f -name '*.gtpack' | head -n1)"
if [ -z "${built_gtpack}" ]; then
  echo "failed to locate built .gtpack under $pack_dir/dist" >&2
  exit 1
fi

(
  cd "$pack_dir"
  greentic-pack doctor "$built_gtpack"
)

if [ -n "$export_pack_source" ]; then
  rm -rf "$export_pack_source"
  mkdir -p "$export_pack_source"
  cp -R "$pack_dir"/. "$export_pack_source"/
fi

cp "$built_gtpack" "$out_dir/fast2flow.gtpack"
sha256sum "$out_dir/fast2flow.gtpack" > "$out_dir/fast2flow.gtpack.sha256"

echo "wrote bundle artifacts to $out_dir"
