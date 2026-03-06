#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

out_dir="$tmp_dir/out"
export_dir="$tmp_dir/pack-src"
mkdir -p "$out_dir"

FAST2FLOW_EXPORT_PACK_SOURCE="$export_dir" bash ci/build_gtpack.sh "$out_dir"

[ -f "$out_dir/fast2flow.gtpack" ]
[ -f "$out_dir/fast2flow.gtpack.sha256" ]
[ -f "$export_dir/pack.yaml" ]

pack_yaml="$export_dir/pack.yaml"

# Dependency requirement: fail clearly when control-chain capability is not available.
rg -q '^dependencies:$' "$pack_yaml"
rg -q '^- alias: control_chain$' "$pack_yaml"
rg -q '^  pack_id: routing\.ingress\.control\.chain$' "$pack_yaml"
rg -q '^  version_req: \^0\.1\.0$' "$pack_yaml"
rg -q '^  required_capabilities:$' "$pack_yaml"
rg -q '^  - greentic\.cap\.ingress\.control\.v1$' "$pack_yaml"

# Component metadata normalization to Fast2Flow world identity.
rg -q '^- id: fast2flow$' "$pack_yaml"
rg -q '^  world: greentic:fast2flow/fast2flow-routing$' "$pack_yaml"

# Wizard replay trace should be present in exported source.
[ -f "$export_dir/wizard.finalize.applied.answers.json" ]

# Generated schema should be included by the scripted flow.
[ -f "$export_dir/schemas/routing-hook/fast2flow/config.schema.json" ]

echo "gtpack replay test passed"
