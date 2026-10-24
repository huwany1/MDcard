# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Verify the shipped archives contain the tested binaries and all font licenses."""
import hashlib
import json
import tarfile
import uuid
import zipfile
from pathlib import Path

root = Path(__file__).resolve().parent.parent
codes = ["zh-CN", "zh-TW", "en", "ko", "ja", "ru"]
required = ["LICENSE", "THIRD_PARTY_NOTICES.txt", "NotoSansSC-OFL.txt", "NotoSansTC-OFL.txt", "NotoSansJP-OFL.txt", "NotoSansKR-OFL.txt", "NotoEmoji-OFL.txt", "font-sources.json", "README.md", "docs/ACCEPTANCE.md", "locales/ui.json"] + [f"locales/welcome-{code}.md" for code in codes]
sha256 = lambda data: hashlib.sha256(data).hexdigest()
results = []
for platform, archive_name, executable in [("windows", "MDcard-0.1.0-windows-x64.zip", "MDcard.exe"), ("linux", "MDcard-0.1.0-linux-x64.tar.gz", "mdcard")]:
    archive_path = root / "dist" / archive_name
    if platform == "windows":
        with zipfile.ZipFile(archive_path) as archive:
            files = {entry.filename.replace("\\", "/"): archive.read(entry) for entry in archive.infolist() if not entry.is_dir()}
        assert [name for name in files if name.endswith(".exe")] == ["MDcard.exe"]
    else:
        with tarfile.open(archive_path, "r:gz") as archive:
            entries = archive.getmembers()
            files = {entry.name.removeprefix("./"): archive.extractfile(entry).read() for entry in entries if entry.isfile()}
            assert next(entry for entry in entries if entry.name.removeprefix("./") == executable).mode == 0o755
    for name in required:
        assert name in files, (platform, "missing", name)
    actual_hash = sha256(files[executable])
    assert actual_hash == sha256((root / "dist" / platform / executable).read_bytes())
    if platform == "windows":
        matrix = json.loads((root / "evidence/localization/windows/matrix.json").read_text(encoding="utf-8-sig"))
        assert matrix["exe_sha256"].lower() == actual_hash
    else:
        assert (root / "evidence/localization/linux/executable.sha256").read_text().split()[0] == actual_hash
    assert set(json.loads(files["locales/ui.json"])) == set(codes)
    for name in required:
        if name.startswith("locales/"):
            assert files[name] == (root / name).read_bytes(), (platform, "stale locale", name)
    results.append({"platform": platform, "archive": str(archive_path.relative_to(root)), "archive_sha256": sha256(archive_path.read_bytes()), "executable_sha256": actual_hash, "all_font_licenses_present": True, "six_language_catalogs_and_welcomes_present": True, "files": len(files)})
report = {"verification_id": str(uuid.uuid4()), "date": "2026-10-19", "timezone": "Asia/Shanghai", "passed": True, "archives": results}
(root / "evidence/localization/package-check.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, indent=2))
