#!/usr/bin/env python3
"""Tests of assemble_artifacts.py and verify_release.py on the archive kinds:
the macOS tarball and the Windows zip assemble, verify, and repack byte for
byte; a tampered zip fails verification.

The shipped binary is a stand-in script that prints the version (the
gates run it with `--version`), so the test runs on any Unix host.

Run: python3 .github/scripts/release/test_release_archives.py
"""

from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
ASSEMBLE = HERE / "assemble_artifacts.py"
VERIFY = HERE / "verify_release.py"
CATALOG = HERE / "bundle_catalog.py"
VERSION = "0.10.42"


def run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, *args], capture_output=True, text=True, check=False)


class ReleaseArchiveTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = Path(tempfile.mkdtemp(prefix="eukhe-archive-test-"))
        cls.catalog = cls.tmp / "catalog"
        generated = run(str(CATALOG), "generate", "--out", str(cls.catalog))
        assert generated.returncode == 0, generated.stderr

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.tmp, ignore_errors=True)

    def binary(self, name: str) -> Path:
        path = self.tmp / "bin" / name
        path.parent.mkdir(exist_ok=True)
        path.write_text(f"#!/bin/sh\necho {VERSION}\n")
        path.chmod(0o755)
        return path

    def assemble(self, target: str, binary: Path, out: Path) -> subprocess.CompletedProcess:
        return run(str(ASSEMBLE), "--repo-root", str(REPO), "--version", VERSION,
                   "--target", target, "--binary", str(binary),
                   "--catalog-assets", str(self.catalog), "--out-dir", str(out))

    def verify(self, target: str, out: Path) -> subprocess.CompletedProcess:
        return run(str(VERIFY), "--dist-dir", str(out), "--version", VERSION, "--target", target)

    def test_the_windows_zip_assembles_verifies_and_repacks_identically(self):
        binary = self.binary("eukhe.exe")
        first, second = self.tmp / "win-a", self.tmp / "win-b"
        for out in (first, second):
            assembled = self.assemble("x86_64-pc-windows-msvc", binary, out)
            self.assertEqual(assembled.returncode, 0, assembled.stderr)
        name = f"eukhe-{VERSION}-windows-x64.zip"
        self.assertEqual((first / name).read_bytes(), (second / name).read_bytes())
        verified = self.verify("x86_64-pc-windows-msvc", first)
        self.assertEqual(verified.returncode, 0, verified.stderr)
        with zipfile.ZipFile(first / name) as archive:
            top = {member.split("/", 1)[0] for member in archive.namelist()}
        self.assertIn("eukhe.exe", top)
        self.assertNotIn("eukhe", top)
        manifest = json.loads((first / "manifest.json").read_text())
        self.assertEqual([b["platform"] for b in manifest["binaries"]], ["windows-x64"])
        sums = (first / "SHA256SUMS").read_text().split()
        self.assertEqual(sums[1], name)

    def test_the_darwin_tarball_still_assembles_and_verifies(self):
        binary = self.binary("eukhe")
        out = self.tmp / "darwin"
        assembled = self.assemble("aarch64-apple-darwin", binary, out)
        self.assertEqual(assembled.returncode, 0, assembled.stderr)
        self.assertTrue((out / f"eukhe-{VERSION}-darwin-arm64.tar.gz").is_file())
        verified = self.verify("aarch64-apple-darwin", out)
        self.assertEqual(verified.returncode, 0, verified.stderr)

    def test_a_zip_with_a_stamped_member_fails_verification(self):
        binary = self.binary("eukhe.exe")
        out = self.tmp / "win-tampered"
        assembled = self.assemble("x86_64-pc-windows-msvc", binary, out)
        self.assertEqual(assembled.returncode, 0, assembled.stderr)
        name = out / f"eukhe-{VERSION}-windows-x64.zip"
        with zipfile.ZipFile(name, "a") as archive:
            stamped = zipfile.ZipInfo("skills/stamped.txt", date_time=(2026, 1, 1, 0, 0, 0))
            stamped.external_attr = 0o100644 << 16
            archive.writestr(stamped, b"x")
        # The checksums follow the tampered zip, so only the shape gate is left.
        digest = hashlib.sha256(name.read_bytes()).hexdigest()
        (out / "SHA256SUMS").write_text(f"{digest}  {name.name}\n")
        manifest = json.loads((out / "manifest.json").read_text())
        manifest["binaries"][0]["sha256"] = digest
        (out / "manifest.json").write_text(json.dumps(manifest))
        verified = self.verify("x86_64-pc-windows-msvc", out)
        self.assertNotEqual(verified.returncode, 0)
        self.assertIn("is not deterministic", verified.stderr)


if __name__ == "__main__":
    unittest.main()
