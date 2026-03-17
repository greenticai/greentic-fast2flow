#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
from pathlib import Path


TOOL_SCHEMA_URL = "https://raw.githubusercontent.com/greentic-biz/customers-tools/main/schemas/tool.schema.json"
DOC_SCHEMA_URL = "https://raw.githubusercontent.com/greentic-biz/customers-tools/main/schemas/doc.schema.json"
DOC_ID = "greentic-fast2flow-readme"
TOOL_ID = "greentic-fast2flow"
TOOL_NAME = "Greentic Fast2Flow"
TOOL_DESCRIPTION = "Commercial Fast2Flow routing CLI distributed from private GitHub releases."
DOC_TITLE = "Greentic Fast2Flow README"
DOC_DOWNLOAD_FILE_NAME = "greentic-fast2flow-guide.md"
DOC_DEFAULT_RELATIVE_PATH = "greentic-fast2flow/greentic-fast2flow-guide.md"
TARGETS = [
    ("linux", "x86_64", "x86_64-unknown-linux-gnu", "tar.gz"),
    ("linux", "aarch64", "aarch64-unknown-linux-gnu", "tar.gz"),
    ("macos", "x86_64", "x86_64-apple-darwin", "tar.gz"),
    ("macos", "aarch64", "aarch64-apple-darwin", "tar.gz"),
    ("windows", "x86_64", "x86_64-pc-windows-msvc", "zip"),
    ("windows", "aarch64", "aarch64-pc-windows-msvc", "zip"),
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate release manifests for Greentic Fast2Flow.")
    parser.add_argument("--artifacts-dir", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--repository", required=True)
    return parser.parse_args()


def read_sha256(sha_path: Path) -> str:
    line = sha_path.read_text(encoding="utf-8").strip()
    checksum = line.split()[0].lower()
    if len(checksum) != 64 or any(ch not in "0123456789abcdef" for ch in checksum):
        raise ValueError(f"invalid sha256 in {sha_path}")
    return checksum


def build_tool_manifest(artifacts_dir: Path, version: str, repository: str) -> dict:
    targets = []
    for os_name, arch, target_triple, archive_ext in TARGETS:
        archive_name = f"{TOOL_ID}-v{version}-{target_triple}.{archive_ext}"
        sha_name = f"{archive_name}.sha256"
        sha_path = artifacts_dir / sha_name
        if not sha_path.exists():
            raise FileNotFoundError(f"missing checksum file: {sha_path}")
        targets.append(
            {
                "os": os_name,
                "arch": arch,
                "url": f"https://github.com/{repository}/releases/download/v{version}/{archive_name}",
                "sha256": read_sha256(sha_path),
            }
        )

    return {
        "$schema": TOOL_SCHEMA_URL,
        "schema_version": "1",
        "id": TOOL_ID,
        "name": TOOL_NAME,
        "description": TOOL_DESCRIPTION,
        "install": {
            "type": "release-binary",
            "binary_name": TOOL_ID,
            "targets": targets,
        },
        "docs": [DOC_ID],
    }


def build_docs_manifest(repository: str) -> dict:
    return {
        "$schema": DOC_SCHEMA_URL,
        "schema_version": "1",
        "id": DOC_ID,
        "title": DOC_TITLE,
        "source": {
            "type": "download",
            "url": f"https://raw.githubusercontent.com/{repository}/master/README.md",
        },
        "download_file_name": DOC_DOWNLOAD_FILE_NAME,
        "default_relative_path": DOC_DEFAULT_RELATIVE_PATH,
    }


def write_json(path: Path, payload: dict) -> None:
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    artifacts_dir = args.artifacts_dir
    if not artifacts_dir.is_dir():
        raise NotADirectoryError(f"artifacts directory not found: {artifacts_dir}")

    write_json(
        artifacts_dir / f"{TOOL_ID}.json",
        build_tool_manifest(artifacts_dir, args.version, args.repository),
    )
    write_json(
        artifacts_dir / f"{TOOL_ID}-docs.json",
        build_docs_manifest(args.repository),
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
