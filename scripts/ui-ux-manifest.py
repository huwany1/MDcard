# SPDX-License-Identifier: AGPL-3.0-only
"""Freeze source identities and index real captures separately from designs."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
from datetime import datetime, timezone
from PIL import Image, ImageDraw, ImageStat

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'evidence/ui-ux/2026-10-28'
stage=sys.argv[1]
files=subprocess.check_output(['git','ls-files','src','vendor','locales'],cwd=ROOT,text=True).splitlines()
source={f:hashlib.sha256((ROOT/f).read_bytes()).hexdigest() for f in files}
captures=[]
folder=OUT/('baseline-native' if stage=='baseline' else 'final-native')
for path in sorted(folder.glob('*.png')):
    info=json.loads(path.with_suffix('.json').read_text(encoding='utf8'))
    im=Image.open(path).convert('RGB')
    if max(ImageStat.Stat(im).stddev)<1:raise RuntimeError(f'Blank capture: {path}')
    captures.append(dict(image=path.relative_to(ROOT).as_posix(),size=im.size,sha256=hashlib.sha256(path.read_bytes()).hexdigest(),captured_utc=info['captured_utc'],mode=info['ui'].get('mode'),appearance=info['ui'].get('appearance'),settings_open=info['ui'].get('settings_open'),quick_open=info['search'].get('quick_open'),search_open=info['search'].get('open'),encoding_open=info['ui'].get('encoding_open'),export_open=info['export'].get('open'),native_scale=info['ui'].get('pixels_per_point')))
for page in range((len(captures)+17)//18):
    group=captures[page*18:(page+1)*18];width=480;cellh=350
    sheet=Image.new('RGB',(width*3,cellh*((len(group)+2)//3)),'#e5e8ed');draw=ImageDraw.Draw(sheet)
    for i,item in enumerate(group):
        im=Image.open(ROOT/item['image']).convert('RGB');im.thumbnail((width-12,cellh-35))
        x=(i%3)*width+6;y=(i//3)*cellh+6;sheet.paste(im,(x,y));draw.text((x,y+cellh-30),Path(item['image']).stem,fill='#293342')
    sheet.save(OUT/f'{stage}-contact-{page+1}.png')
result=dict(stage=stage,recorded_utc=datetime.now(timezone.utc).isoformat(),base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),git_diff_stat=subprocess.check_output(['git','diff','--stat'],cwd=ROOT,text=True),source_sha256=source,captures=captures,binaries={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in (OUT/'binaries').iterdir() if p.is_file()})
(OUT/f'{stage}-manifest.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf8')
print(json.dumps(dict(stage=stage,count=len(captures),recorded_utc=result['recorded_utc'],source_diff=result['git_diff_stat']),ensure_ascii=False))
