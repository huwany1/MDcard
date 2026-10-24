# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Final read-only verification after accepted release promotion and cleanup."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "evidence/release-r29-2026-10-24"


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    output = OUT / "post-cleanup-verification.json"
    assert not output.exists()
    latest = read(ROOT / "dist/latest.json")
    promotion = read(OUT / "promotion.json")
    assert latest == promotion["packages"] and sha(ROOT / "dist/latest.json") == promotion["latest_sha256"]
    packages = []
    for record in latest:
        assert record["release_channel"] == "stable" and record["acceptance_status"] == "passed"
        stage = ROOT / "dist" / record["platform"]
        assert Path(record["stage"]).resolve() == stage.resolve()
        payload = read(OUT / (record["platform"] + "-after-cleanup.json"))
        assert payload["passed"] and payload["verified_payload_files"] == record["manifest_files"]
        manifest = read(stage / "manifest.json")
        assert manifest["buildId"] == record["build_id"] == "precision-b-20261024-r29"
        assert manifest["binarySourceCommit"] == "40b5e3bb16e1658e333cf2fddf965a22d4e878ff"
        path = Path(record["archive"])
        assert path.parent.resolve() == (ROOT / "dist").resolve() and sha(path) == record["archive_sha256"]
        packages.append(dict(platform=record["platform"], files=payload["verified_payload_files"],
                             binary_sha256=record["binary_sha256"], archive_sha256=record["archive_sha256"]))
    closing = read(ROOT / "artifacts/reading-precision/delivery/DELIVERY_SHA256.json")
    evidence = []
    for record in [closing["evidence_archive"], closing["evidence_sidecar"], closing["source_snapshot"], closing["source_patch"]]:
        path = ROOT / record["path"]
        assert path.stat().st_size == record["bytes"] and sha(path) == record["sha256"]
        evidence.append(dict(path=record["path"], sha256=record["sha256"]))
    plan, cleanup = read(OUT / "cleanup-plan-final.json"), read(OUT / "cleanup-result.json")
    assert cleanup["complete"] and cleanup["retained_changed"] == 0
    assert cleanup["deleted_targets"] == len(plan["targets"])
    assert all(not os.path.lexists(ROOT / entry["path"]) for entry in plan["targets"])
    tracked = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode().split("\0")
    assert all((ROOT / name).is_file() for name in tracked if name)
    binding = read(ROOT / "evidence/reading-precision/master/b-r29-commit-source-binding.json")
    assert binding["passed"] and len(binding["inputs"]) == 82
    for entry in binding["inputs"]:
        assert sha(ROOT / entry["path"]) == entry["compiled_raw_sha256"], entry["path"]
    changes = subprocess.check_output(["git", "diff", "--name-only", "40b5e3bb16e1658e333cf2fddf965a22d4e878ff", "--",
        "src", "vendor", "Cargo.toml", "Cargo.lock", "assets", "locales", "renderer"], cwd=ROOT, text=True).splitlines()
    assert all(name.startswith("assets/branding/") for name in changes), changes
    native = subprocess.check_output(["docker", "exec", "mdcard-dev", "python3", "-c",
        "import os,json;from pathlib import Path;r=Path('/work/dist/linux');names=['mdcard','renderer/runtime/node','renderer/runtime/chromium/chrome','renderer/runtime/java/bin/java','renderer/runtime/pandoc/pandoc'];assert all(os.access(r/n,os.X_OK) for n in names);assert not any(Path('/work/target').iterdir());print(json.dumps({'runtime_permissions_passed':True,'project_target_empty':True}))"], text=True)
    result = dict(passed=True, created_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  latest_sha256=sha(ROOT / "dist/latest.json"), packages=packages, evidence=evidence,
                  all_planned_targets_absent=True, tracked_source_preserved=True,
                  actual_compiled_82_inputs_unchanged=True, concurrent_unpublished_branding_paths=changes,
                  native_linux=json.loads(native),
                  boundary="Fresh final-directory native GUI was run before cleanup; complete payload and current archives/evidence SHA rechecked afterwards. No rebuild/repack/repeated performance or universal-platform claim. Concurrent branding additions have their own Git commit, are outside R29's actual compiled82 inputs and were neither modified nor republished by this task.")
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"passed": True, "runtime_files": sum(p["files"] for p in packages), "deleted_targets_absent": cleanup["deleted_targets"]}))


if __name__ == "__main__":
    main()
