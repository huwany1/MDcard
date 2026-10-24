# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Promote unchanged, accepted R29 payloads after final-directory GUI checks."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import tempfile

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "evidence/release-r29-2026-10-24"


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    assert not (OUT / "promotion.json").exists()
    staged = read(OUT / "staged.json")
    closing = read(ROOT / "artifacts/reading-precision/delivery/DELIVERY_SHA256.json")
    win_path = ROOT / "evidence/reading-precision/master/release-r29-promoted-20261024-reflow-read/result.json"
    linux_path = ROOT / "evidence/reading-precision/implementation/release-r29-promoted-20261024/result.json"
    win, linux = read(win_path), read(linux_path)
    assert win["passed"] and linux["passed"]
    assert Path(win["binary"]).resolve() == (ROOT / "dist/windows/MDcard.exe").resolve()
    assert win["binary_sha256"].lower() == closing["complete_runtime_packages"][0]["binary_sha256"].lower()
    assert linux["binary_sha256"] == closing["complete_runtime_packages"][1]["binary_sha256"]
    execution = read(OUT / "linux-promoted-gui-execution.json")
    assert execution["exit_code"] == 0 and execution["actual_exe"] == "/work/dist/linux/mdcard"
    assert execution["actual_renderer"] == "/work/dist/linux/renderer"
    now = datetime.datetime.now(datetime.timezone.utc).isoformat()
    records = []
    for original in staged["input_packages"]:
        platform = original["platform"]
        verified = read(OUT / (platform + "-stable-payload.json"))
        stage = ROOT / "dist" / platform
        assert verified["passed"] and Path(verified["actual_package"]).resolve() == stage.resolve()
        manifest = read(stage / "manifest.json")
        archive = ROOT / "dist" / Path(original["archive"]).name
        assert sha(archive) == original["archive_sha256"]
        assert manifest["binarySourceCommit"] == closing["product_commit"]
        records.append({**original, "stage": str(stage), "archive": str(archive),
                        "build_id": "precision-b-20261024-r29", "release_name": "R29",
                        "release_channel": "stable", "acceptance_status": "passed",
                        "promoted_at": now, "acceptance_commit": closing["acceptance_commit"],
                        "promotion_record": "evidence/release-r29-2026-10-24/promotion.json",
                        "evidence_archive": closing["evidence_archive"]["path"],
                        "evidence_archive_sha256": closing["evidence_archive"]["sha256"]})
    latest = ROOT / "dist/latest.json"
    assert latest.read_bytes() == (OUT / "latest-before.json").read_bytes(), "Another publisher changed latest"
    payload = (json.dumps(records, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    with tempfile.NamedTemporaryFile(dir=latest.parent, prefix=".latest-r29-", delete=False) as stream:
        temporary = Path(stream.name)
        stream.write(payload)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, latest)
    report = {"passed": True, "published_at": now, "product_commit": closing["product_commit"],
              "acceptance_commit": closing["acceptance_commit"], "rebuilt": False, "repacked": False,
              "latest_sha256": sha(latest), "packages": records,
              "new_directory_gui": [{"path": p.relative_to(ROOT).as_posix(), "sha256": sha(p)} for p in [win_path, linux_path]],
              "boundary": "Local official release promotion authorized by user. Accepted executable/archive/manifest bytes unchanged; embedded acceptanceCandidate is the immutable build-time snapshot. Official stable channel and completed external acceptance are recorded in latest/promotion. No new version-wide performance or full38 rerun claim; prior accepted scope preserved."}
    (OUT / "promotion.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    (ROOT / "docs/releases/2026-10-24-r29.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"passed": True, "release": "R29", "channel": "stable", "platforms": len(records)}))


if __name__ == "__main__":
    main()
