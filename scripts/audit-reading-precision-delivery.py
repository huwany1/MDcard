# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Bind the unchanged 38 acceptance IDs to the completed independent evidence.

This final index supplements the native/algorithm/raw-performance auditors;
parsing a green table is never treated as GUI or algorithm verification.
"""
import datetime
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent
EVIDENCE = ROOT / "evidence/reading-precision/master"


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    output = EVIDENCE / "b-r29-final-criteria-audit.json"
    assert not output.exists(), "Retain prior acceptance"
    table = ROOT / "docs/READING_PRECISION_ACCEPTANCE.md"
    rows = [list(map(str.strip, line.split("|")[1:-1])) for line in table.read_text(encoding="utf-8").splitlines()
            if any(line.startswith("| " + group + "-") for group in ["A", "B", "R", "P", "D"])]
    expected = [f"{group}-{i:02}" for group, count in [("A", 11), ("B", 11), ("R", 5), ("P", 6), ("D", 5)] for i in range(1, count + 1)]
    assert [row[0] for row in rows] == expected
    assert all(row[2] == "通过" and row[3] for row in rows)
    gates = []

    def gate(name, flag="passed"):
        path = EVIDENCE / name
        value = read(path)
        assert value[flag] is True, name
        gates.append({"path": path.relative_to(ROOT).as_posix(), "sha256": sha(path)})
        return value

    build = gate("b-r29-build-inputs-independent-audit.json", "audit_passed")
    assert (build["listed_input_count"], build["actual_local_compiler_input_count"]) == (82, 70)
    binding = gate("b-r29-commit-source-binding.json")
    assert binding["commit"] == "40b5e3bb16e1658e333cf2fddf965a22d4e878ff"
    for entry in binding["inputs"]:
        assert sha(ROOT / entry["path"]) == entry["compiled_raw_sha256"], entry["path"]
    assert len(binding["inputs"]) == 82
    manifest = ROOT / "artifacts/reading-precision/stage-b-r29-source.json"
    assert sha(manifest) == "655cfdafcdd4feae4d3199206b49a39ba72c497966adec6378bcfe32d2210b55"
    perf = gate("b-r29-full-update-independent-audit.json", "audit_passed")
    assert perf["all_modes_budget_passed"] and perf["raw_probe_count"] == 48 and len(perf["rows"]) == 24 and not perf["failed_rows"]
    stages = gate("b-r29-six-stages-independent-audit.json", "audit_passed")
    assert stages["completed_matching_calls"] == 30
    windows = gate("b-r29-windows-package-independent-audit.json")
    assert len(windows["groups"]) == 8 and windows["original_cohort_passed"]
    assert len(windows["unclamped_samples"]) == 40 and len(windows["clamped_samples"]) == 2
    assert windows["maximum_unclamped_error_logical_px"] <= 2
    linux = gate("b-r29-linux-package-independent-audit.json")
    assert len(linux["groups"]) == 11 and len(linux["navigation"]) == 2
    assets = gate("b-r29-package-async-and-visual-independent-audit.json")
    assert len(assets["async_original_source_actual_layout"]) == 2
    assert len(assets["actual_screenshots_viewed"]) == 5
    for platform, count in [("windows", 22059), ("linux", 22003)]:
        post = gate(f"b-r29-extracted-{platform}-post-gui.json")
        assert post["verified_payload_files"] == count and post["exact_regular_file_set"]

    corpus = ROOT / "artifacts/reading-precision/master/frozen-corpus-20261023"
    assert sha(corpus / "inputs.jsonl") == "980f8136cc87edb9bcca12a4beecd5e7cec12c64fdcb64689d9d88805f5d69f5"
    assert sha(corpus / "answers.jsonl") == "2cdeac356af1b00545f38d79eddc5c60a6d7135ed66f40e22849e1cdc73c08a0"
    assert sha(ROOT / "src/reading_precision.rs") == "685a8475a8cf8314c05ae8bbf7a72ffdf099201613a9e992c8935366919f1af9"
    summary = EVIDENCE / "stage-b-r25-locator/score.json/summary.json"
    # The dedicated scorer and original raw results establish the denominators;
    # this index binds that already independently reviewed immutable output.
    corpus_binding = {"path": summary.relative_to(ROOT).as_posix(), "sha256": sha(summary),
                      "pure_component_sha256": sha(ROOT / "src/reading_precision.rs")}
    archive_audit_path = EVIDENCE / "b-r29-complete-package-archives-audit.json"
    archive_audits = read(archive_audit_path)
    assert len(archive_audits) == 2 and all(record["passed"] for record in archive_audits)
    packages = read(ROOT / "artifacts/reading-precision/packages/precision-b-20261024-r29/packages.json")
    for record in packages:
        path = Path(record["archive"])
        assert sha(path) == record["archive_sha256"]
        audit = next(a for a in archive_audits if a["platform"] == record["platform"])
        assert audit["archive_sha256"] == record["archive_sha256"] and audit["binary_sha256"] == record["binary_sha256"]
    result = {"passed": True, "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "product_commit": binding["commit"],
              "acceptance_source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "criteria": [{"id": r[0], "verdict": r[2], "scope_and_evidence": r[3]} for r in rows],
              "table_sha256": sha(table), "independent_gates": gates, "frozen_corpus_binding": corpus_binding,
              "complete_package_archives_audit_sha256": sha(archive_audit_path),
              "packages": packages,
              "boundary": "All original38 IDs retained and separately evidence-backed; this is a final index after actual native GUI, original-text/geometry checks, independent corpus, and complete raw-performance audit. It does not turn ignored tests, clamps, nontext fallbacks or untested platforms into precision passes. Aggregate evidence archive completion is recorded separately; immutable package documents are the pre-GUI build snapshot."}
    with output.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(result, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    print(json.dumps({"passed": True, "original_criteria": len(rows), "independent_gates": len(gates)}))


if __name__ == "__main__":
    main()
