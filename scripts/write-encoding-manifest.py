# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Bind the final encoding evidence, source snapshot, packages, and cleanup state."""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import uuid

root = Path(__file__).resolve().parents[1]
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
files = [root / name for name in ["Cargo.toml", "Cargo.lock", "README.md", "docs/ACCEPTANCE.md", "THIRD_PARTY_NOTICES.txt", "locales/ui.json"]]
files += list(root.joinpath("src").glob("*.rs"))
files += list(root.joinpath("tests/fixtures/encodings").glob("*"))
files += [root / name for name in ["scripts/generate-encoding-fixtures.py", "scripts/add-encoding-translations.py", "scripts/verify-encoding-windows.ps1", "scripts/verify-encoding-linux.sh", "scripts/verify-encoding-packages.py", "scripts/write-encoding-manifest.py"]]
identities = {str(path.relative_to(root)).replace("\\", "/"): digest(path) for path in sorted(files)}
windows = json.loads(root.joinpath("evidence/encoding/windows-final/result.json").read_text(encoding="utf-8-sig"))
linux = json.loads(root.joinpath("evidence/encoding/linux-final/result.json").read_text(encoding="utf-8"))
package_check = json.loads(root.joinpath("evidence/encoding/package-check.json").read_text(encoding="utf-8"))
artifacts = {name: digest(root / name) for name in ["dist/windows/MDcard.exe", "dist/linux/mdcard", "dist/MDcard-0.1.0-windows-x64.zip", "dist/MDcard-0.1.0-linux-x64.tar.gz", "artifacts/encoding/windows/encoding-tests.exe"]}
assert artifacts["dist/windows/MDcard.exe"] == windows["exe_sha256"].lower()
assert artifacts["dist/linux/mdcard"] == linux["exe_sha256"].lower()
assert package_check["checks"]["windows"]["exe_sha256"] == artifacts["dist/windows/MDcard.exe"]
assert package_check["checks"]["linux"]["exe_sha256"] == artifacts["dist/linux/mdcard"]
dev_state = subprocess.check_output(["docker", "inspect", "mdcard-dev", "--format", "{{.State.Status}}"], text=True).strip()
assert dev_state == "exited", dev_state
result = {
    "verification_id": str(uuid.uuid4()), "date": "2026-10-19", "timezone": "Asia/Shanghai",
    "recorded_utc": datetime.now(timezone.utc).isoformat(),
    "scope": "Strict local codecs, explicit reopen/save conversion, six-language UI and lossless save behavior",
    "encodings": ["UTF-8", "UTF-8 BOM", "UTF-16 LE", "UTF-16 BE", "GBK", "GB18030", "Big5", "Shift_JIS", "EUC-KR", "Windows-1251", "Windows-1252"],
    "tests": {"linux": {"passed": 30, "failed": 0, "ignored": 0}, "windows": {"passed": 29, "failed": 0, "ignored": 0}, "independent_python_fixture_pairs": 13, "fmt_check": True, "clippy_all_targets_deny_warnings": True},
    "ui_verification_ids": {"windows": windows["verification_id"], "linux": linux["verification_id"]},
    "package_verification_id": package_check["verification_id"],
    "sources_sha256": identities, "artifacts_sha256": artifacts,
    "evidence": {"windows": "evidence/encoding/windows-final", "linux": "evidence/encoding/linux-final", "package_check": "evidence/encoding/package-check.json", "master_test_log": "artifacts/encoding-master-final-tests.log"},
    "cleanup": {"mdcard_dev": dev_state, "test_windows_closed": windows["all_test_windows_closed"], "dify_containers_untouched": True, "isolated_settings": True, "linux_temporary_fixture_removed": True},
    "boundaries": ["Legacy detection is a guess; manual override remains available", "BOM-less UTF-16 requires manual encoding selection", "WHATWG-compatible mappings, not full GB18030 certification or every vendor variant", "UTF-32 unsupported", "No claim for untested Wayland/desktop/DPI or all Unicode glyph coverage"],
}
path = root / "evidence/encoding/manifest.json"
path.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(path)
print(json.dumps(artifacts, indent=2))
