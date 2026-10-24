# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Archive explicitly selected, completed local evidence and verify every member.

This is an evidence bundle, not the full application runtime package.
Never follows symlinks or silently overwrites an earlier delivery.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parent.parent


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--plan", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    plan_path = (ROOT / args.plan).resolve()
    destination = (ROOT / args.output).resolve()
    if not plan_path.is_relative_to(ROOT) or not destination.is_relative_to(ROOT):
        raise ValueError("Plan and output must stay inside the workspace")
    plan = json.loads(plan_path.read_text(encoding="utf-8-sig"))
    files = {}
    skipped = []

    def add(path):
        if path.is_symlink():
            skipped.append({"path": path.relative_to(ROOT).as_posix(), "reason": "symlink not followed"})
            return
        resolved = path.resolve(strict=True)
        if not resolved.is_relative_to(ROOT) or not resolved.is_file():
            raise ValueError("Evidence member is not a regular workspace file: " + str(path))
        path = resolved
        if resolved == destination:
            raise ValueError("Cannot include the archive in itself")
        name = path.relative_to(ROOT).as_posix()
        if name.casefold() == "evidence-manifest.json":
            raise ValueError("Reserved evidence manifest name")
        folded = name.casefold()
        if folded in files and files[folded] != path:
            raise ValueError("Case-colliding archive members: " + name)
        files[folded] = path

    add(plan_path)
    for value in plan["include"]:
        path = ROOT / value
        if not path.resolve(strict=True).is_relative_to(ROOT):
            raise ValueError("Selected evidence escapes the workspace")
        if path.is_symlink() or path.is_file():
            add(path)
            continue
        for directory, directories, names in os.walk(path, followlinks=False):
            directory = Path(directory)
            for name in list(directories):
                child = directory / name
                if child.is_symlink():
                    skipped.append({"path": child.relative_to(ROOT).as_posix(), "reason": "symlink directory not followed"})
                    directories.remove(name)
            for name in names:
                add(directory / name)
    records = []
    destination.parent.mkdir(parents=True, exist_ok=True)
    sidecar = destination.with_suffix(destination.suffix + ".manifest.json")
    if sidecar.exists():
        raise FileExistsError("Preserve the previous manifest: " + str(sidecar))
    with zipfile.ZipFile(destination, "x", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
        for path in sorted(files.values()):
            name = path.relative_to(ROOT).as_posix()
            size = path.stat().st_size
            checksum = digest(path)
            archive.write(path, name)
            if path.stat().st_size != size or digest(path) != checksum:
                raise RuntimeError("Evidence changed during archiving: " + name)
            records.append({"path": name, "bytes": size, "sha256": checksum})
        manifest = {
            "schema": 1,
            "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "plan_sha256": digest(plan_path),
            "product_commit": plan["product_commit"],
            "scope": "Explicit local evidence selection; historical failures remain in the workspace and selected history records. Not a runtime package or a new acceptance claim.",
            "files": records,
            "skipped": skipped,
        }
        archive.writestr("evidence-manifest.json", json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
    with zipfile.ZipFile(destination) as archive:
        if len(archive.namelist()) != len(records) + 1:
            raise RuntimeError("Unexpected or duplicate evidence members")
        for record in records:
            with archive.open(record["path"]) as stream:
                checksum = hashlib.file_digest(stream, "sha256").hexdigest()
            if archive.getinfo(record["path"]).file_size != record["bytes"] or checksum != record["sha256"]:
                raise RuntimeError("Archived evidence checksum mismatch: " + record["path"])
    manifest.update(archive=destination.relative_to(ROOT).as_posix(), archive_bytes=destination.stat().st_size,
                    archive_sha256=digest(destination), every_member_verified=True)
    with sidecar.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(manifest, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    print(json.dumps({"passed": True, "files": len(records), "bytes": manifest["archive_bytes"],
                      "sha256": manifest["archive_sha256"], "manifest": sidecar.relative_to(ROOT).as_posix()}))


if __name__ == "__main__":
    main()
