# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Synchronize reviewed complete stages and archives without rebuilding/repacking.

Only explicit files below the resolved workspace dist/{windows,linux} are changed.
Unlisted old files are backed up in artifacts; old archives are preserved.
Every source file is verified before mutation.
"""
import argparse
import hashlib
import json
import shutil
from pathlib import Path

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument("packages", type=Path)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
records = json.loads(args.packages.read_text("utf-8-sig"))
dist = (root / "dist").resolve()
if not dist.is_relative_to(root.resolve()) or dist.name != "dist":
    raise ValueError("Unexpected distribution root")


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


verified = []
for record in records:
    platform = record["platform"]
    if platform not in {"windows", "linux"}:
        raise ValueError("Unknown platform")
    stage = Path(record["stage"]).resolve()
    manifest = json.loads((stage / "manifest.json").read_text("utf-8-sig"))
    names = {entry["path"] for entry in manifest["files"]}
    if len(names) != len(manifest["files"]):
        raise ValueError("Duplicate manifest path")
    for entry in manifest["files"]:
        source = stage / entry["path"]
        if not source.resolve().is_relative_to(stage) or source.is_symlink():
            raise ValueError("Unsafe source path")
        if source.stat().st_size != entry["bytes"] or sha(source) != entry["sha256"]:
            raise ValueError("Source checksum mismatch: " + str(source))
    archive = Path(record["archive"]).resolve()
    if sha(archive) != record["archive_sha256"]:
        raise ValueError("Archive checksum mismatch")
    verified.append((record, stage, manifest, names | {"manifest.json"}, archive))

published = []
for record, stage, manifest, names, archive in verified:
    target_path = dist / record["platform"]
    if target_path.is_symlink():
        raise ValueError("Distribution target is a symlink")
    target = target_path.resolve()
    if target.parent != dist or target.name != record["platform"] or not target.is_relative_to(root.resolve()):
        raise ValueError("Unsafe distribution target")
    target.mkdir(parents=True, exist_ok=True)
    for current in target.rglob("*"):
        if current.is_symlink() or not current.resolve().is_relative_to(target):
            raise ValueError("Unsafe destination entry")
        if current.is_file() and current.relative_to(target).as_posix() not in names:
            relative = current.relative_to(target)
            backup = root / "artifacts/workbench-redesign-2026-10-22/dist-preserved" / manifest["buildId"] / record["platform"] / relative
            if not backup.resolve().is_relative_to(root.resolve() / "artifacts"):
                raise ValueError("Unsafe backup target")
            backup.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(current, backup)
            if sha(backup) != sha(current):
                raise ValueError("Old-file backup mismatch")
            current.unlink()
    for name in sorted(names):
        destination = target / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(stage / name, destination)
    for entry in manifest["files"]:
        if sha(target / entry["path"]) != entry["sha256"]:
            raise ValueError("Destination checksum mismatch")
    actual = {path.relative_to(target).as_posix() for path in target.rglob("*") if path.is_file()}
    if actual != names or sha(target / "manifest.json") != sha(stage / "manifest.json"):
        raise ValueError("Destination membership/manifest mismatch")
    destination_archive = dist / archive.name
    shutil.copy2(archive, destination_archive)
    if sha(destination_archive) != record["archive_sha256"]:
        raise ValueError("Copied archive checksum mismatch")
    published.append({**record, "stage": str(target), "archive": str(destination_archive),
                      "build_id": manifest["buildId"]})
    print("Verified synchronized", record["platform"], len(names), flush=True)
(dist / "latest.json").write_text(json.dumps(published, indent=2) + "\n", "utf-8")
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps({"status": "通过", "rebuilt": False, "repacked": False,
                                  "packages": published}, indent=2, ensure_ascii=False) + "\n", "utf-8")
