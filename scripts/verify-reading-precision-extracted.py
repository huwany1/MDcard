# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Read-only full-payload check of an already extracted precision package.

Run after native package GUI closes. This never extracts, repairs, or removes
files, and refuses to overwrite earlier acceptance evidence.
"""
import argparse
import concurrent.futures
import datetime
import hashlib
import json
import os
from pathlib import Path
import stat


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--platform", choices=["windows", "linux"], required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    package = args.package.resolve(strict=True)
    output = args.output.resolve()
    assert not output.exists(), "Keep prior evidence"
    manifest_path = package / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert manifest["sourceCommit"] == "8c09f82daad47f623d9020b2370dfe57123ba80f"
    assert manifest["binarySourceCommit"] == "40b5e3bb16e1658e333cf2fddf965a22d4e878ff"
    expected = {entry["path"]: entry for entry in manifest["files"]}
    assert len(expected) == len(manifest["files"])
    actual = set()
    for parent, directories, names in os.walk(package, followlinks=False):
        for name in directories + names:
            path = Path(parent, name)
            info = path.lstat()
            assert not stat.S_ISLNK(info.st_mode), path
            assert not getattr(info, "st_file_attributes", 0) & 0x400, path
            assert path.resolve(strict=True).is_relative_to(package), path
        for name in names:
            path = Path(parent, name)
            assert path.is_file()
            actual.add(path.relative_to(package).as_posix())
    assert actual == set(expected) | {"manifest.json"}, {
        "extra": sorted(actual - set(expected) - {"manifest.json"}),
        "missing": sorted(set(expected) - actual),
    }

    def verify(item):
        name, record = item
        path = package / name
        assert path.stat().st_size == record["bytes"], name
        assert sha(path) == record["sha256"], name
        return record["bytes"]

    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        total_bytes = sum(pool.map(verify, expected.items()))
    app, node, required = (
        ("MDcard.exe", "renderer/runtime/node.exe", [])
        if args.platform == "windows"
        else ("mdcard", "renderer/runtime/node", ["renderer/runtime/chromium/chrome"])
    )
    expected_binary = (
        "face40f4af716c004b39e4ce992af088dedda171f924a73df8cf8504eefb67e1"
        if args.platform == "windows"
        else "b438bd9e042366a77c63bcb9f6ccc989414119788959086cec98114f2c6cd9cf"
    )
    assert expected[app]["sha256"] == expected_binary
    resources = {}
    for name in [app, node, "renderer/worker.mjs"] + required:
        path = package / name
        assert path.resolve(strict=True).is_relative_to(package)
        if args.platform == "linux" and name != "renderer/worker.mjs":
            assert os.access(path, os.X_OK), name
        resources[name] = expected[name]["sha256"]
    assert (package / "renderer/vendor").is_dir()
    result = {
        "passed": True,
        "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "platform": args.platform,
        "actual_package": str(package),
        "source_commit": manifest["sourceCommit"],
        "binary_source_commit": manifest["binarySourceCommit"],
        "verified_payload_files": len(expected),
        "verified_payload_bytes": total_bytes,
        "exact_regular_file_set": True,
        "manifest_sha256": sha(manifest_path),
        "resources": resources,
        "boundary": "Read-only actual extracted payload after GUI. Every manifest SHA and size, exact file set, no symlink/reparse entry, package resource ownership; GUI correctness is separate evidence.",
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(result, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    print(json.dumps({"passed": True, "platform": args.platform, "files": len(expected)}))


if __name__ == "__main__":
    main()
