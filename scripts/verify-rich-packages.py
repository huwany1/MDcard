# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Independent archive inventory, path, content, runtime and mode verification."""
import datetime
import hashlib
import json
from pathlib import Path, PurePosixPath
import tarfile
import uuid
import zipfile

ROOT = Path(__file__).resolve().parent.parent
RUN = str(uuid.uuid4())
OUT = ROOT / "artifacts/rich/master-packages" / RUN
EVIDENCE = ROOT / "evidence/rich/master-review/packages"


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def safe(name):
    path = PurePosixPath(name)
    assert not path.is_absolute() and ".." not in path.parts
    assert "\\" not in name and ":" not in name


records = []
for platform, extension, executable in [
    ("windows", ".zip", "MDcard.exe"),
    ("linux", ".tar.gz", "mdcard"),
]:
    archive = ROOT / f"dist/MDcard-0.1.0-rich-{platform}-x64{extension}"
    folder = OUT / platform
    folder.mkdir(parents=True)
    if platform == "windows":
        with zipfile.ZipFile(archive) as handle:
            names = handle.namelist()
            assert len(names) == len(set(names)), "Duplicate archive entry"
            for name in names:
                safe(name)
            handle.extractall(folder)
    else:
        with tarfile.open(archive) as handle:
            members = handle.getmembers()
            assert len(members) == len({member.name for member in members})
            for member in members:
                safe(member.name)
                assert member.isfile() or member.isdir(), "Links/devices are forbidden"
                if member.isfile():
                    expected = 0o755 if member.name.lstrip("./") in (
                        executable, "renderer/runtime/node"
                    ) else 0o644
                    assert member.mode == expected, (member.name, oct(member.mode))
            handle.extractall(folder, filter="data")
    manifest = json.loads((folder / "manifest.json").read_text(encoding="utf-8"))
    assert manifest["platform"] == platform
    inventory = {row["path"]: row for row in manifest["files"]}
    assert len(inventory) == len(manifest["files"]), "Duplicate manifest entry"
    actual = {
        path.relative_to(folder).as_posix(): path
        for path in folder.rglob("*")
        if path.is_file() and path != folder / "manifest.json"
    }
    assert inventory.keys() == actual.keys(), "Archive file inventory mismatch"
    for name, path in actual.items():
        assert path.stat().st_size == inventory[name]["bytes"], name
        assert sha(path) == inventory[name]["sha256"], name
    assert sha(folder / executable) == sha(ROOT / "artifacts/rich" / executable)
    for name in ["worker.mjs", "package-lock.json", "runtime-sources.json", "vendor/browser.js",
                 "vendor/katex.css", "vendor/diagram-font.css", "vendor/plantuml.jar",
                 "vendor/fonts/MDcardDiagramSans-Regular.ttf", "vendor/fonts/MDcardDiagramSansKR-Regular.ttf"]:
        assert sha(folder / "renderer" / name) == sha(ROOT / "renderer" / name), name
    spec = json.loads((folder / "renderer/runtime-sources.json").read_text(encoding="utf-8"))
    assert sha(folder / "renderer/vendor/plantuml.jar") == spec["plantuml"]["sha256"]
    if platform == "windows":
        assert sha(folder / "renderer/runtime/node.exe") == spec["nodeWindows"]["sha256"]
    else:
        assert sha(folder / "renderer/runtime/node") == sha(ROOT / "renderer/runtime/node")
    records.append({
        "platform": platform, "archive": str(archive), "sha256": sha(archive),
        "extracted": str(folder), "filesVerified": len(actual),
        "executableSha256": sha(folder / executable),
        "inventoryAndHashes": True, "safePaths": True,
        "executableModes": platform == "linux",
    })

EVIDENCE.mkdir(parents=True, exist_ok=True)
for platform in ["windows", "linux"]:
    engines = json.loads((ROOT / f"evidence/rich/master-review/{platform}-engines/result.json").read_text(encoding="utf-8"))
    assert engines["passed"] and not engines["failures"]
    for name, expected in engines["sources"].items():
        assert sha(ROOT / "renderer" / name) == expected, f"Engine evidence is stale: {name}"
result = {"runId": RUN, "date": datetime.datetime.now(datetime.timezone.utc).isoformat(),
          "passed": True, "engineEvidenceMatchesPackagedSources": True, "records": records}
(EVIDENCE / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
print(json.dumps(result, indent=2))
