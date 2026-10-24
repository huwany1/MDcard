# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Verify current packages contain the freshly verified executables and docs."""
import hashlib
import json
from pathlib import Path
import tarfile
import uuid
import zipfile

root = Path(__file__).resolve().parents[1]
sha = lambda data: hashlib.sha256(data).hexdigest()
checks = {}
with zipfile.ZipFile(root / "dist/MDcard-0.1.0-windows-x64.zip") as archive:
    assert "MDcard.exe" in archive.namelist()
    assert not any("before-" in name and name.endswith(".exe") for name in archive.namelist())
    assert archive.read("MDcard.exe") == (root / "dist/windows/MDcard.exe").read_bytes()
    for name in ["README.md", "docs/ACCEPTANCE.md", "locales/ui.json", "THIRD_PARTY_NOTICES.txt"]:
        assert archive.read(name) == (root / name).read_bytes(), name
    checks["windows"] = {"exe_sha256": sha(archive.read("MDcard.exe")), "fresh_docs_and_locales": True}
with tarfile.open(root / "dist/MDcard-0.1.0-linux-x64.tar.gz") as archive:
    members = {m.name.removeprefix("./"): m for m in archive.getmembers()}
    assert members["mdcard"].mode & 0o111
    assert archive.extractfile(members["mdcard"]).read() == (root / "dist/linux/mdcard").read_bytes()
    for name in ["README.md", "docs/ACCEPTANCE.md", "locales/ui.json", "THIRD_PARTY_NOTICES.txt"]:
        assert archive.extractfile(members[name]).read() == (root / name).read_bytes(), name
    checks["linux"] = {"exe_sha256": sha((root / "dist/linux/mdcard").read_bytes()), "mode": oct(members["mdcard"].mode), "fresh_docs_and_locales": True}
for platform in ["windows", "linux"]:
    report = json.loads((root / f"evidence/encoding/{platform}-final/result.json").read_text(encoding="utf-8-sig"))
    assert report["exe_sha256"].lower() == checks[platform]["exe_sha256"], platform
result = {"verification_id": str(uuid.uuid4()), "date": "2026-10-19", "timezone": "Asia/Shanghai", "checks": checks}
output = root / "evidence/encoding/package-check.json"
output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
print(json.dumps(result, indent=2))
