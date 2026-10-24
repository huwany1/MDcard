# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Independent, deterministic native-GUI reading-state acceptance fixtures.

Golden source positions use Python's UTF-8 encoding, not MDcard's mapping code.
No fixture contains user manuscripts; output must be under this checkout.
"""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument("--output", required=True)
args = parser.parse_args()
out = Path(args.output).resolve()
if not out.is_relative_to(ROOT):
    raise ValueError("Fixture output must be inside the checkout")
out.mkdir(parents=True, exist_ok=True)
files = []


def write(relative, text, encoding="utf-8"):
    path = out / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    data = text.encode(encoding)
    path.write_bytes(data)
    decoded = data.decode(encoding)
    markers = []
    for line in decoded.splitlines():
        if line.startswith("BLOCK-"):
            marker = line.split(" ", 1)[0]
            char = decoded.index(marker)
            markers.append({"text": marker, "char": char,
                            "byte": len(decoded[:char].encode("utf-8"))})
    files.append({"path": str(path), "relative": relative, "encoding": encoding,
                  "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data),
                  "decoded_utf8_bytes": len(decoded.encode("utf-8")), "markers": markers})


def markdown(prefix, count, long=False):
    text = [f"# {prefix} 阅读进度 🧭\n\n"]
    for n in range(count):
        if n % 12 == 0:
            text.append(f"## {prefix} Chapter {n // 12:03d} 中文 🦀\n\n")
        text.append(f"BLOCK-{prefix}-{n:04d} ")
        text.append(("独立段落 中文🙂e\u0301 **bold** [link](#target) `code` &amp; "
                     "reading position source boundaries. ") * (14 if long else 3))
        text.append("\n\n")
    return "".join(text)


write("a.md", markdown("A", 80))
write("b.md", markdown("B", 65))
write("nested/same.md", markdown("SAME-NESTED", 40))
write("same.md", markdown("SAME-ROOT", 45))
write("large.md", markdown("LARGE", 1300))
write("long-paragraph.md", markdown("TALL", 30, True))
plain = "# 原样文本 **不是粗体** 中文 🦀\n\n" + "\n".join(
    f"BLOCK-PLAIN-{n:04d} 中文🙂 plain <literal> " + "long plain text " * 7
    for n in range(320))
write("plain.txt", plain)
write("large-plain.txt", plain + "\n" + "\n".join(
    f"BLOCK-LARGE-PLAIN-{n:04d} 中文🙂 " + "virtual plain text " * 10
    for n in range(1800)))
write("utf16.txt", plain, "utf-16")
write("gb18030.txt", plain, "gb18030")
write("empty.md", "")
write("mapping.md", "# 转换坐标 🦀\n\nBefore $x^2$ after.\n\n"
      + "```math\n\\frac{a}{b} + x^2\n```\n\n"
      + "```mermaid\ngraph LR\n A[开始] --> B[结束]\n```\n\n"
      + markdown("MAP", 65))
write("mapping-large.md", "# 转换大文档 🦀\n\nBefore $x^2$ after.\n\n"
      + "```math\n\\frac{a}{b} + x^2\n```\n\n" + markdown("MAP-LARGE", 1300))
write("duplicates.md", "# Repeated paragraphs\n\n## Left\n\n重复内容🙂\n\n"
      + "## Right\n\n重复内容🙂\n\n" + markdown("DUP", 20))
(out / "fixtures.json").write_text(json.dumps({"generator": str(Path(__file__).resolve()),
    "coordinate_boundary": "UTF-8 bytes of decoded Unicode text; source encoding bytes are separate",
    "files": files}, ensure_ascii=False, indent=2), encoding="utf-8")
print(json.dumps({"files": len(files), "output": str(out),
                  "sizes": {f["relative"]: f["bytes"] for f in files}}, ensure_ascii=False))
