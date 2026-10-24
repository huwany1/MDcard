# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Build isolated audit candidates from verified existing full-runtime stages.

Does not change dist or mark the candidate as a published release.
"""
import argparse
import concurrent.futures
import datetime
import hashlib
import json
import shutil
import subprocess
import tarfile
import tomllib
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument("--windows-exe", required=True)
parser.add_argument("--linux-exe", required=True)
parser.add_argument("--build-id", required=True)
parser.add_argument("--output-root", default="artifacts/bug-audit")
parser.add_argument("--candidate-kind", default="bug-audit")
parser.add_argument("--binary-source-commit", help="Optional original binary commit for documentation-only release snapshots")
args = parser.parse_args()
if not args.build_id or any(c not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_" for c in args.build_id):
    raise ValueError("Use a simple build ID")
if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT):
    raise RuntimeError("Commit the reviewed source and documentation before packaging")
output_root = (ROOT / args.output_root).resolve()
if not output_root.is_relative_to(ROOT.resolve()):
    raise ValueError("Candidate output must stay inside the workspace")
base = output_root / args.build_id
base.mkdir(parents=True, exist_ok=False)
version = tomllib.loads((ROOT / "Cargo.toml").read_text("utf8"))["package"]["version"]
commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()


def sha(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def record(path, stage):
    return {"path": path.relative_to(stage).as_posix(), "bytes": path.stat().st_size, "sha256": sha(path)}


def linux_permissions(info):
    name = info.name.removeprefix("./")
    executable = (
        name == "mdcard" or name == "renderer/runtime/node"
        or name.startswith("renderer/runtime/java/bin/") or name.endswith("/jspawnhelper")
        or name in {"renderer/runtime/pandoc/pandoc", "renderer/runtime/pandoc/bin/pandoc",
                    "renderer/runtime/chromium/chrome", "renderer/runtime/chromium/chrome_crashpad_handler",
                    "renderer/runtime/chromium/chrome_sandbox"}
    )
    info.mode = 0o755 if info.isdir() or executable or ".so" in name else 0o644
    info.uid = info.gid = 0
    info.uname = info.gname = ""
    return info


records = []
for platform, binary in [("windows", args.windows_exe), ("linux", args.linux_exe)]:
    original = ROOT / "dist" / platform
    original_manifest = json.loads((original / "manifest.json").read_text("utf8"))

    def verify(entry):
        path = original / entry["path"]
        if not path.resolve().is_relative_to(original.resolve()) or path.is_symlink():
            raise RuntimeError("Base manifest path is unsafe: " + str(path))
        if path.stat().st_size != entry["bytes"] or sha(path) != entry["sha256"]:
            raise RuntimeError("Base runtime/source checksum mismatch: " + str(path))

    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        list(pool.map(verify, original_manifest["files"]))
    print("Verified original", platform, len(original_manifest["files"]), flush=True)
    stage = base / platform
    stage.mkdir()
    for entry in original_manifest["files"]:
        destination = stage / entry["path"]
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(original / entry["path"], destination)
    shutil.copy2(binary, stage / ("MDcard.exe" if platform == "windows" else "mdcard"))
    for name in ["README.md", "LICENSE", "THIRD_PARTY_NOTICES.txt"]:
        shutil.copy2(ROOT / name, stage / name)
    shutil.copytree(ROOT / "docs", stage / "docs", dirs_exist_ok=True)
    shutil.copytree(ROOT / "locales", stage / "locales", dirs_exist_ok=True)
    # Include current font provenance and notices alongside embedded UI fonts.
    # Runtime resources stay compatible; new regular UI instances also ship as
    # auditable files so their hashes and OFL source can be checked separately.
    shutil.copytree(ROOT / "assets", stage / "assets", dirs_exist_ok=True)
    # Runtime binaries/dependencies come from the verified release. Engine source
    # must come from this candidate, including process-lifecycle bug fixes.
    tracked_renderer = subprocess.check_output(
        ["git", "ls-files", "-z", "--", "renderer"], cwd=ROOT
    ).decode("utf8").split("\0")
    for name in filter(None, tracked_renderer):
        destination = stage / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / name, destination)
    manifest = dict(original_manifest)
    manifest.update(sourceCommit=commit, auditId=args.build_id, buildId=args.build_id,
                    candidateKind=args.candidate_kind, acceptanceCandidate=True,
                    createdAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                    baseRuntimeSourceCommit=original_manifest['sourceCommit'])
    if args.binary_source_commit:
        manifest["binarySourceCommit"] = subprocess.check_output(
            ["git", "rev-parse", args.binary_source_commit + "^{commit}"], cwd=ROOT, text=True
        ).strip()
    paths = sorted(path for path in stage.rglob("*") if path.is_file() and path != stage / "manifest.json")
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        manifest["files"] = list(pool.map(lambda path: record(path, stage), paths))
    (stage / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", "utf8")
    archive = base / f"MDcard-{version}-{args.build_id}-{platform}-x64{'.zip' if platform == 'windows' else '.tar.gz'}"
    if platform == "windows":
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as output:
            for path in sorted(stage.rglob("*")):
                if path.is_file():
                    output.write(path, path.relative_to(stage).as_posix())
    else:
        with tarfile.open(archive, "w:gz", compresslevel=6) as output:
            output.add(stage, arcname=".", filter=linux_permissions)
    records.append({"platform": platform, "stage": str(stage), "archive": str(archive),
                    "archive_sha256": sha(archive), "binary_sha256": sha(stage / ("MDcard.exe" if platform == "windows" else "mdcard")),
                    "manifest_files": len(manifest["files"]), "source_commit": commit,
                    **({"binary_source_commit": manifest["binarySourceCommit"]} if args.binary_source_commit else {})})
    print("Candidate ready", platform, flush=True)
(base / "packages.json").write_text(json.dumps(records, indent=2) + "\n", "utf8")
print(json.dumps(records, indent=2), flush=True)
