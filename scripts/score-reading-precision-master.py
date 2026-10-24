# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Score frozen independent provenance answers, retaining all denominators."""
import argparse
import collections
import datetime
import hashlib
import json
import math
from pathlib import Path


def sha(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def read_rows(path):
    result = {}
    if not Path(path).exists():
        return result
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        try:
            row = json.loads(line)
        except ValueError:
            # A truncated final row is an incomplete case, not a deleted denominator.
            continue
        if row["case_id"] in result:
            raise ValueError("Duplicate evaluation case " + row["case_id"])
        result[row["case_id"]] = row
    return result


def summary(values):
    values = sorted(values)
    if not values:
        return dict(count=0, p50=None, p95=None, maximum=None)
    return dict(count=len(values), p50=values[math.ceil(len(values) * .5) - 1],
                p95=values[math.ceil(len(values) * .95) - 1], maximum=values[-1])


def rate(value, denominator):
    return value / denominator if denominator else None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--corpus", required=True)
    parser.add_argument("--results", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--binary", required=True)
    args = parser.parse_args()
    corpus = Path(args.corpus)
    freeze = json.loads((corpus / "freeze.json").read_text(encoding="utf-8"))
    assert sha(corpus / freeze["inputs"]) == freeze["inputs_sha256"]
    assert sha(corpus / freeze["answers"]) == freeze["answers_sha256"]
    answers = read_rows(corpus / freeze["answers"])
    inputs = read_rows(corpus / freeze["inputs"])
    observations = read_rows(args.results)
    assert len(answers) == freeze["cases"] and answers.keys() == inputs.keys()
    assert observations.keys() <= answers.keys(), "Unexpected cases in evaluation output"
    cases = []
    counts = collections.Counter()
    kinds = collections.defaultdict(collections.Counter)
    for case_id, truth in answers.items():
        result = observations.get(case_id, {})
        resolution = result.get("resolution", {})
        # The stable API contract uses explicit exact/source_byte fields. Missing
        # precision cannot silently turn a lower-tier result into a correct hit.
        exact = resolution.get("exact") is True
        byte = resolution.get("source_byte")
        observed_range = resolution.get("grapheme_range", {})
        eligible = truth["category"] == "eligible"
        correct_byte = eligible and byte == truth["expected_byte"]
        in_bounds = isinstance(byte, int) and 0 <= byte <= len(inputs[case_id]["new"].encode("utf-8"))
        legal_boundary = False
        actual_text = None
        if in_bounds:
            try:
                new_bytes = inputs[case_id]["new"].encode("utf-8")
                remainder = new_bytes[byte:].decode("utf-8")
                range_start = observed_range.get("start")
                range_end = observed_range.get("end")
                legal_boundary = (range_start == byte and isinstance(range_end, int)
                                  and byte <= range_end <= len(new_bytes))
                if legal_boundary:
                    new_bytes[range_end:].decode("utf-8")
                actual_text = remainder[:len(truth["expected_grapheme"])]
            except UnicodeDecodeError:
                legal_boundary = False
        correct_range = (eligible and observed_range.get("start") == truth["expected_byte"]
                         and observed_range.get("end") == truth["expected_byte"]
                         + len(truth["expected_grapheme"].encode("utf-8")))
        correct = (exact and resolution.get("tier") == "text" and correct_byte and correct_range
                   and actual_text == truth["expected_grapheme"] and legal_boundary)
        false_exact = exact and not correct
        status = result.get("status", "missing-or-timeout")
        counters = {
            "total": 1, "eligible": int(eligible), "declared_exact": int(exact),
            "eligible_declared_exact": int(eligible and exact),
            "correct_exact": int(correct), "false_exact": int(false_exact),
            "downgraded": int(status == "completed" and not exact),
            "missing_or_timeout": int(status == "missing-or-timeout"),
            "panic": int(status == "panic"), "cancelled": int(status == "cancelled"),
            "invalid_boundary": int(status == "completed" and not legal_boundary),
        }
        counts.update(counters)
        kinds[truth["kind"]].update(counters)
        cases.append({"case_id": case_id, "category": truth["category"], "kind": truth["kind"],
                      "status": status, "declared_exact": exact, "correct_exact": correct,
                      "false_exact": false_exact, "expected_byte": truth["expected_byte"],
                      "observed_byte": byte, "expected_grapheme": truth["expected_grapheme"],
                      "observed_grapheme_range": observed_range,
                      "observed_text_prefix": actual_text, "legal_utf8_boundary": legal_boundary,
                      "decision": resolution, "anchor": result.get("anchor"),
                      "capture_wall_ms": result.get("capture_wall_ms"),
                      "resolve_wall_ms": result.get("resolve_wall_ms")})
    metrics = {
        "exact_hit_rate": rate(counts["correct_exact"], counts["eligible"]),
        "automatic_exact_coverage_eligible": rate(counts["eligible_declared_exact"], counts["eligible"]),
        "exact_declaration_precision": rate(counts["correct_exact"], counts["declared_exact"]),
        "false_exact_rate_all_cases": rate(counts["false_exact"], counts["total"]),
        "downgrade_rate": rate(counts["downgraded"], counts["total"]),
        "timeout_rate": rate(counts["missing_or_timeout"], counts["total"]),
        "cancellation_rate": rate(counts["cancelled"], counts["total"]),
    }
    complete = not any(counts[key] for key in ["missing_or_timeout", "panic", "invalid_boundary"])
    passed = complete and metrics["exact_hit_rate"] >= .98 and counts["false_exact"] == 0
    out = Path(args.output)
    out.mkdir(parents=True, exist_ok=False)
    with (out / "cases.jsonl").open("w", encoding="utf-8", newline="\n") as target:
        for case in cases:
            target.write(json.dumps(case, ensure_ascii=False) + "\n")
    failures = [case for case in cases if case["false_exact"] or case["status"] != "completed"
                or case["category"] == "eligible" and not case["correct_exact"]]
    (out / "failures.json").write_text(json.dumps(failures, ensure_ascii=False, indent=2), encoding="utf-8")
    report = {
        "passed": passed, "created_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "counts": dict(counts), "metrics": metrics,
        "per_kind": {kind: dict(value) for kind, value in kinds.items()},
        "capture_wall_ms": summary([c["capture_wall_ms"] for c in cases if c["capture_wall_ms"] is not None]),
        "resolve_wall_ms": summary([c["resolve_wall_ms"] for c in cases if c["resolve_wall_ms"] is not None]),
        "inputs_sha256": freeze["inputs_sha256"], "answers_sha256": freeze["answers_sha256"],
        "raw_results_sha256": sha(args.results), "adapter_binary_sha256": sha(args.binary),
        "scorer_sha256": sha(__file__), "false_exact_scope": "observed frozen corpus only; not a universal zero-error claim",
        "boundary": "pure locator correctness; GUI geometry, visibility, and persistence require separate acceptance",
    }
    (out / "summary.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    raise SystemExit(0 if passed else 1)


if __name__ == "__main__":
    main()
