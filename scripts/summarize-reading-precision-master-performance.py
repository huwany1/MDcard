# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Independent Windows full-update paired statistics, with raw samples retained."""
import argparse
import datetime
import hashlib
import json
import math
from pathlib import Path


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def stats(values):
    values = sorted(values)
    assert values and all(math.isfinite(x) and x >= 0 for x in values)
    return dict(count=len(values), p50=values[math.ceil(.50 * len(values)) - 1],
                p95=values[math.ceil(.95 * len(values)) - 1], maximum=values[-1])


def cohort(directory):
    record = read(directory / "result.json")
    assert record["passed"], record.get("error")
    reports = {}
    for app in record["apps"]:
        path = directory / f"{app['name']}-full-update.json"
        probe = read(path)
        assert probe["queue_dropped_samples"] == 0
        assert probe["total_samples"] == len(probe["samples"]), "Observer retention truncated the sampled interval"
        frames, wheel_frames = {}, set()
        for sample in probe["samples"]:
            frame = sample["frame"]
            frames[frame] = frames.get(frame, 0) + sample["elapsed_wall_ms"]
            if sample["wheel_input"]:
                wheel_frames.add(frame)
        reports[app["name"]] = {
            "path": app["path"], "manuscript_sha256": app["manuscript_sha256"],
            "actual_dpi": app["dpi"], "scope": probe["scope"],
            "actual_window_capture_pixels": app["window_capture_pixels"],
            "all_update_wall_ms": stats([s["elapsed_wall_ms"] for s in probe["samples"]]),
            "wheel_update_wall_ms": stats([s["elapsed_wall_ms"] for s in probe["samples"] if s["wheel_input"]]),
            "all_frame_update_sums_wall_ms": stats(list(frames.values())),
            "wheel_frame_update_sums_wall_ms": stats([frames[f] for f in wheel_frames]),
            "raw_report_sha256": digest(path), "queue_dropped_samples": 0,
        }
    return record, reports


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    assert not args.output.exists(), "Retain earlier comparisons; use a fresh output"
    baseline, before = cohort(args.baseline)
    candidate, after = cohort(args.candidate)
    for key in ["size", "diagnostics", "harness_sha256", "protocol"]:
        assert baseline[key] == candidate[key], key
    comparisons = {}
    for name in before:
        old, new = before[name], after[name]
        for key in ["path", "manuscript_sha256", "actual_dpi", "actual_window_capture_pixels", "scope"]:
            assert old[key] == new[key], key
        result = {}
        for key in ["all_update_wall_ms", "wheel_update_wall_ms",
                    "all_frame_update_sums_wall_ms", "wheel_frame_update_sums_wall_ms"]:
            original = old[key]["p95"]
            increment = new[key]["p95"] - original
            budget = max(.20 * original, 2.)
            result[key] = {"baseline": old[key], "candidate": new[key],
                           "p95_increment_ms": increment, "increment_budget_ms": budget,
                           "within_same_reference_budget": increment <= budget}
        # The frozen budget is per complete App::update invocation. Frame sums
        # are separately visible; they are not a render/display/FPS measurement.
        result["invocation_budget_passed"] = all(result[key]["within_same_reference_budget"]
                                                for key in ["all_update_wall_ms", "wheel_update_wall_ms"])
        comparisons[name] = result
    output = {
        "passed": all(c["invocation_budget_passed"] for c in comparisons.values()),
        "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "size": baseline["size"], "diagnostics": baseline["diagnostics"],
        "baseline_directory": str(args.baseline), "candidate_directory": str(args.candidate),
        "baseline_binary_sha256": baseline["binary_sha256"],
        "candidate_binary_sha256": candidate["binary_sha256"],
        "driver_sha256": baseline["harness_sha256"],
        "comparisons": comparisons,
        "baseline_raw": before, "candidate_raw": after,
        "baseline_peak_observed_combined_app_working_set_bytes": baseline["peak_observed_combined_app_working_set_bytes"],
        "candidate_peak_observed_combined_app_working_set_bytes": candidate["peak_observed_combined_app_working_set_bytes"],
        "boundary": "Nearest-rank percentiles; raw startup maxima retained. Normal Off and diagnostic On remain separate, including failures. Complete function wall includes actual product work and existing synchronous diagnostics, excludes only observer enqueue/JSON writer. It is not thread CPU, framework render, physical display latency or FPS. WorkingSet64 samples exclude renderer descendants and are not allocator peaks.",
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2), encoding="utf-8")
    print(json.dumps({k: v for k, v in output.items() if k in ["passed", "size", "diagnostics", "comparisons"]}))
    raise SystemExit(0 if output["passed"] else 1)


if __name__ == "__main__":
    main()
