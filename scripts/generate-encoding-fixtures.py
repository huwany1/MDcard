# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Independent reference bytes from Python's codecs, not MDcard's codecs."""
import json
from pathlib import Path

out = Path(__file__).resolve().parents[1] / "tests/fixtures/encodings"
out.mkdir(parents=True, exist_ok=True)
cases = [
    ("utf8", "utf-8", "中文 🦀", b""),
    ("utf8-bom", "utf-8", "Русский 🦀", b"\xef\xbb\xbf"),
    ("utf16le", "utf-16-le", "中文 🦀", b"\xff\xfe"),
    ("utf16be", "utf-16-be", "日本語 🦀", b"\xfe\xff"),
    ("utf16le-no-bom", "utf-16-le", "中文 🦀", b""),
    ("utf16be-no-bom", "utf-16-be", "日本語 🦀", b""),
    ("gbk", "gbk", "简体中文测试", b""),
    ("gb18030", "gb18030", "中文𠀀🦀", b""),
    ("big5", "big5", "繁體中文測試", b""),
    ("shift-jis", "shift_jis", "日本語テスト", b""),
    ("euc-kr", "euc_kr", "한국어 테스트", b""),
    ("windows1251", "cp1251", "Русский текст", b""),
    ("windows1252", "cp1252", "café € résumé", b""),
]
manifest = []
for name, codec, content, bom in cases:
    text = content + "\r\nline\nlast\r"
    (out / f"{name}.txt").write_bytes(bom + text.encode(codec))
    (out / f"{name}.edited.txt").write_bytes(bom + (text + "edit\r\n").encode(codec))
    manifest.append({"name": name, "codec": codec, "bom": bool(bom), "text": text})
(out / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(f"Generated {len(cases)} original and edited fixtures with Python codecs in {out}")
