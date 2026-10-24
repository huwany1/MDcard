# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Independent read-only audit of the frozen eleven native package GUI jobs."""
import datetime
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ART = ROOT / "artifacts/reading-precision"
BIN = "b438bd9e042366a77c63bcb9f6ccc989414119788959086cec98114f2c6cd9cf"


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def local(path):
    assert path.startswith("/work/")
    return ROOT / path.removeprefix("/work/")


def point(raw, geometry, viewport=None):
    start, end = geometry["source_byte"], geometry["source_end"]
    raw[:start].decode("utf-8")
    text = raw[start:end].decode("utf-8")
    assert text == geometry["source_text"] == geometry["text"]
    assert geometry["grapheme_range"] == {"start": start, "end": end}
    assert raw[:start].count(b"\n") == geometry["logical_line"]
    assert geometry["origin"] == "galley-current-pass"
    if viewport:
        assert abs(geometry["rect"][1] - viewport[1] - geometry["viewport_y"]) < .001
    return start


def main():
    output = ROOT / "evidence/reading-precision/master/b-r29-linux-package-independent-audit.json"
    assert not output.exists()
    execution = ART / "package-linux-r29-execution"
    status = read(execution / "execution-status.json")
    freeze = ART / "package-linux-r29-driver-snapshots-v3"
    frozen = read(freeze / "manifest.json")
    assert len(status) == frozen["linux_commands"] == 11
    for name, info in frozen["files"].items():
        assert sha(ROOT / name) == sha(freeze / name) == info["sha256"]
    rows = []
    for done in status:
        assert done["passed"] and done["exit_code"] == 0
        assert done["driver_sha256"] == frozen["files"][done["script"]]["sha256"]
        directory = local(done["evidence"])
        result = read(directory / "result.json")
        assert sha(directory / "result.json") == done["result_sha256"]
        assert result["passed"] and result["binary_sha256"] == BIN
        package = done["env"]["MDCARD_READING_EXE"].removesuffix("/mdcard")
        apps = [r for r in done["runtime_ownership"] if r["kind"] == "actual-package-app"]
        assert apps
        for app in apps:
            assert app["executable"] == package + "/mdcard"
            assert app["renderer_ownership"]["passed"]
            assert app["renderer_ownership"]["kind"] in ["actual-package-renderer", "approved-async-wrapper"]
        nodes = [r for r in done["runtime_ownership"] if r["kind"] == "actual-owned-renderer-node"]
        assert all(n["executable"] == package + "/renderer/runtime/node" and n["within_package"] for n in nodes)
        if done["index"] == 4:
            assert nodes, "Real async renderer child must be observed"
        rows.append({"index": done["index"], "script": done["script"], "result": str(directory.relative_to(ROOT)),
                     "result_sha256": done["result_sha256"], "actual_app_samples": len(apps), "owned_node_samples": len(nodes)})

    base = ROOT / "evidence/reading-precision/implementation"
    font = base / "precision-b-package-r29-font"
    source = (ART / "performance-inputs/ordinary.md").read_bytes()
    observations = []
    for snapshot in sorted(font.glob("*.json")):
        if snapshot.stem in ["reading", "reading-state", "search", "ui", "settings", "result"] or snapshot.stem.startswith("font-"):
            continue
        data = read(snapshot)
        for name, view in data.get("views", {}).items():
            geometry = view.get("geometry")
            if not geometry or geometry["precision"] != "text":
                continue
            point(source, geometry, view["viewport_rect"])
            anchor = view["position"].get("text_anchor")
            if anchor and geometry["source_byte"] == anchor["source"]["source_byte"]:
                error = abs(geometry["viewport_y"] - anchor["viewport_y"])
                if not geometry["clamped"]:
                    assert error <= 2, (snapshot, name, error)
                observations.append({"snapshot": snapshot.name, "view": name, "source_byte": geometry["source_byte"], "error_px": error, "clamped": geometry["clamped"]})
    assert observations

    relocation = base / "precision-b-package-r29-relocation"
    oracles = []
    for case in read(relocation / "result.json")["cases"]:
        directory = relocation / case["name"]
        old, new = (directory / "old.txt").read_bytes(), (directory / "new.txt").read_bytes()
        assert hashlib.sha256(old).hexdigest() == case["old_sha256"]
        assert hashlib.sha256(new).hexdigest() == case["new_sha256"]
        decision = case["relocation"]
        if case["name"] == "md-prefix":
            delta = len(new) - len(old)
            assert new[delta:] == old
            expected = case["old_byte"] + delta
        elif case["name"] == "large-txt-lines-split-middle":
            text = old.decode("utf-8")
            target_scalar = len(old[:case["old_byte"]].decode("utf-8"))
            edits = case["edit"]["independent_scalar_edits"]
            shifted = target_scalar + sum(len(replacement) - (end - start) for start, end, replacement in edits if end <= target_scalar)
            for start, end, replacement in reversed(edits):
                text = text[:start] + replacement + text[end:]
            assert text.encode("utf-8") == new
            expected = len(text[:shifted].encode("utf-8"))
        else:
            assert not decision["exact"] and decision["tier"] != "text"
            if case["name"] == "md-ambiguous":
                assert new == old + b"\n\n" + old
            else:
                start, end = case["edit"]["deleted_range"]
                assert new == old[:start] + old[end:]
            oracles.append({"name": case["name"], "tier": decision["tier"], "reason": decision["reason"]})
            continue
        geometry = read(directory / "02-after.json")["views"]["main/preview"]["geometry"]
        point(new, geometry)
        assert geometry["source_byte"] == case["expected_byte"] == expected
        assert old[case["old_byte"]:case["old_byte"] + len(case["old_grapheme"].encode())].decode() == geometry["source_text"]
        error = abs(geometry["viewport_y"] - case["old_viewport_y"])
        assert error <= 2 and not geometry["clamped"]
        oracles.append({"name": case["name"], "independent_new_byte": expected, "error_px": error})

    navigation = []
    for kind in ["search", "outline"]:
        directory = base / ("precision-b-package-r29-" + kind + "-navigation")
        result = read(directory / "result.json")
        raw = (directory / "workspace/heavy.md").read_bytes()
        assert hashlib.sha256(raw).hexdigest() == result["changed_sha256"]
        geometry = read(directory / "02-stable-after-layout.json")["views"]["main/source"]["geometry"]
        point(raw, geometry)
        assert geometry["source_byte"] == result["expected_source_byte"]
        assert geometry["logical_line"] == result["expected_logical_line"]
        before, after = result["before_task"], result["after_task"]
        assert before["running"] and before["task_id"] == after["task_id"]
        assert after["cancel_requested_while_running"] and not after["running"]
        tail = result["tail"]
        duration = (tail[-1]["monotonic_ns"] - tail[0]["monotonic_ns"]) / 1e6
        assert duration >= 2000 and all(x["glyph"]["source_byte"] == geometry["source_byte"] for x in tail)
        navigation.append({"kind": kind, "source_byte": geometry["source_byte"], "actual_logical_line": geometry["logical_line"], "stable_ms": duration, "running_cancel": True})

    result = {"passed": True, "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "product_commit": "40b5e3bb16e1658e333cf2fddf965a22d4e878ff", "binary_sha256": BIN,
              "groups": rows, "font_original_source_and_actual_geometry": observations,
              "relocation_independent_original_edit_oracles": oracles, "navigation": navigation,
              "boundary": "Current extracted package native GUI evidence, frozen drivers, actual /proc ownership, original decoded UTF-8 and actual layout coordinates independently checked. Scalar boundaries here supplement the separate grapheme-boundary tests and Windows StringInfo checks. Actual screenshots separately viewed by master; this does not replace full payload post-GUI verification or claim cancellation performance."}
    with output.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(result, stream, indent=2)
        stream.write("\n")
    print(json.dumps({"passed": True, "groups": len(rows), "font_observations": len(observations), "relocation_oracles": len(oracles), "navigation": len(navigation)}))


if __name__ == "__main__":
    main()
