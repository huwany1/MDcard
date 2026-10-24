# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Inventory all retained native Windows acceptance runs without hiding failures."""
import argparse
import collections
import datetime
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--source-manifest", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    output = (ROOT / args.output).resolve()
    assert output.is_relative_to(ROOT / "evidence/reading-precision/master")
    assert not output.exists(), "Retain earlier inventories; choose a fresh output"
    manifest_path = ROOT / args.source_manifest
    manifest = json.loads(manifest_path.read_text(encoding="utf-8-sig"))
    mismatch = [path for path, sha in manifest["source_sha256"].items()
                if hashlib.sha256((ROOT / path).read_bytes()).hexdigest() != sha]
    assert not mismatch, f"Product sources have changed since candidate freeze: {mismatch}"
    wanted = args.binary_sha256.lower()
    runs = []
    files = list((ROOT / "evidence/reading-precision/master").glob("*/result.json"))
    for pattern in ("precision-a-*/result.json", "precision-b-*/result.json"):
        files += list((ROOT / "evidence/reading-state/master").glob(pattern))
    for path in sorted(files):
        raw = path.read_bytes()
        record = json.loads(raw.decode("utf-8-sig"))
        sha = str(record.get("binary_sha256", "")).lower()
        if not sha:
            continue
        cases = record.get("cases", [])
        text_samples = []
        clamps = []
        for case in cases:
            evidence = case.get("evidence", {})
            if "dy_logical_px" not in evidence:
                continue
            sample = {"case": case["name"], "dy_logical_px": evidence["dy_logical_px"]}
            if evidence.get("boundary_clamped", evidence.get("clamped", False)):
                sample["reason"] = evidence.get("reason", evidence.get("actual", {}).get("clamp_reason"))
                clamps.append(sample)
            else:
                sample["original_source_byte"] = evidence.get("actual", {}).get("source_byte")
                text_samples.append(sample)
        runs.append({"path": str(path.relative_to(ROOT)).replace("\\", "/"),
                     "result_sha256": hashlib.sha256(raw).hexdigest(),
                     "binary_sha256": sha, "current_candidate": sha == wanted,
                     "verification_id": record.get("verification_id"),
                     "stage": record.get("stage"), "passed": record.get("passed") is True,
                     "error": record.get("error"), "assertions_recorded": len(cases),
                     "unclamped_text_samples": text_samples, "clamped_samples": clamps,
                     "manuscript_bytes_unchanged": record.get("manuscript_bytes_unchanged"),
                     "started": record.get("started"), "finished": record.get("finished")})
    current = [run for run in runs if run["current_candidate"]]
    samples = [sample for run in current if run["passed"] for sample in run["unclamped_text_samples"]]
    counts = collections.Counter("passed" if run["passed"] else "failed" for run in current)
    result = {"created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "candidate_binary_sha256": wanted,
              "candidate_source_digest": manifest.get("source_digest") or hashlib.sha256(
                  json.dumps(manifest["source_sha256"], sort_keys=True, separators=(",", ":")).encode("utf-8")
              ).hexdigest(),
              "source_digest_method": "manifest source_digest when supplied; otherwise SHA256 of sorted compact source_sha256 JSON",
              "source_manifest_sha256": hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
              "source_manifest": str(manifest_path.relative_to(ROOT)).replace("\\", "/"),
              "source_paths_verified": len(manifest["source_sha256"]),
              "current_native_runs": dict(counts),
              "current_passed_run_assertions": sum(run["assertions_recorded"] for run in current if run["passed"]),
              "current_passed_run_unclamped_text_samples": len(samples),
              "current_passed_run_maximum_unclamped_y_error_logical_px": max((s["dy_logical_px"] for s in samples), default=None),
              "all_retained_runs": runs,
              "boundary": "Each run is retained, including partial passes and harness failures. Assertions are not unique documents, graphemes, or algorithm corpus cases. This inventory does not itself prove screenshots or omitted acceptance items."}
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({k: v for k, v in result.items() if k != "all_retained_runs"}, ensure_ascii=True))


if __name__ == "__main__":
    main()
