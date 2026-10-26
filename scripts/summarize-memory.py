#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
"""Summarize paired native runs without filtering slow frames or high memory."""
import argparse
import hashlib
import json
import math
import statistics
from pathlib import Path


def distribution(values):
    values = sorted(float(value) for value in values)
    if not values:
        return {"n": 0}
    return {"n": len(values), "median": statistics.median(values),
            "p95": values[max(0, math.ceil(len(values) * .95) - 1)], "max": max(values)}


def load(directory):
    data = json.loads((directory / "measurements.json").read_text(encoding="utf-8-sig"))
    result = {}
    for run in data:
        saved = (directory / run["label"] / "manuscript.md").read_bytes()
        line = b"A measured manuscript paragraph with plain ASCII text for consistent byte counts. \n"
        expected = (line * math.ceil(run["source_bytes"] / len(line)))[:run["source_bytes"]] + b"\nRecovery measurement edit." + b"x" * 120
        if saved != expected:
            raise ValueError(f"Committed input/manual-save mismatch: {directory / run['label']}")
        samples = [json.loads(line) for line in
                   (directory / run["label"] / "samples.jsonl").read_text(encoding="utf-8-sig").splitlines()]
        commits = {}
        for sample in samples:
            checkpoint = sample.get("checkpoint") or {}
            if checkpoint.get("committed_at", 0) > 0:
                commits[checkpoint["committed_at"]] = checkpoint["worker_ms"]
        stages = {}
        for stage in sorted({sample["stage"] for sample in samples}):
            rows = [sample for sample in samples if sample["stage"] == stage]
            stages[stage] = {"n": len(rows),
                             "max_working_set_mib": max(row["working_set"] for row in rows) / 2**20,
                             "max_private_bytes_mib": max(row["private_bytes"] for row in rows) / 2**20}
        result[run["label"]] = {
            "mib": run["mib"], "recovery": run["recovery"], "source_bytes": run["source_bytes"],
            "exe_sha256": run["exe_sha256"], "samples": len(samples),
            "saved_text_exact": True, "saved_bytes": len(saved),
            "saved_text_sha256": hashlib.sha256(saved).hexdigest(),
            "native_peak_working_set_mib": run["peak_working_set"] / 2**20,
            "sampled_peak_working_set_mib": max(sample["working_set"] for sample in samples) / 2**20,
            "sampled_peak_private_bytes_mib": max(sample["private_bytes"] for sample in samples) / 2**20,
            "private_bytes_end_mib": run["private_bytes_end"] / 2**20,
            "sample_interval_ms": distribution(b["at"] - a["at"] for a, b in zip(samples, samples[1:])),
            "input_update_cpu_ms": distribution(run["input_cpu_ms"]),
            "all_update_cpu_ms": distribution(run["update_cpu_ms"]),
            "checkpoint_worker_ms": distribution(commits.values()),
            "observed_max_writers": max((sample.get("writers") or 0) for sample in samples),
            "observed_max_pending_versions": max((sample.get("checkpoint") or {}).get("pending_versions", 0) for sample in samples),
            "stages": stages,
        }
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    before, after = load(args.before), load(args.after)
    comparison = []
    for label in before.keys() & after.keys():
        old, new = before[label], after[label]
        comparison.append({"label": label, "before": old, "after": new,
                           "working_set_peak_reduction_percent": 100 * (1 - new["native_peak_working_set_mib"] / old["native_peak_working_set_mib"]),
                           "private_bytes_peak_reduction_percent": 100 * (1 - new["sampled_peak_private_bytes_mib"] / old["sampled_peak_private_bytes_mib"])})
    comparison.sort(key=lambda row: (row["after"]["mib"], row["after"]["recovery"]))
    args.output.write_text(json.dumps({"comparison": comparison,
        "measurement": "Process WorkingSet64 and PrivateMemorySize64, native PeakWorkingSet64; sampler sleeps 50ms plus actual read overhead (see observed intervals); input timing is application update wall time, not OS/GPU presentation latency.",
        "scenario": "Independent process per size and recovery choice; original autosave=false; 120 committed clipboard pastes, scroll, Read/Write switch, checkpoint, manual save; UI/performance metadata only; no recovery body in diagnostic output."}, ensure_ascii=False, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
