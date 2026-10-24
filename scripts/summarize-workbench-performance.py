# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Compare preserved native samples against the pre-implementation contract."""
import json
from pathlib import Path

root = Path(__file__).resolve().parent.parent
reports = root / "evidence/workbench-redesign-2026-10-22/reports"


def read(relative):
    return json.loads((reports / relative).read_text("utf-8-sig"))


before = read("performance-before/result.json")
after = read("performance-final-r9/result.json")
old_transitions = read("transitions-before/result.json")
new_transitions = read("transitions-after/result.json")
old_search = read("search-before/performance.json")
new_search = read("search-after/performance.json")
gates = []


def compare(name, baseline, candidate, additive, fraction):
    limit = baseline + max(additive, baseline * fraction)
    gates.append({"metric": name, "baseline_ms": baseline, "candidate_ms": candidate,
                  "limit_ms": limit, "status": "通过" if candidate <= limit else "失败"})


compare("typing_injection_to_gpu_complete_p95", before["typing_injection_to_gpu_complete"]["p95_ms"],
        after["typing_injection_to_gpu_complete"]["p95_ms"], 2, .2)
compare("scroll_gpu_complete_p95", before["scroll"]["p95_ms"], after["scroll"]["p95_ms"], 2, .2)
compare("disk_open_to_gpu_complete", before["disk_open_to_gpu_complete_ms"],
        after["disk_open_to_gpu_complete_ms"], 20, 0)
for action in ("mode", "resize", "theme"):
    compare(action + "_observed_p95", old_transitions["actions"][action]["p95_ms"],
            new_transitions["actions"][action]["p95_ms"], 10, .2)
compare("search_visible_p95", old_search["visible"]["p95_ms"], new_search["visible"]["p95_ms"], 5, .2)
result = {"binary_sha256": after["binary_sha256"].lower(), "relative_gates": gates,
          "relative_status": "通过" if all(item["status"] == "通过" for item in gates) else "失败",
          "original_strict_gates": after["thresholds"],
          "strict_scroll_status": "通过" if after["thresholds"]["scroll_all_frames_under_16_667_ms"] else "失败",
          "scroll_outliers": {"count": after["scroll"]["over_16_667"], "samples": after["scroll"]["samples"],
                              "max_ms": after["scroll"]["max_ms"]},
          "search_existing_50ms_gates": new_search["visible"]["p95_ms"] < 50 and new_search["worker"]["max_ms"] < 50,
          "boundary": "Native SendInput/QPC through GPU completion and report-observed transitions; no display scanout measurement. All samples retained."}
output = reports / "performance-comparison.json"
output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n", "utf-8")
print(json.dumps(result, indent=2, ensure_ascii=False))
