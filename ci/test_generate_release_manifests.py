#!/usr/bin/env python3
"""Tests for ci/generate_release_manifests.py.

Run: python3 -m unittest discover -s ci -p 'test_generate_release_manifests.py'
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import generate_release_manifests as grm  # noqa: E402

VERSION = "1.2.3"
REPO = "greentic-biz/greentic-fast2flow"


def fake_sha(seed: str) -> str:
    # Deterministic, valid 64-hex digest per archive.
    import hashlib

    return hashlib.sha256(seed.encode()).hexdigest()


def write_sidecars(root: Path, tool_id: str, specs) -> dict[str, str]:
    shas = {}
    for _os, _arch, triple, ext in specs:
        archive = f"{tool_id}-v{VERSION}-{triple}.{ext}"
        sha = fake_sha(archive)
        (root / f"{archive}.sha256").write_text(f"{sha}  {archive}\n", encoding="utf-8")
        shas[archive] = sha
    return shas


class RoutingHostDescriptorTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def test_routing_host_descriptor_shape(self) -> None:
        shas = write_sidecars(self.root, grm.ROUTING_HOST_ID, grm.ROUTING_HOST_TARGETS)
        manifest = grm.build_routing_host_manifest(self.root, VERSION, REPO)

        self.assertEqual(manifest["id"], "greentic-fast2flow-routing-host")
        self.assertEqual(manifest["schema_version"], "1")
        self.assertEqual(manifest["$schema"], grm.TOOL_SCHEMA_URL)
        install = manifest["install"]
        self.assertEqual(install["type"], "release-binary")
        self.assertEqual(install["binary_name"], "greentic-fast2flow-routing-host")
        self.assertEqual(len(install["targets"]), 6)
        for target in install["targets"]:
            archive = target["url"].rsplit("/", 1)[1]
            self.assertEqual(
                target["url"],
                f"https://github.com/{REPO}/releases/download/v{VERSION}/{archive}",
            )
            # The digest comes from the sidecar of exactly that archive.
            self.assertEqual(target["sha256"], shas[archive])

    def test_linux_targets_are_musl_and_unique_per_platform(self) -> None:
        write_sidecars(self.root, grm.ROUTING_HOST_ID, grm.ROUTING_HOST_TARGETS)
        targets = grm.build_routing_host_manifest(self.root, VERSION, REPO)["install"]["targets"]

        pairs = [(t["os"], t["arch"]) for t in targets]
        # greentic-dev picks the FIRST os+arch match, so a duplicate is dead or wrong.
        self.assertEqual(len(pairs), len(set(pairs)))
        linux = [t["url"] for t in targets if t["os"] == "linux"]
        self.assertEqual(len(linux), 2)
        for url in linux:
            self.assertIn("-unknown-linux-musl.tar.gz", url)
            self.assertNotIn("linux-gnu", url)

    def test_missing_musl_sidecar_fails_closed(self) -> None:
        write_sidecars(self.root, grm.ROUTING_HOST_ID, grm.ROUTING_HOST_TARGETS)
        missing = self.root / (
            f"{grm.ROUTING_HOST_ID}-v{VERSION}-aarch64-unknown-linux-musl.tar.gz.sha256"
        )
        missing.unlink()
        with self.assertRaises(FileNotFoundError):
            grm.build_routing_host_manifest(self.root, VERSION, REPO)

    def test_invalid_sha_rejected(self) -> None:
        write_sidecars(self.root, grm.ROUTING_HOST_ID, grm.ROUTING_HOST_TARGETS)
        bad = self.root / (
            f"{grm.ROUTING_HOST_ID}-v{VERSION}-x86_64-unknown-linux-musl.tar.gz.sha256"
        )
        bad.write_text("nothex  x\n", encoding="utf-8")
        with self.assertRaises(ValueError):
            grm.build_routing_host_manifest(self.root, VERSION, REPO)

    def test_cli_descriptor_unchanged(self) -> None:
        shas = write_sidecars(self.root, grm.TOOL_ID, grm.TARGETS)
        manifest = grm.build_tool_manifest(self.root, VERSION, REPO)
        self.assertEqual(manifest["id"], "greentic-fast2flow")
        self.assertEqual(manifest["install"]["binary_name"], "greentic-fast2flow")
        urls = [t["url"] for t in manifest["install"]["targets"]]
        self.assertEqual(len(urls), 6)
        self.assertTrue(any("x86_64-unknown-linux-gnu" in u for u in urls))
        self.assertFalse(any("musl" in u for u in urls))
        for target in manifest["install"]["targets"]:
            self.assertEqual(target["sha256"], shas[target["url"].rsplit("/", 1)[1]])

    def test_main_writes_all_four_files(self) -> None:
        write_sidecars(self.root, grm.TOOL_ID, grm.TARGETS)
        write_sidecars(self.root, grm.ROUTING_HOST_ID, grm.ROUTING_HOST_TARGETS)
        rc = grm.main(
            ["--artifacts-dir", str(self.root), "--version", VERSION, "--repository", REPO]
        )
        self.assertEqual(rc, 0)
        for name in (
            "greentic-fast2flow.json",
            "greentic-fast2flow-routing-host.json",
            "greentic-fast2flow-docs.json",
            "greentic-fast2flow-store.json",
        ):
            path = self.root / name
            self.assertTrue(path.is_file(), name)
            json.loads(path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
