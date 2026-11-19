#!/usr/bin/env python3
"""Verify an assembled release archive (host build) end to end.

The local mirror of the eukhe-release.yml build-job gates (a tarball, a zip
on Windows): the archive must contain exactly the designed payload at its root,
checksums must match SHA256SUMS and manifest.json, and the staged binary must
report the release version from a scratch cwd with `EUKHE_PACKAGE_DIR` unset
(the shipped artifact never depends on it).

Usage:
    python3 .github/scripts/release/verify_release.py \
        --dist-dir <dir> --version <x.y.z> --target <triple>
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Callable

# The bundled-catalog gate (same release-scripts directory).
from bundle_catalog import validate_bundled_catalog_dir
# The payload layout, target aliases, and shipped-content policy live with
# the assembler (same release-scripts directory).
from assemble_artifacts import (
    TARGET_ALIASES,
    RUNTIME_EXCLUDED_NAMES,
    RUNTIME_EXCLUDED_SUFFIXES,
    ZIP_EPOCH,
    archive_name,
    binary_name,
    staged_entries,
)
from runtime_lock import RUNTIME_LOCK_FILE, LockError, parse_requirements


@dataclass
class Member:
    """One archive entry, tar or zip alike."""
    name: str
    is_dir: bool
    is_file: bool
    is_link: bool
    mode: int
    # None when deterministic, else what differs.
    nondeterminism: str | None
    read: Callable[[], bytes]


def tar_members(archive: tarfile.TarFile) -> list[Member]:
    def reader(member):
        def read():
            data = archive.extractfile(member)
            if data is None:
                fail(f"archive entry {member.name!r} has no file data")
            with data:
                return data.read()
        return read
    return [
        Member(
            name=member.name,
            is_dir=member.isdir(),
            is_file=member.isfile(),
            is_link=member.issym() or member.islnk(),
            mode=member.mode,
            nondeterminism=None if member.uid == 0 and member.gid == 0 and member.mtime == 0
            else f"uid={member.uid} gid={member.gid} mtime={member.mtime}",
            read=reader(member),
        )
        for member in archive.getmembers()
    ]


def zip_members(archive: zipfile.ZipFile) -> list[Member]:
    def reader(info):
        return lambda: archive.read(info)
    members = []
    for info in archive.infolist():
        unix_mode = info.external_attr >> 16
        members.append(Member(
            name=info.filename.rstrip("/"),
            is_dir=info.is_dir(),
            is_file=not info.is_dir() and (unix_mode & 0o170000) == 0o100000,
            is_link=(unix_mode & 0o170000) == 0o120000,
            mode=unix_mode & 0o7777,
            nondeterminism=None if info.date_time == ZIP_EPOCH
            else f"date_time={info.date_time}",
            read=reader(info),
        ))
    return members


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def fail(message: str) -> None:
    print(f"error: {message}", file=sys.stderr)
    sys.exit(1)


def top_level_members(members: list[Member]) -> set:
    return {member.name.split("/", maxsplit=1)[0] for member in members}


def check_and_extract(members: list[Member], expected_top_level: set, scratch: Path) -> None:
    """Gates 1 and the extraction of gate 3, on any archive kind."""
    # 1. Deterministic archive shape: exact top-level payload, no link entries.
    if top_level_members(members) != expected_top_level:
        fail(
            f"archive top-level entries {sorted(top_level_members(members))} "
            f"!= designed payload {sorted(expected_top_level)}"
        )
    for member in members:
        if member.name.startswith(("skills/", "eukhe-runtime/")) and any(
            part in RUNTIME_EXCLUDED_NAMES or part.endswith(RUNTIME_EXCLUDED_SUFFIXES)
            for part in Path(member.name).parts
        ):
            fail(f"archive contains a development cache entry {member.name!r}")
        if member.is_link:
            fail(f"archive contains a link entry {member.name!r}")
        if member.nondeterminism is not None:
            fail(f"archive entry {member.name!r} is not deterministic "
                 f"({member.nondeterminism})")
    # A kernel installs Eukhe's packages only from this hash-locked file.
    lock_member = f"eukhe-runtime/{RUNTIME_LOCK_FILE}"
    lock = next((member for member in members if member.name == lock_member), None)
    if lock is None:
        fail(f"archive is missing the kernel runtime lock {lock_member!r}")
    if not lock.is_file:
        fail(f"kernel runtime lock {lock_member!r} is not a regular file")
    try:
        parse_requirements(lock.read().decode("utf-8"))
    except LockError as error:
        fail(f"kernel runtime lock {lock_member!r} rejected: {error}")

    # Manual extraction: every member must be a plain file or directory
    # whose relative name stays inside scratch, checked before any write;
    # staying on explicit member writes keeps the gate working on any
    # Python >= 3.8 (no `filter=` kwarg).
    scratch_root = str(scratch.resolve())
    for member in members:
        name = PurePosixPath(member.name)
        if name.is_absolute() or ".." in name.parts:
            fail(f"archive entry {member.name!r} is not a relative in-tree path")
        if not (member.is_dir or member.is_file):
            fail(f"archive entry {member.name!r} is not a regular file or directory")
        target = scratch / member.name
        if os.path.commonpath([scratch_root, str(target.resolve())]) != scratch_root:
            fail(f"archive entry {member.name!r} escapes the extraction directory")
        if member.is_dir:
            target.mkdir(parents=True, exist_ok=True)
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(member.read())
            os.chmod(target, member.mode & 0o755)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dist-dir", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", required=True)
    args = parser.parse_args()
    if args.target not in TARGET_ALIASES:
        fail(f"unknown release target {args.target!r} (known: {', '.join(TARGET_ALIASES)})")

    archive_file = archive_name(args.version, args.target)
    archive = args.dist_dir / archive_file
    if not archive.is_file():
        fail(f"archive {archive} not found; run assemble_artifacts.py first")

    # The designed archive-root payload (staged_entries in
    # assemble_artifacts.py). The bundled catalog assets among them must
    # also pass the same validation gates the packer enforced at assembly.
    expected_top_level = set(staged_entries(args.target))
    binary_entry = binary_name(args.target)

    # 1. The archive's shape, 2. its checksums, 3. the staged binary reports
    # the release version from a scratch cwd with EUKHE_PACKAGE_DIR unset:
    # shipped artifacts never depend on it.
    scratch = Path(tempfile.mkdtemp(prefix="eukhe-verify-"))
    try:
        if archive.suffix == ".zip":
            with zipfile.ZipFile(archive) as zipped:
                check_and_extract(zip_members(zipped), expected_top_level, scratch)
        else:
            with tarfile.open(archive) as tar:
                check_and_extract(tar_members(tar), expected_top_level, scratch)
        # 2. Checksums agree with SHA256SUMS and manifest.json.
        archive_sha = sha256_file(archive)
        sums_path = args.dist_dir / "SHA256SUMS"
        sums = {
            Path(name.strip()).name: digest.strip()
            for digest, name in
            (line.split(None, 1) for line in sums_path.read_text().splitlines() if line.strip())
        }
        if sums.get(archive_file) != archive_sha:
            fail(f"SHA256SUMS mismatch for {archive_file}")
        manifest = json.loads((args.dist_dir / "manifest.json").read_text())
        entries = {b["file"]: b for b in manifest["binaries"]}
        if archive_file not in entries:
            fail(f"manifest.json has no entry for {archive_file}")
        if entries[archive_file]["sha256"] != archive_sha:
            fail(f"manifest.json sha256 mismatch for {archive_file}")

        # 3. The bundled catalog assets must be present and valid in the
        # installed layout (the full packer gates: no small-fixture waiver).
        catalog_facts = validate_bundled_catalog_dir(scratch)
        binary = scratch / binary_entry
        if not os.access(binary, os.X_OK):
            fail(f"staged {binary_entry} is not executable")
        if entries[archive_file]["executableSha256"] != sha256_file(binary):
            fail("executableSha256 in manifest.json does not match the staged binary")
        env = {k: v for k, v in os.environ.items() if k != "EUKHE_PACKAGE_DIR"}
        run = subprocess.run(
            [str(binary), "--version"],
            capture_output=True, text=True, env=env, cwd=scratch, check=False,
        )
        if run.returncode != 0:
            fail(f"staged eukhe --version failed: {run.stderr.strip()}")
        version_out = run.stdout.strip()
        if version_out != args.version:
            fail(
                f"staged eukhe reports {version_out!r}, "
                f"expected {args.version!r}"
            )
    finally:
        shutil.rmtree(scratch, ignore_errors=True)

    print(
        f"verified {archive_file}: payload, checksums, manifest, "
        f"catalog assets ({json.dumps(catalog_facts)}), livecheck all OK"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
