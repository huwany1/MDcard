# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Recompute full-update acceptance from retained raw probe samples."""
import argparse
import datetime
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def quantiles(values, platform):
    values = sorted(values)
    assert values and all(math.isfinite(v) and v >= 0 for v in values)
    index = lambda p: (math.ceil(p * len(values)) - 1 if platform == "windows"
                       else min(len(values) - 1, int(p * len(values))))
    return {"n": len(values), "p50_ms": values[index(.5)],
            "p95_ms": values[index(.95)], "max_ms": values[-1]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--revision", choices=("r25", "r26", "r28", "r29"), default="r25")
    parser.add_argument("--platform", choices=("both", "windows", "linux"), default="both")
    parser.add_argument("--baseline-manifest", help="R29 requires the rebuilt 264d baseline with the same observer")
    options = parser.parse_args()
    suffix = "" if options.platform == "both" else "-" + options.platform
    destination = ROOT / ("evidence/reading-precision/master/b-" + options.revision +
                          "-full-update-independent-audit" + suffix + ".json")
    assert not destination.exists(), "Retain earlier audits"
    expected = {
        "windows": {"baseline": "503e29a165caec5929e713551a1f67a833c02c1575d80167b94f9c0f7bd7c8c6",
                    "candidate": "75c819cbbd2bff9a0c786647dda99dfcad8006268df2f47ff573c7cf7270b6e7"},
        "linux": {"baseline": "99391251e6ee53636a0307c3c98935b6078e79d6b06e2b2ac4d1236389928302",
                  "candidate": "e8bb4de31266b5e519f110c489cc013a71a6142aac4eb39f3471fa0e5bfbe386"}}
    if options.revision == "r26":
        expected["windows"]["candidate"] = "13d8e38d215f3b4a5bba2a61944a94ce3d7ad1dd93133ab9ff0148a046939187"
        expected["linux"]["candidate"] = "c72763777aec9fb357f6f2800b82b64c79c19b69c35702e591a4316ccfcaf647"
    manifest_path = None
    baseline_manifest_path = None
    if options.revision in ("r28", "r29"):
        manifest_path = ROOT / ("artifacts/reading-precision/stage-b-" + options.revision + "-source.json")
        manifest = read(manifest_path)
        for platform in ("windows", "linux"):
            expected[platform]["candidate"] = manifest["binaries"][platform]["sha256"]
    if options.revision == "r29":
        if not options.baseline_manifest:
            parser.error("R29 must explicitly bind the rebuilt baseline manifest")
        baseline_manifest_path = (ROOT / options.baseline_manifest).resolve()
        assert baseline_manifest_path.is_relative_to(ROOT)
        baseline_manifest = read(baseline_manifest_path)
        candidate_observer = manifest.get("observer_sha256") or manifest["source_sha256"]["src/update_probe.rs"]
        baseline_observer = baseline_manifest.get("observer_sha256") or baseline_manifest["source_sha256"]["src/update_probe.rs"]
        assert candidate_observer == baseline_observer
        for platform in ("windows", "linux"):
            expected[platform]["baseline"] = baseline_manifest["binaries"][platform]["sha256"]
    platforms = ("windows", "linux") if options.platform == "both" else (options.platform,)
    rows, sources = [], {}
    if manifest_path:
        sources[manifest_path.relative_to(ROOT).as_posix()] = sha(manifest_path)
    if baseline_manifest_path:
        sources[baseline_manifest_path.relative_to(ROOT).as_posix()] = sha(baseline_manifest_path)
    for platform in platforms:
        cohort = platform + "-final-" + options.revision + "-perf"
        aggregate = ROOT / "artifacts/reading-precision" / cohort
        plan_path = aggregate / "plan-before-first-measurement.json"
        sources[plan_path.relative_to(ROOT).as_posix()] = sha(plan_path)
        status = read(aggregate / "execution-status.json")
        assert len(status) == 12 and all(item["exit_code"] == 0 for item in status)
        linux_summary = read(aggregate / "comparison.json") if platform == "linux" else None
        for mode in ("off", "on"):
            for size in ("ordinary", "one-mib", "ten-mib"):
                reports, probes = {}, {}
                for role in ("baseline", "candidate"):
                    run = cohort + "-" + mode + "-" + role + "-" + size
                    base = ROOT / "evidence/reading-precision" / ("master" if platform == "windows" else "implementation") / run
                    record = read(base / "result.json")
                    assert record["passed"] and record["binary_sha256"].lower() == expected[platform][role]
                    reports[role] = record
                    for app in ("a", "b"):
                        path = base / (app + "-full-update.json")
                        raw = read(path)
                        assert raw["queue_dropped_samples"] == 0
                        assert raw["total_samples"] == len(raw["samples"])
                        if options.revision == "r29":
                            assert raw["flush_complete"] is True
                            assert raw["submitted_samples"] == raw["total_samples"]
                            assert raw["processed_samples"] == raw["total_samples"]
                            assert raw["retention_dropped_samples"] == 0
                        sources[path.relative_to(ROOT).as_posix()] = sha(path)
                        values = [sample["elapsed_wall_ms"] for sample in raw["samples"]]
                        wheels = [sample["elapsed_wall_ms"] for sample in raw["samples"] if sample["wheel_input"]]
                        assert len(wheels) > 30
                        probes[role, app] = {"all": quantiles(values, platform),
                                             "wheel": quantiles(wheels, platform), "scope": raw["scope"]}
                for key in ("harness_sha256", "protocol", "size", "diagnostics"):
                    if key in reports["baseline"]:
                        assert reports["baseline"][key] == reports["candidate"][key]
                comparison = read(aggregate / (mode + "-" + size + "-comparison.json")) if platform == "windows" else None
                for app, format_name in (("a", "Markdown"), ("b", "plain text")):
                    if platform == "windows":
                        selected = comparison["comparisons"][app]
                    else:
                        selected = next(item for item in linux_summary["rows"]
                                        if (item["mode"], item["size"], item["format"]) == (mode, size, format_name))["comparisons"]
                    assert probes["baseline", app]["scope"] == probes["candidate", app]["scope"]
                    metrics = {}
                    for category in ("all", "wheel"):
                        before, after = probes["baseline", app][category], probes["candidate", app][category]
                        reported = selected[category + "_update_wall_ms" if platform == "windows" else category + "_update"]
                        for role, actual in (("baseline", before), ("candidate", after)):
                            fields = {"n": "count", "p50_ms": "p50", "p95_ms": "p95", "max_ms": "maximum"} if platform == "windows" else {k: k for k in actual}
                            for key, other in fields.items():
                                assert math.isclose(actual[key], reported[role][other], abs_tol=1e-9, rel_tol=1e-12)
                        budget = max(.2 * before["p95_ms"], 2.)
                        delta = after["p95_ms"] - before["p95_ms"]
                        passed = delta <= budget
                        published = reported["within_same_reference_budget" if platform == "windows" else "passed"]
                        assert published == passed
                        metrics[category] = {"baseline": before, "candidate": after,
                                             "increment_ms": delta, "budget_ms": budget, "passed": passed}
                    rows.append({"platform": platform, "mode": mode, "size": size,
                                 "format": format_name, "metrics": metrics,
                                 "passed": all(value["passed"] for value in metrics.values())})
    failed = [row for row in rows if not row["passed"]]
    if options.revision == "r25":
        expected_failures = [("linux", "on", "ordinary", "Markdown"),
                             ("linux", "on", "ordinary", "plain text")] if "linux" in platforms else []
        assert [(row["platform"], row["mode"], row["size"], row["format"]) for row in failed] == expected_failures
    # Audit validity and product budget acceptance are deliberately separate.
    result = {"audit_passed": True, "all_modes_budget_passed": not failed,
              "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "revision": options.revision, "platforms": platforms,
              "raw_probe_count": len(sources) - len(platforms) - int(manifest_path is not None) - int(baseline_manifest_path is not None),
              "rows": rows, "failed_rows": failed,
              "source_sha256": sources,
              "boundary": "Independent raw-sample recomputation of the selected platforms' original frozen quantiles and all/wheel invocation P95 budgets. All samples, maxima and zero-drop checks retained. Any failed rows remain failures; wall-clock intervals are not thread CPU, display latency or FPS."}
    destination.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"audit_passed": True, "raw_probes": result["raw_probe_count"],
                      "format_rows": len(rows), "failed_rows": len(failed), "all_modes_budget_passed": not failed}))


if __name__ == "__main__":
    main()
