# SPDX-License-Identifier: AGPL-3.0-only
"""Independent calculation for the foreground of the shared primary action."""
import json
from pathlib import Path

def lum(rgb):
    x=[c/255 for c in rgb]
    x=[v/12.92 if v<=0.04045 else ((v+.055)/1.055)**2.4 for v in x]
    return sum(a*b for a,b in zip(x,[.2126,.7152,.0722]))

colors=[[[58,86,157],[120,68,183],[183,46,110],[165,83,17],[33,120,73],[85,94,112]],[[145,177,250],[192,152,255],[255,138,189],[255,182,112],[111,205,155],[190,197,211]]]
rows=[]
for dark in [False,True]:
    fg=[24,34,49] if dark else [255,255,255]
    for name,bg in zip(['Blue','Purple','Pink','Orange','Green','Graphite'],colors[int(dark)]):
        a,b=lum(fg),lum(bg);ratio=(max(a,b)+.05)/(min(a,b)+.05)
        assert ratio>=4.5
        rows.append(dict(dark=dark,accent=name,foreground=fg,background=bg,contrast=round(ratio,3)))
out=Path(__file__).resolve().parents[1]/'evidence/ui-ux/2026-10-28/primary-button-contrast.json'
out.write_text(json.dumps(dict(method='WCAG sRGB relative luminance',threshold=4.5,rows=rows),indent=2))
print(f'{len(rows)} primary button states pass; minimum {min(x["contrast"] for x in rows)}:1')
