# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Independent edit-provenance oracle for native GUI relocation cases.

Transforms real test manuscripts, never MDcard state. A surviving scalar keeps
its original identity through explicit splices and permutations. No production
matcher, find-first search, or product-reported new offset is used as an answer.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location(
    "independent_edit", Path(__file__).with_name("generate-reading-precision-master-fixtures.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
Edit = module.Edit
KINDS = ("prefix-insert", "prefix-delete", "move-rename", "split-merge",
         "rewrite-left", "rewrite-right", "duplicate-all", "delete-section", "unrelated", "empty", "undo-prefix")


def transform(source: str, byte: int, end_byte: int, kind: str, prefix_to_undo: str | None = None):
    original = source.encode("utf-8")
    prefix = original[:byte].decode("utf-8", errors="strict")
    cluster = original[byte:end_byte].decode("utf-8", errors="strict")
    assert cluster and 0 <= byte < end_byte <= len(original)
    origin = len(prefix)
    finish = origin + len(cluster)
    edit = Edit(source)
    category = "eligible"
    sections = [m.start() for m in re.finditer(r"(?m)^## ", source)]
    if not sections:
        # TXT has no Markdown headings: logical lines provide independent
        # movable/deletable source scopes without inventing preview offsets.
        sections = [m.start() for m in re.finditer(r"(?m)^", source)
                    if m.start() < len(source)]
    starts = sorted(set([0] + sections))
    ranges = list(zip(starts, starts[1:] + [len(source)]))
    section = next((a, b) for a, b in ranges if a <= origin < b)
    if kind == "undo-prefix":
        assert prefix_to_undo and source.startswith(prefix_to_undo)
        assert origin >= len(prefix_to_undo), "The genuine old reading target must survive the explicit undo"
        edit.replace(0, len(prefix_to_undo), "")
    elif kind == "prefix-insert":
        paragraph = "新增审核前言：先检查文档身份、原始文字与版本，再核对当前窗口的布局。\n\n"
        added = paragraph * (len(source) // len(paragraph) // 2 + 1)
        edit.replace(0, 0, added)
    elif kind == "prefix-delete":
        cutoffs = [a for a in starts if a < origin]
        # Keep two prior source scopes so this precision case has enough
        # scroll range. Boundary clamping has a separate acceptance matrix.
        cutoff = cutoffs[max(0, len(cutoffs) - 3)]
        assert cutoff / len(source) >= .30, "Select a later actual reading anchor for major prefix deletion"
        edit.replace(0, cutoff, "")
    elif kind == "move-rename":
        order = list(range(len(ranges)))
        split = max(1, len(order) // 2)
        edit.reorder(ranges, order[split:] + order[:split])
        headings = list(re.finditer(r"(?m)^#{1,2} [^\n]+", edit.text))
        for match in reversed(headings):
            edit.replace(match.start(), match.end(), "### 重排后的独立审核章节")
    elif kind == "split-merge":
        # Both sides change paragraph structure, while the actual old target
        # and nearby evidence retain independently tracked provenance.
        edit.replace(min(len(source), finish + 24), min(len(source), finish + 24), "\n\n")
        edit.replace(max(section[0], origin - 24), max(section[0], origin - 24), "\n\n")
        previous_break = edit.text.rfind("\n\n", 0, max(0, origin - 50))
        if previous_break >= 0:
            edit.replace(previous_break, previous_break + 2, " ")
    elif kind == "rewrite-left":
        assert origin > 90
        edit.replace(origin - 80, origin - 6, "这里改写了阅读核对流程，需按上下文和结构确认对应文字。")
    elif kind == "rewrite-right":
        assert finish + 90 < len(source)
        edit.replace(finish + 7, finish + 82, "此处更新了异步任务与缓存说明，但保存的阅读文字仍然存在。")
    elif kind == "duplicate-all":
        # All observable source evidence occurs three times; provenance alone
        # cannot make a candidate identifiable to the application.
        edit.replace(0, 0, source)
        edit.replace(len(edit.items), len(edit.items), source)
        category = "ambiguous"
    elif kind == "delete-section":
        edit.replace(section[0], section[1], "")
        category = "deleted"
    elif kind == "unrelated":
        edit.replace(0, len(source), "# 新的设备日志\n\n今天只记录天气和温度。\n")
        category = "deleted"
    elif kind == "empty":
        edit.replace(0, len(source), "")
        category = "deleted"
    else:
        raise ValueError(kind)
    answer = {"category": category, "kind": kind, "old_source_byte": byte,
              "old_source_end": end_byte, "original_grapheme": cluster,
              "old_sha256": hashlib.sha256(original).hexdigest(),
              "new_sha256": hashlib.sha256(edit.text.encode("utf-8")).hexdigest(),
              "character_edit_ratio": (edit.inserted + edit.deleted) / max(1, len(source)),
              "moved_block_ratio": edit.moved_blocks / max(1, edit.block_count),
              "operations": edit.operations,
              "oracle": "original scalar provenance under explicit independent GUI-manuscript transforms"}
    if category == "eligible":
        target = edit.original_index(origin)
        assert edit.text[target:target + len(cluster)] == cluster
        assert all(edit.items[target + offset][1] == origin + offset for offset in range(len(cluster)))
        answer["expected_source_byte"] = len(edit.text[:target].encode("utf-8"))
        answer["expected_source_end"] = answer["expected_source_byte"] + len(cluster.encode("utf-8"))
    else:
        answer["expected_source_byte"] = None
        answer["expected_source_end"] = None
    return edit.text, answer


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True)
    parser.add_argument("--byte", type=int, required=True)
    parser.add_argument("--end-byte", type=int, required=True)
    parser.add_argument("--kind", choices=KINDS, required=True)
    parser.add_argument("--answer", required=True)
    parser.add_argument("--output", help="Write transformed text separately; default edits only the owned test manuscript")
    parser.add_argument("--prefix-file", help="Independent native-paste text for an undo-prefix case")
    args = parser.parse_args()
    source = Path(args.source).resolve()
    destination = Path(args.output).resolve() if args.output else source
    answer_path = Path(args.answer).resolve()
    for path in (source, destination):
        assert path.is_relative_to((ROOT / "artifacts/reading-precision/master").resolve()), path
    assert any(answer_path.is_relative_to((ROOT / relative).resolve()) for relative in
               ("artifacts/reading-precision/master", "evidence/reading-precision/master")), answer_path
    assert not answer_path.exists(), "Use fresh oracle evidence"
    old = source.read_bytes().decode("utf-8")
    prefix_to_undo = None
    if args.prefix_file:
        prefix_path = Path(args.prefix_file).resolve()
        assert prefix_path.is_relative_to((ROOT / "artifacts/reading-precision/master").resolve())
        prefix_to_undo = prefix_path.read_bytes().decode("utf-8")
    new, answer = transform(old, args.byte, args.end_byte, args.kind, prefix_to_undo)
    destination.write_bytes(new.encode("utf-8"))
    answer_path.parent.mkdir(parents=True, exist_ok=True)
    answer_path.write_text(json.dumps(answer, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(answer, ensure_ascii=True))


if __name__ == "__main__":
    main()
