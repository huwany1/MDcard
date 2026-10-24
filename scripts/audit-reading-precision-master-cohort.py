# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Independently bind a completed Windows cohort to its frozen plan and geometry.

This audit supplements the native drivers. It does not claim that PNG hashes
constitute visual inspection, or that driver assertions are distinct documents.
"""
import argparse
import datetime
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--batch", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--driver-manifest")
    parser.add_argument("--replacement", action="append", default=[], metavar="JOB=RESULT")
    args = parser.parse_args()
    batch = (ROOT / args.batch).resolve()
    output = (ROOT / args.output).resolve()
    assert batch.is_relative_to(ROOT) and output.is_relative_to(ROOT)
    assert not output.exists(), "Retain earlier audit results"
    plan_path = batch / "plan-before-execution.json"
    plan, status = read(plan_path), read(batch / "execution-status.json")
    assert len(status) == len(plan["jobs"]), "Cohort is not complete"
    candidate = plan["binary_sha256"].lower()
    assert digest(Path(plan["binary"])) == candidate
    frozen_drivers = {}
    if args.driver_manifest:
        manifest_path = ROOT / args.driver_manifest
        manifest = read(manifest_path)
        assert manifest["plan_sha256"] == digest(plan_path)
        for entry in manifest["scripts"]:
            assert digest(manifest_path.parent / entry["path"]) == entry["sha256"]
            frozen_drivers[entry["path"]] = entry["sha256"]
    elif plan.get("drivers"):
        for entry in plan["drivers"]:
            snapshot = Path(entry["snapshot"]).resolve()
            assert snapshot.is_relative_to(ROOT) and digest(snapshot) == entry["sha256"]
            frozen_drivers[entry["path"]] = entry["sha256"]
    replacements = dict(value.split("=", 1) for value in args.replacement)
    assert set(replacements).issubset({job["name"] for job in plan["jobs"]})
    runs, unclamped, clamps, source_oracles, original_failures = [], [], [], [], []
    for job, done in zip(plan["jobs"], status):
        assert done["name"] == job["name"]
        result_path = ROOT / job["result"]
        assert digest(result_path) == done["result_sha256"]
        result = read(result_path)
        replaced = job["name"] in replacements
        if not done["passed"]:
            original_failures.append({"name": job["name"], "result": job["result"],
                                      "sha256": done["result_sha256"], "error": result.get("error"),
                                      "partial_assertions": len(result.get("cases", []))})
            assert replaced, "Failed original run needs an explicit retained complete recheck"
        else:
            assert done["exit_code"] == 0 and not replaced
        if replaced:
            result_path = ROOT / replacements[job["name"]]
            result = read(result_path)
            assert result["base_document"] == job["parameters"]["BaseDocument"]
            assert len(result["cases"]) == 10, "External-edit recheck must execute all ten original classes"
        if isinstance(result, list):  # The legacy runner retains all nested runs.
            assert result and all(item["passed"] for item in result)
            nested = []
            for item in result:
                child_path = ROOT / item["result"]
                child = read(child_path)
                assert child["passed"] is True
                assert child["binary_sha256"].lower() == candidate
                assert item["binary_sha256"].lower() == candidate
                nested.append({"scenario": item["scenario"], "result": item["result"],
                               "sha256": digest(child_path), "assertions": len(child.get("cases", []))})
            runs.append({"name": job["name"], "result": job["result"],
                         "sha256": done["result_sha256"], "legacy_groups": nested})
            continue
        assert result["passed"] is True
        assert result.get("binary_sha256", result.get("exe_sha256", "")).lower() == candidate
        if "harness_sha256" in result:
            expected_driver = digest(ROOT / job["script"]) if replaced else frozen_drivers.get(
                job["script"], digest(ROOT / job["script"]))
            assert expected_driver == result["harness_sha256"].lower()
        for case in result.get("cases", []):
            assert case["status"] in ("通过", "PASS", "passed")
            e = case.get("evidence", {})
            if not isinstance(e, dict) or "dy_logical_px" not in e:
                continue
            actual, tail = e.get("actual", e.get("after")), e.get("tail")
            expected = e.get("expected", e.get("old", e.get("before")))
            assert actual["origin"] == "galley-current-pass"
            assert actual["source_byte"] == actual["grapheme_range"]["start"]
            assert actual["source_end"] == actual["grapheme_range"]["end"]
            assert 0 <= actual["source_byte"] < actual["source_end"]
            measured = abs(actual["viewport_y"] - expected["viewport_y"])
            assert math.isclose(measured, e["dy_logical_px"], abs_tol=1e-6)
            clipped = bool(e.get("boundary_clamped", e.get("clamped", actual.get("clamped"))))
            assert clipped == bool(actual.get("clamped")), "Outer clamp classification disagrees with actual geometry"
            if not e.get("oracle"):
                assert (actual["source_byte"], actual["source_end"], actual["source_text"]) == (
                    expected["source_byte"], expected["source_end"], expected["source_text"])
            item = {"run": job["name"], "case": case["name"],
                    "source_byte": actual["source_byte"], "source_end": actual["source_end"],
                    "source_text": actual["source_text"], "dy_logical_px": measured}
            if clipped:
                item["reason"] = e.get("reason") or actual.get("clamp_reason")
                assert item["reason"]
                clamps.append(item)
            else:
                assert measured <= 2
                unclamped.append(item)
            if tail:
                assert e["stable_tail_ms"] >= 2000
                assert (tail["source_byte"], tail["source_end"], tail["source_text"]) == (
                    actual["source_byte"], actual["source_end"], actual["source_text"])
                reference = actual if clipped else expected
                assert abs(tail["viewport_y"] - reference["viewport_y"]) <= 2
            oracle = e.get("oracle")
            if oracle and oracle["category"] == "eligible":
                assert (actual["source_byte"], actual["source_end"], actual["source_text"]) == (
                    oracle["expected_source_byte"], oracle["expected_source_end"], oracle["original_grapheme"])
                scratch = ROOT / "artifacts/reading-precision/master" / result["stage"]
                if oracle["kind"] == "undo-prefix":
                    new_raw = (scratch / "independent-expected-new.md").read_bytes()
                    old_raw = (scratch / "native-paste-prefix.txt").read_bytes() + new_raw
                elif "BaseDocument" not in job["parameters"]:
                    assert job["script"].endswith("master-secondary-windows.ps1")
                    assert oracle["kind"] == "prefix-insert" and len(oracle["operations"]) == 1
                    operation = oracle["operations"][0]
                    assert operation["kind"] == "splice" and operation["start_scalar"] == 0 and operation["deleted_scalars"] == 0
                    new_raw = (scratch / "workspace" / case["name"].split(" secondary ")[0]).read_bytes()
                    # Recover the old text by the independently recorded edit,
                    # then verify its complete original SHA below.
                    old_raw = new_raw.decode("utf-8")[operation["inserted_scalars"]:].encode("utf-8")
                else:
                    fixture_dir = scratch / "workspace"
                    suffix = Path(job["parameters"]["BaseDocument"]).suffix
                    new_raw = (fixture_dir / ("relocate-" + oracle["kind"] + suffix)).read_bytes()
                    old_raw = (fixture_dir / job["parameters"]["BaseDocument"]).read_bytes()
                assert hashlib.sha256(new_raw).hexdigest() == oracle["new_sha256"]
                assert hashlib.sha256(old_raw).hexdigest() == oracle["old_sha256"]
                for raw, start, end in (
                    (old_raw, oracle["old_source_byte"], oracle["old_source_end"]),
                    (new_raw, actual["source_byte"], actual["source_end"])):
                    assert raw[start:end].decode("utf-8") == oracle["original_grapheme"]
                    # Strict prefix decode independently rejects an interior UTF-8 boundary.
                    raw[:start].decode("utf-8")
                logical_line = new_raw[:actual["source_byte"]].count(b"\n")
                assert logical_line == actual["logical_line"]
                source_oracles.append({"run": job["name"], "kind": oracle["kind"],
                                       "old_sha256": oracle["old_sha256"], "new_sha256": oracle["new_sha256"],
                                       "byte": actual["source_byte"], "logical_line": logical_line})
        pngs = []
        for png in sorted(result_path.parent.glob("*.png")):
            assert png.read_bytes()[:8] == b"\x89PNG\r\n\x1a\n"
            pngs.append({"path": png.relative_to(ROOT).as_posix(), "sha256": digest(png)})
        inputs = result_path.parent / "native-key-input.jsonl"
        events = [json.loads(line) for line in inputs.read_text(encoding="utf-8-sig").splitlines()] if inputs.exists() else []
        assert all(event["owned"] is True and event["pid"] == event["foreground_pid"] for event in events)
        runs.append({"name": job["name"], "result": result_path.relative_to(ROOT).as_posix(),
                     "sha256": digest(result_path), "rechecked_after_retained_failure": replaced,
                     "assertions": len(result.get("cases", [])), "screenshots": pngs,
                     "owned_keyboard_events": len(events)})
    record = {"passed": True, "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "binary_sha256": candidate, "plan_sha256": digest(plan_path), "groups": runs,
              "original_cohort_passed": not original_failures, "retained_original_failures": original_failures,
              "unclamped_samples": unclamped, "clamped_samples": clamps,
              "maximum_unclamped_error_logical_px": max((x["dy_logical_px"] for x in unclamped), default=None),
              "independently_redecoded_external_oracles": source_oracles,
              "boundary": "Completed frozen cohort only. Raw UTF8/oracle and measured coordinate arithmetic checked here; complete grapheme boundaries and owned input also checked by native drivers. PNG integrity is not visual inspection. Clamps excluded from unclamped precision denominator; assertion counts are not unique documents."}
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(record, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"passed": True, "groups": len(runs), "unclamped": len(unclamped),
                      "clamps": len(clamps), "external_oracles": len(source_oracles),
                      "maximum_error": record["maximum_unclamped_error_logical_px"]}))


if __name__ == "__main__":
    main()
