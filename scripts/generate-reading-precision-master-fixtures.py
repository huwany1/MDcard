# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Freeze independent old/new anchor inputs and edit-provenance answers.

Does not import or call MDcard. UTF-8 answers derive from surviving original
Unicode scalars, not search candidates, offsets returned by the product, or
production mapping functions. Inputs and answers are stored separately.
"""
from __future__ import annotations

import argparse
import collections
import datetime
import hashlib
import json
import random
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SEED = 2026102301
CLUSTERS = ["中", "🙂", "👩🏽\u200d💻", "e\u0301", "🇨🇳", "账", "🦀", "👍🏿"]
TOPICS = ["恢复记录", "事务提交", "窗口排版", "离线索引", "本地备份", "资源调度",
          "缓存淘汰", "字符边界", "内容版本", "配置迁移", "文件权限", "事件顺序"]
QUALIFIERS = ["清晨", "雨后", "晚间", "北侧", "南侧", "旧版", "新版", "首次",
              "再次", "平稳", "持续", "短暂", "分组", "独立", "临时", "最终"]
KINDS = ["major-prefix-insert", "major-prefix-delete", "major-prefix-replace",
         "chapter-rotation", "title-rename-and-reorder", "paragraph-split",
         "paragraph-merge", "local-rewrite-left", "local-rewrite-right",
         "duplicate-context-distinct-section"]


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def dumps(value) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


class Edit:
    """Each old scalar has an immutable provenance index. Inserts have none."""

    def __init__(self, text):
        self.old = text
        self.items = [(ch, at) for at, ch in enumerate(text)]
        self.inserted = 0
        self.deleted = 0
        self.moved_blocks = 0
        self.block_count = 10
        self.operations = []

    @property
    def text(self):
        return "".join(ch for ch, _ in self.items)

    def replace(self, start, end, value):
        self.deleted += end - start
        self.inserted += len(value)
        self.operations.append({"kind": "splice", "start_scalar": start,
                                "deleted_scalars": end - start, "inserted_scalars": len(value)})
        self.items[start:end] = [(ch, None) for ch in value]

    def original_index(self, origin):
        matches = [at for at, (_, previous) in enumerate(self.items) if previous == origin]
        if len(matches) != 1:
            raise AssertionError("Expected one surviving original scalar")
        return matches[0]

    def reorder(self, ranges, order):
        chunks = [self.items[a:b] for a, b in ranges]
        self.items = [item for index in order for item in chunks[index]]
        self.moved_blocks = sum(a != b for a, b in enumerate(order))
        self.block_count = len(order)
        self.operations.append({"kind": "block-permutation", "new_order": order})


def document(serial, kind):
    topic = TOPICS[serial % len(TOPICS)]
    qualifier = QUALIFIERS[(serial // len(TOPICS)) % len(QUALIFIERS)]
    cluster = CLUSTERS[serial % len(CLUSTERS)]
    sections = []
    anchor_origin = None
    for section in range(10):
        heading = f"## {qualifier}的{TOPICS[(serial + section) % len(TOPICS)]}·步骤{section + 1}\n\n"
        unique = f"在第{serial + 17}次检修中，{qualifier}的{topic}由第{section + 1}组复核。"
        paragraph = (unique + "应先比较原始内容与新内容的边界，再检查恢复任务是否仍属于当前窗口。"
                     "The operator records context before applying a revision. "
                     "源码位置和阅读位置分别保存，不能让文首光标覆盖后部的阅读状态。"
                     f"记录事项：{cluster}恢复目标仍需依据实际文字与相邻证据确认，"
                     "重复内容需要保守判断；遇到不可靠候选应给出可查看的原因。"
                     "最后核对事务、版本和权限，避免迟到结果影响新的文档。\n\n")
        if kind == "duplicate-context-distinct-section":
            paragraph = ("以下是各章节共同采用的恢复操作，先确认文档身份，然后保存阅读锚点。"
                         "对于重复正文，应使用章节证据而不是首次出现的位置。" * 3
                         + f"记录事项：{cluster}恢复目标仍需依据实际文字与相邻证据确认，"
                         + "重复内容不能依靠距离最近来判定。" * 8 + "\n\n")
        value = heading + paragraph
        if section == 6:
            anchor_origin = sum(map(len, sections)) + value.index(f"{cluster}恢复目标")
        sections.append(value)
    source = "".join(sections)
    assert source[anchor_origin:anchor_origin + len(cluster)] == cluster
    offsets = [0]
    for value in sections:
        offsets.append(offsets[-1] + len(value))
    return source, anchor_origin, cluster, offsets


def positive(serial, kind):
    source, origin, cluster, boundaries = document(serial, kind)
    edit = Edit(source)
    if kind == "major-prefix-insert":
        prefix = ("新增前言：审阅者先检查证据，再比较不同版本的内容。"
                  "New introductory material changes all subsequent offsets.\n\n")
        prefix = prefix * (len(source) // len(prefix) // 2 + 1)
        edit.replace(0, 0, prefix)
    elif kind == "major-prefix-delete":
        edit.replace(0, boundaries[4], "")
    elif kind == "major-prefix-replace":
        replacement = "替换的操作说明：先检查权限与版本，再建立新的任务。\n\n" * 30
        edit.replace(0, boundaries[4], replacement)
    elif kind in {"chapter-rotation", "duplicate-context-distinct-section"}:
        edit.reorder(list(zip(boundaries, boundaries[1:])), list(range(7, 10)) + list(range(7)))
    elif kind == "title-rename-and-reorder":
        edit.reorder(list(zip(boundaries, boundaries[1:])), [9, 7, 3, 1, 6, 0, 4, 8, 2, 5])
        text = edit.text
        heading_ranges = []
        at = 0
        for line in text.splitlines(keepends=True):
            if line.startswith("## "):
                heading_ranges.append((at, at + len(line), line.replace("步骤", "重整阶段")))
            at += len(line)
        for start, end, value in reversed(heading_ranges):
            edit.replace(start, end, value)
    elif kind == "paragraph-split":
        edit.replace(origin + len(cluster) + 18, origin + len(cluster) + 18, "\n\n")
        edit.replace(origin - 18, origin - 18, "\n\n")
    elif kind == "paragraph-merge":
        # Merge adjacent paragraphs/sections while preserving the original target.
        right = boundaries[7]
        left = boundaries[6]
        edit.replace(right - 2, right, " ")
        edit.replace(left, source.index("\n\n", left) + 2, "")
        edit.replace(left - 2, left, " ")
    elif kind == "local-rewrite-left":
        edit.replace(origin - 70, origin - 5,
                     "重写的核对说明要求把任务所属窗口、正文版本与用户意图分别记录。")
    elif kind == "local-rewrite-right":
        begin = origin + len(cluster) + 7
        edit.replace(begin, begin + 75,
                     "此处改写了验证过程，之后仍需按上下文和章节结构重新确认同一个文字锚点。")
    else:
        raise AssertionError(kind)
    new_index = edit.original_index(origin)
    for shift in range(len(cluster)):
        assert edit.items[new_index + shift][1] == origin + shift
    assert edit.text[new_index:new_index + len(cluster)] == cluster
    return source, origin, cluster, edit, new_index


def ambiguous(serial):
    cluster = CLUSTERS[serial % len(CLUSTERS)]
    section = (f"## 共用说明\n\n" + "重复章节中的上下文、标题及邻近内容均一致。" * 10
               + f"{cluster}重复阅读位置" + "没有独立证据表明哪一份复制正文对应原目标。" * 10 + "\n\n")
    source = section * 5
    origin = len(section) * (1 + serial % 3) + section.index(f"{cluster}重复")
    edit = Edit(source)
    # Copies carry no observable identity: all source evidence remains symmetric.
    edit.replace(0, 0, section * (1 + serial % 3))
    return source, origin, cluster, edit


def deleted(serial):
    source, origin, cluster, boundaries = document(serial + 2000, "deleted")
    edit = Edit(source)
    kind = ["delete-target-section", "replace-target-paragraph", "delete-all", "unrelated-short"][serial % 4]
    if kind == "delete-target-section":
        edit.replace(boundaries[6], boundaries[7], "")
    elif kind == "replace-target-paragraph":
        edit.replace(boundaries[6], boundaries[7],
                     "## 更新后的无关记录\n\n本节不再保留旧阅读目标，任务应执行保守降级。\n\n")
    elif kind == "delete-all":
        edit.replace(0, len(source), "")
    else:
        edit.replace(0, len(source), "# 全新文档\n\n仅有常见的操作说明，不再包含旧阅读锚点。\n")
    assert all(previous != origin for _, previous in edit.items)
    return source, origin, cluster, edit, kind


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    output = Path(args.output).resolve()
    if not output.is_relative_to(ROOT.resolve()):
        raise ValueError("Independent fixtures must stay inside the workspace")
    output.mkdir(parents=True, exist_ok=True)
    freeze = output / "freeze.json"
    if freeze.exists():
        raise FileExistsError("Use a fresh output directory; frozen evidence is immutable")
    cases = []
    for i in range(1000):
        kind = KINDS[i % len(KINDS)]
        source, origin, cluster, edit, new_index = positive(i, kind)
        cases.append((f"positive-{i:04}", "eligible", kind, source, origin, cluster, edit, new_index))
    for i in range(200):
        source, origin, cluster, edit = ambiguous(i)
        cases.append((f"ambiguous-{i:04}", "ambiguous", "symmetric-duplicate", source, origin, cluster, edit, None))
    for i in range(200):
        source, origin, cluster, edit, kind = deleted(i)
        cases.append((f"deleted-{i:04}", "deleted", kind, source, origin, cluster, edit, None))
    random.Random(SEED).shuffle(cases)
    inputs = output / "inputs.jsonl"
    answers = output / "answers.jsonl"
    categories = collections.Counter()
    kinds = collections.Counter()
    major_characters = 0
    major_blocks = 0
    with inputs.open("w", encoding="utf-8", newline="\n") as data, answers.open("w", encoding="utf-8", newline="\n") as truth:
        for case_id, category, kind, source, origin, cluster, edit, new_index in cases:
            old_byte = len(source[:origin].encode("utf-8"))
            data.write(dumps({"case_id": case_id, "old": source, "new": edit.text,
                              "old_byte": old_byte,
                              "document_name": f"document-{case_id}.{'txt' if int(case_id[-4:]) % 3 == 0 else 'md'}"}) + "\n")
            edit_ratio = (edit.inserted + edit.deleted) / max(1, len(source))
            moved_ratio = edit.moved_blocks / max(1, edit.block_count)
            truth.write(dumps({"case_id": case_id, "category": category, "kind": kind,
                               "expected_byte": None if new_index is None else len(edit.text[:new_index].encode("utf-8")),
                               "expected_grapheme": cluster, "old_scalar": origin,
                               "old_scalar_count": len(source), "new_scalar_count": len(edit.text),
                               "inserted_scalars": edit.inserted, "deleted_scalars": edit.deleted,
                               "character_edit_ratio": edit_ratio, "block_move_ratio": moved_ratio,
                               "operations": edit.operations}) + "\n")
            categories[category] += 1
            kinds[kind] += 1
            major_characters += edit_ratio >= 0.3
            major_blocks += moved_ratio >= 0.3
    manifest = {
        "created_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "seed": SEED, "generator_sha256": digest(Path(__file__)),
        "inputs_sha256": digest(inputs), "answers_sha256": digest(answers),
        "cases": len(cases), "distinct_document_pairs": len(cases),
        "categories": dict(categories), "kinds": dict(kinds),
        "major_character_edit_cases": major_characters,
        "major_block_move_cases": major_blocks,
        "oracle": "surviving old Unicode scalar provenance under explicit independent edit operations; no MDcard imports",
        "thresholds": {"eligible_exact_hit_rate_minimum": 0.98, "observed_false_exact_maximum": 0},
        "coordinates": "UTF-8 bytes in decoded Unicode; chosen graphemes have explicit independent boundaries",
        "inputs": inputs.name, "answers": answers.name,
    }
    freeze.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(manifest, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
