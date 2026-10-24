# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Independently inspect the isolated R27 diagnostic investigation, not a budget pass."""
import datetime
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASE = ROOT / "artifacts/reading-precision/r27-full-update-qa"


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def quantiles(values):
    values = sorted(values)
    assert values and all(math.isfinite(value) and value >= 0 for value in values)
    return {"n": len(values), "p50_ms": values[len(values) // 2],
            "p95_ms": values[min(len(values) - 1, int(len(values) * .95))], "max_ms": values[-1]}


def main():
    output = ROOT / "evidence/reading-precision/master/r27-full-update-phase-independent-audit.json"
    assert not output.exists()
    manifest = read(BASE / "result-manifest.json")
    status = read(BASE / "execution-status.json")
    assert len(status) == 4 and all(run["exit_code"] == 0 for run in status)
    records = []
    for item in manifest["files"]:
        path = ROOT / Path(item["path"].removeprefix("/work/"))
        assert path.resolve().is_relative_to(ROOT) and path.stat().st_size == item["bytes"]
        assert sha(path) == item["sha256"]
        if not path.name.endswith(".report-phases.json"):
            continue
        phase = read(path)
        original_path = path.with_name(path.name.removesuffix(".report-phases.json"))
        original = read(original_path)
        samples, published = phase["samples"], original["samples"]
        assert phase["overflow"] == original["queue_dropped_samples"] == 0
        assert len(samples) == phase["retained"] == phase["total_calls"] < phase["capacity"]
        assert len(published) == original["total_samples"] <= len(samples)
        names = phase["phase_order"]
        assert len(names) == 12
        for index, sample in enumerate(samples):
            assert sample["call"] == index + 1 and len(sample["phase_wall_ms"]) == 12
            times = sample["phase_wall_ms"]
            assert all(math.isfinite(value) and value >= 0 for value in times)
            remainder = sample["elapsed_wall_ms"] - sum(times[index] for index in (0, 4, 8))
            assert math.isclose(remainder, sample["remaining_update_wall_ms"], abs_tol=1e-8)
            assert remainder >= -1e-8
            for start in (0, 4, 8):
                assert times[start] + 1e-8 >= sum(times[start + 1:start + 4])
            if index < len(published):
                before = published[index]
                assert all(before[key] == sample[key] for key in ("frame", "pass", "wheel_input"))
                assert before["elapsed_wall_ms"] + 1e-8 >= sample["elapsed_wall_ms"]
        selected = {}
        for category in ("all", "wheel"):
            rows = samples if category == "all" else [sample for sample in samples if sample["wheel_input"]]
            if not rows:
                selected[category] = {"n": 0}
                continue
            selected[category] = {
                "full_phase_range": quantiles([sample["elapsed_wall_ms"] for sample in rows]),
                "remaining_update": quantiles([max(0., sample["remaining_update_wall_ms"]) for sample in rows]),
                "phases": {name: quantiles([sample["phase_wall_ms"][index] for sample in rows])
                           for index, name in enumerate(names)},
                "slowest_five": sorted(rows, key=lambda sample: sample["elapsed_wall_ms"], reverse=True)[:5]}
        tail = samples[len(published):]
        common_wheel = [sample["elapsed_wall_ms"] for sample in published if sample["wheel_input"]]
        records.append({"path": path.relative_to(ROOT).as_posix(), "sha256": sha(path),
                        "phase_calls": len(samples), "published_calls": len(published),
                        "unpublished_suffix_calls": tail, "suffix_wheel_calls": sum(row["wheel_input"] for row in tail),
                        "published_wheel": quantiles(common_wheel) if common_wheel else {"n": 0},
                        "categories": selected})
    assert len(records) == 16
    result = {"audit_passed": True, "formal_budget_pass_claim": False,
              "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "manifest_sha256": sha(BASE / "result-manifest.json"), "datasets": records,
              "boundary": "All phase calls retained; original observer's published ordered prefix is joined by ordinal and independently checked for frame/pass/wheel. Unpublished suffix remains explicit, including any wheel calls. No queue drops or phase overflow. Complete phase range and published original range are different scopes. Added in-memory observation overhead remains; this does not replace failed formal R26 budget runs."}
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"audit_passed": True, "datasets": len(records),
                      "unpublished_suffix_calls": sum(len(row["unpublished_suffix_calls"]) for row in records),
                      "suffix_wheel_calls": sum(row["suffix_wheel_calls"] for row in records)}))


if __name__ == "__main__":
    main()
