# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Generate a regular static instance for resvg (which does not apply VF axes).
Requires fonttools==4.61.1. Original OFL copyright and license stay unchanged.
"""
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont
root=Path(__file__).resolve().parent.parent
for region,family,filename in [('SC','MDcard Diagram Sans','MDcardDiagramSans-Regular'),('KR','MDcard Diagram Sans KR','MDcardDiagramSansKR-Regular')]:
 font=instantiateVariableFont(TTFont(root/f'assets/NotoSans{region}.ttf'),{'wght':400},inplace=False)
 names={1:family,2:'Regular',3:filename,4:family+' Regular',6:filename,16:family,17:'Regular'}
 for record in font['name'].names:
  if record.nameID in names:record.string=names[record.nameID].encode(record.getEncoding(),errors='replace')
 font['OS/2'].usWeightClass=400
 font.save(root/f'assets/{filename}.ttf')
 print(root/f'assets/{filename}.ttf')
