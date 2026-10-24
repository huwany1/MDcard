# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Create regular UI instances from the bundled OFL variable fonts.

The original Google Fonts files and license notices are retained. SC/KR reuse
the existing regular diagram instances. egui's rasterizer uses the variable
font's default axis (100), so passing the original VF silently draws Thin.
Requires fonttools. No network resources are used.
"""
import hashlib
import json
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

root = Path(__file__).resolve().parent.parent
catalog = root / "assets/font-sources.json"
entries = json.loads(catalog.read_text("utf8"))
for region in ("JP", "TC"):
    source = f"assets/NotoSans{region}.ttf"
    filename = f"MDcardUISans{region}-Regular"
    destination = root / f"assets/{filename}.ttf"
    family = f"MDcard UI Sans {region}"
    font = instantiateVariableFont(TTFont(root / source), {"wght": 400}, inplace=False)
    names = {1: family, 2: "Regular", 3: filename, 4: family + " Regular",
             6: filename, 16: family, 17: "Regular"}
    for record in font["name"].names:
        if record.nameID in names:
            record.string = names[record.nameID].encode(record.getEncoding(), errors="replace")
    font["OS/2"].usWeightClass = 400
    font.save(destination)
    item = dict(path=destination.relative_to(root).as_posix(), source=source,
                generator="scripts/generate-ui-fonts.py; fonttools; wght=400; OFL family renamed",
                sha256=hashlib.sha256(destination.read_bytes()).hexdigest(),
                bytes=destination.stat().st_size)
    entries = [entry for entry in entries if entry["path"] != item["path"]] + [item]
    print(item, flush=True)
catalog.write_text(json.dumps(entries, indent=2) + "\n", "utf8", newline="\n")
