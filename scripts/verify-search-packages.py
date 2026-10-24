# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Independent archive, manifest, executable, docs and runtime acceptance."""
import hashlib
import json
import tarfile
import uuid
import zipfile
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "evidence/search/master-review"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def verify(record):
    archive = Path(record["archive"])
    platform = record["platform"]
    assert digest(archive.read_bytes()) == record["sha256"]
    if platform == "windows":
        with zipfile.ZipFile(archive) as bundle:
            files = {n: bundle.read(n) for n in bundle.namelist() if not n.endswith("/")}
        modes = {}
    else:
        with tarfile.open(archive) as bundle:
            files = {m.name.removeprefix("./"): bundle.extractfile(m).read() for m in bundle if m.isfile()}
            modes = {m.name.removeprefix("./"): m.mode for m in bundle if m.isfile()}
    for name in files:
        path = PurePosixPath(name)
        assert not path.is_absolute() and ".." not in path.parts, name
    manifest = json.loads(files["manifest.json"])
    expected = {f["path"]: f for f in manifest["files"]}
    assert set(files) == set(expected) | {"manifest.json"}
    for name, item in expected.items():
        assert len(files[name]) == item["bytes"], name
        assert digest(files[name]) == item["sha256"], name
    exe = "MDcard.exe" if platform == "windows" else "mdcard"
    assert files[exe] == (ROOT / "artifacts/global-search" / exe).read_bytes()
    for name in ["README.md", "docs/GLOBAL_SEARCH.md", "docs/ACCEPTANCE.md", "locales/ui.json", "THIRD_PARTY_NOTICES.txt"]:
        assert files[name] == (ROOT / name).read_bytes(), name
    node = "node.exe" if platform == "windows" else "node"
    assert files[f"renderer/runtime/{node}"] == (ROOT / "renderer/runtime" / node).read_bytes()
    assert files["renderer/vendor/plantuml.jar"] == (ROOT / "renderer/vendor/plantuml.jar").read_bytes()
    for name in ["renderer/worker.mjs", "renderer/Node-LICENSE.txt", "renderer/PlantUML-MIT-LICENSE.txt", "renderer/THIRD_PARTY_NOTICES.txt", "renderer/node_modules/playwright/package.json", "renderer/node_modules/playwright-core/package.json"]:
        assert name in files, name
    if platform == "linux":
        assert all(mode == (0o755 if name in ["mdcard", "renderer/runtime/node"] else 0o644) for name, mode in modes.items())
    gui = "master-review/windows/result.json" if platform == "windows" else "linux-final/result.json"
    gui = json.loads((ROOT / "evidence/search" / gui).read_text(encoding="utf-8-sig"))
    assert digest(files[exe]).lower() == gui["exe_sha256"].lower()
    return {"platform": platform, "archive": str(archive), "sha256": record["sha256"], "files_verified": len(files), "exe_sha256": digest(files[exe]), "gui_matches": True, "source_docs_match": True, "runtimes_match": True, "permissions_verified": platform == "linux"}


if __name__ == "__main__":
    records = json.loads((ROOT / "evidence/search/packages/result.json").read_text(encoding="utf-8-sig"))
    results = [verify(r) for r in records]
    report = {"verification_id": str(uuid.uuid4()), "date": "2026-10-20", "timezone": "Asia/Shanghai", "packages": results, "mismatches": 0}
    (OUT / "packages.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps(report, indent=2))
