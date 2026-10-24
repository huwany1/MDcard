# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Native-GUI manuscripts and independent original-source mapping landmarks."""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent


def markdown(count, long=False):
    pieces = ["# 文字恢复与实际排版 🦀\n\n"]
    for n in range(count):
        if n % 8 == 0:
            pieces.append(f"## 操作章节 {n // 8:04d}\n\n")
        pieces.append(f"段落 {n:05d} source-{n:05d}：")
        pieces.append(("中文🙂 e\u0301 👩🏽\u200d💻 👍🏿 **粗体文字** "
                       "[可见链接](https://example.invalid/hidden-long-destination) "
                       "`inline_code` \\*转义星号\\* &amp; "
                       "reading anchors retain text identity across layout changes. ") * (75 if long else 3))
        pieces.append("\n\n")
    return "".join(pieces)


def plain(count, long=False):
    return "\n".join(
        f"纯文本 {n:05d} text-{n:05d}：" +
        ("中文🙂 e\u0301 👩🏽\u200d💻 👍🏿 literal **source** [label](url) "
         "a long logical line wraps into different visual rows. ") * (100 if long else 4)
        for n in range(count)) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    out = Path(args.output).resolve()
    assert out.is_relative_to(ROOT.resolve())
    out.mkdir(parents=True, exist_ok=True)
    manifest = []

    def write(name, text, encoding="utf-8"):
        path = out / name
        path.parent.mkdir(parents=True, exist_ok=True)
        data = text.encode(encoding)
        path.write_bytes(data)
        manifest.append({"relative": name, "path": str(path), "encoding": encoding,
                         "bytes": len(data), "decoded_utf8_bytes": len(text.encode("utf-8")),
                         "decoded_utf8_sha256": hashlib.sha256(text.encode("utf-8")).hexdigest(),
                         "sha256": hashlib.sha256(data).hexdigest()})

    write("precision.md", markdown(90))
    write("precision.txt", plain(140))
    write("secondary.md", markdown(65))
    write("nested/precision.md", markdown(55))
    write("long-paragraph.md", markdown(8, True))
    write("long-line.txt", plain(8, True))
    mega = ("中文🙂 e\u0301 👩🏽\u200d💻 👍🏿 同一逻辑长段需要按真实文字排版恢复。 "
            "A very long paragraph crosses many screens and virtual windows. ") * 12000
    write("mega-paragraph.md", "# 单段虚拟化专项\n\n" + mega + "\n")
    write("mega-line.txt", mega + "\n")
    write("large.md", markdown(1800))
    write("large.txt", plain(2400))
    write("utf16.txt", plain(100).replace("\n", "\r\n"), "utf-16")
    write("gb18030.txt", plain(100), "gb18030")
    write("empty.md", "")
    tall_image = Image.new("RGB", (240, 3200), "#e9eef5")
    painter = ImageDraw.Draw(tall_image)
    for y in range(0, 3200, 80):
        painter.rectangle((0, y, 239, y + 31), fill=(70 + y % 90, 100, 150))
    image_path = out / "tall-image.png"
    tall_image.save(image_path, format="PNG", compress_level=9)
    with Image.open(image_path) as decoded:
        decoded.load()
        assert decoded.size == (240, 3200)
    image_bytes = image_path.read_bytes()
    manifest.append({"relative": image_path.name, "path": str(image_path), "encoding": "binary PNG",
                     "bytes": len(image_bytes), "sha256": hashlib.sha256(image_bytes).hexdigest()})
    write("nontext.md", "# 非文字视口的降级\n\n![测试用高图片](tall-image.png)\n\n图片之后的独立段落。\n")

    source = "# 映射标题🙂\n\n"
    landmarks = []

    def landmark(kind, prefix, visible, suffix):
        nonlocal source
        start = len((source + prefix).encode("utf-8"))
        landmarks.append({"kind": kind, "visible": visible, "source_start": start,
                          "source_end": start + len(visible.encode("utf-8"))})
        source += prefix + visible + suffix

    for n in range(10):
        landmark("paragraph", "\n\n", f"普通段落{n} 中文🙂 e\u0301", "\n\n")
        landmark("emphasis", "**", f"强调正文{n} 👩🏽\u200d💻", "**\n\n")
        landmark("link-label", "[", f"链接标签{n} 👍🏿", "](https://example.invalid/very-long-hidden-target)\n\n")
        landmark("escaped-surrounding", "\\*", f"转义中部{n} 中文", "\\*\n\n")
        landmark("inline-code", "`", f"内联代码{n}_🙂", "`\n\n")
        landmark("list", "- ", f"列表正文{n} e\u0301", "\n\n")
        landmark("quote", "> ", f"引用正文{n} 中文🙂", "\n\n")
        landmark("table-header", "| ", f"表头{n} 中文", " | 数值 |\n| --- | --- |\n")
        landmark("table-cell", "| ", f"单元格{n} 🙂", " | 123 |\n\n")
        landmark("fenced-code", "```text\n", f"代码行{n} **原样** 🙂", "\nsecond code line\n```\n\n")
        source += "生成节点之前 $x^2$ 之后。\n\n```math\n\\frac{a}{b}+x^2\n```\n\n"
    write("mapping.md", source)
    (out / "mapping-golden.json").write_text(json.dumps({"source_sha256": hashlib.sha256(source.encode("utf-8")).hexdigest(),
        "landmarks": landmarks, "oracle": "independent literal construction of original UTF-8 ranges; no MDcard mapping calls"},
        ensure_ascii=False, indent=2), encoding="utf-8")
    large_mapping = source * 110
    write("mapping-large.md", large_mapping)
    (out / "mapping-large-golden.json").write_text(json.dumps({
        "source_sha256": hashlib.sha256(large_mapping.encode("utf-8")).hexdigest(),
        "landmarks": landmarks,
        "oracle": "same independently constructed original ranges in the first region of a large decoded manuscript; no production source map"},
        ensure_ascii=False, indent=2), encoding="utf-8")
    (out / "precision-fixtures.json").write_text(json.dumps({"files": manifest,
        "mapping_golden_sha256": hashlib.sha256((out / "mapping-golden.json").read_bytes()).hexdigest()},
        ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"output": str(out), "files": len(manifest),
                      "sizes": {entry["relative"]: entry["bytes"] for entry in manifest}}, ensure_ascii=False))


if __name__ == "__main__":
    main()
