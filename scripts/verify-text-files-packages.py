# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Check the current archives against the final text-file UI evidence."""
import hashlib
import json
import tarfile
import uuid
import zipfile
from pathlib import Path, PurePosixPath

root = Path(__file__).resolve().parent.parent
proof = root / "evidence/text-files"
sha = lambda data: hashlib.sha256(data).hexdigest()
windows = json.loads((proof / "master-review/result.json").read_text(encoding="utf-8-sig"))
assert windows["status"] == "passed"
expected = {
    "windows": windows["tested_exe_sha256"].lower(),
    "linux": (proof / "linux/executable.sha256").read_text(encoding="utf-8").split()[0],
}
required = ["LICENSE", "THIRD_PARTY_NOTICES.txt", "NotoSansSC-OFL.txt", "NotoSansTC-OFL.txt", "NotoSansJP-OFL.txt", "NotoSansKR-OFL.txt", "NotoEmoji-OFL.txt", "font-sources.json", "README.md", "docs/ACCEPTANCE.md"]
required += [str(p.relative_to(root)).replace("\\", "/") for p in (root / "locales").glob("*") if p.is_file()]
results = []
for platform, name, executable in [("windows", "MDcard-0.1.0-windows-x64.zip", "MDcard.exe"), ("linux", "MDcard-0.1.0-linux-x64.tar.gz", "mdcard")]:
    archive_path = root / "dist" / name
    if platform == "windows":
        with zipfile.ZipFile(archive_path) as archive:
            files = {e.filename.replace("\\", "/"): archive.read(e) for e in archive.infolist() if not e.is_dir()}
        assert [p for p in files if p.endswith(".exe")] == [executable]
    else:
        with tarfile.open(archive_path, "r:gz") as archive:
            entries = archive.getmembers()
            assert all(not e.issym() and not e.islnk() for e in entries)
            files = {e.name.removeprefix("./"): archive.extractfile(e).read() for e in entries if e.isfile()}
            assert next(e for e in entries if e.name.removeprefix("./") == executable).mode == 0o755
    assert all(not PurePosixPath(p).is_absolute() and ".." not in PurePosixPath(p).parts for p in files)
    assert sha(files[executable]) == expected[platform] == sha((root / "dist" / platform / executable).read_bytes())
    for path in required:
        assert path in files, (platform, "missing", path)
        if path == "README.md" or path.startswith(("docs/", "locales/")):
            assert files[path] == (root / path).read_bytes(), (platform, "stale", path)
    results.append({"platform": platform, "archive": str(archive_path.relative_to(root)), "archive_sha256": sha(archive_path.read_bytes()), "executable_sha256": expected[platform], "files": len(files), "current_docs_and_locales": True, "all_font_licenses": True, "safe_paths": True})
report = {"verification_id": str(uuid.uuid4()), "date": "2026-10-19", "timezone": "Asia/Shanghai", "passed": True, "archives": results}
(proof / "package-check.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, indent=2))
