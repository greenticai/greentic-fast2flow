#!/usr/bin/env python3
import argparse
import hashlib
import json
import shutil
from pathlib import Path


def load_json(path: Path) -> dict:
    return json.loads(path.read_text())


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description="Package Fast2Flow external WASM components for OCI publication.")
    parser.add_argument("--repo-root", default=".", help="Fast2Flow repository root")
    parser.add_argument("--output-dir", default="target/component-packages", help="Package output directory")
    parser.add_argument("--check", action="store_true", help="Validate required artifacts without writing packages")
    args = parser.parse_args()

    repo_root = Path(args.repo_root).resolve()
    output_dir = (repo_root / args.output_dir).resolve()
    manifest = load_json(repo_root / "components" / "manifest.json")
    packages = []

    for component in manifest.get("components", []):
        crate_root = repo_root / component["crate_path"]
        component_manifest_path = crate_root / "component.manifest.json"
        wasm_path = repo_root / component["wasm_artifact"]
        if not component_manifest_path.is_file():
            raise SystemExit(f"missing component manifest: {component_manifest_path}")
        if not wasm_path.is_file():
            raise SystemExit(f"missing wasm artifact: {wasm_path}")
        component_manifest = load_json(component_manifest_path)
        if component_manifest.get("id") != component["package_id"]:
            raise SystemExit(
                f"manifest id mismatch for {component['component_id']}: "
                f"{component_manifest.get('id')} != {component['package_id']}"
            )
        package_record = {
            "component_id": component["component_id"],
            "package_id": component["package_id"],
            "kind": component["kind"],
            "runtime": component["runtime"],
            "oci_ref": component["oci_ref"],
            "artifact_type": manifest["artifact_type"],
            "layers": {
                "component_wasm": {
                    "path": "component.wasm",
                    "media_type": manifest["wasm_layer_media_type"],
                    "sha256": sha256_file(wasm_path),
                },
                "component_manifest": {
                    "path": "component.manifest.json",
                    "media_type": manifest["manifest_layer_media_type"],
                    "sha256": sha256_file(component_manifest_path),
                },
            },
        }
        packages.append(package_record)
        if not args.check:
            package_dir = output_dir / component["package_id"]
            package_dir.mkdir(parents=True, exist_ok=True)
            shutil.copy2(wasm_path, package_dir / "component.wasm")
            shutil.copy2(component_manifest_path, package_dir / "component.manifest.json")
            (package_dir / "package.json").write_text(json.dumps(package_record, indent=2, sort_keys=True) + "\n")

    if not args.check:
        output_dir.mkdir(parents=True, exist_ok=True)
        (output_dir / "index.json").write_text(json.dumps({
            "manifest_version": manifest["manifest_version"],
            "package_base_ref": manifest["package_base_ref"],
            "packages": packages,
        }, indent=2, sort_keys=True) + "\n")
    print(f"validated {len(packages)} component package(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
