# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Keep resvg alpha exactly; replace unassociated RGB with each intended ink."""
from pathlib import Path
from PIL import Image

root = Path(__file__).resolve().parents[1]
colors = {'color': (45,91,202), 'black': (0,0,0), 'white': (255,255,255)}
for variant, color in colors.items():
    for path in sorted((root/'png'/variant).glob('mdcard-*.png')):
        with Image.open(path) as source:
            alpha = source.convert('RGBA').getchannel('A')
            output = Image.new('RGBA', source.size, (*color,255))
            output.putalpha(alpha)
            output.save(path)
print('Normalized straight RGB in 27 PNGs; resvg alpha preserved byte for byte.')
