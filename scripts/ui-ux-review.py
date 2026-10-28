# SPDX-License-Identifier: AGPL-3.0-only
"""Local baseline/design/runtime review with relative links and no hosting."""
from pathlib import Path
import html
import json
import os
from PIL import Image,ImageDraw

ROOT=Path(__file__).resolve().parents[1];DOC=ROOT/'docs/ui-ux-2026-10-28';OUT=ROOT/'evidence/ui-ux/2026-10-28'
rows=[
('Main / split','01-split','01-main-split','01-split'),
('Writing','02-write','02-write','02-write'),
('Reading','03-read','03-read','03-read'),
('Preferences','14-preferences-alone','08-preferences','05-preferences-top'),
('Preferences / bottom','06-preferences-bottom','09-preferences-bottom','06-preferences-bottom'),
('Narrow preferences','20-narrow-preferences','10-preferences-narrow','20-narrow-preferences'),
('Narrow workspace','19-narrow-disabled-split','05-narrow','19-narrow-read'),
('Dark workspace','24-dark-split','04-dark','24-dark-split'),
('Export','16-export','11-export','16-export'),
('Search results','11-find-results','12-search','11-find-results'),
('Quick switcher','08-quick-results','13-quick','08-quick-results'),
('Structure','17-structure-valid','14-structure','09-structure'),
('Encoding','13-encoding','15-encoding','13-encoding'),
('Unsaved confirmation','23-unsaved-dialog','16-unsaved','23-unsaved-dialog'),
('Rename','15-rename','18-rename','15-rename'),
('Recovery','31-recovery-dialog','19-recovery','31-recovery-dialog'),
('Second document','32-secondary-pane','20-secondary','32-secondary-pane'),
('Replacement history','34-replacement-history','21-history','34-replacement-history'),
]

def paths(row):return [OUT/'baseline-native'/(row[1]+'.png'),DOC/'design'/(row[2]+'.png'),OUT/'final-native'/(row[3]+'.png')]
def relative(path):return os.path.relpath(path,DOC).replace('\\','/')

markup=['<!doctype html><meta charset="utf-8"><title>MDcard UI review</title><style>body{font:14px/1.6 system-ui,sans-serif;margin:24px;color:#293342;background:#f1f4f8}h1{font-size:24px}section{margin:30px 0}.grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:12px}figure{margin:0;background:white;padding:10px;border:1px solid #d6dde7;border-radius:5px}img{width:100%;height:auto;display:block}figcaption{font-weight:600;margin:0 0 8px}.note{font-size:12px;color:#596678}a{color:#3a569d}@media(max-width:900px){.grid{grid-template-columns:1fr}}</style><h1>MDcard：真实基线 → 静态设计预期 → 最终实机</h1><p>主对比为同源Linux原生应用，专属Xvfb :141、100%、1240×820；窄窗680×480。设计图增加32px标注栏，不属于应用。点击图片查看原尺寸。所有链接相对本文件，可在Windows本地打开。</p><p class="note">设置下半区和窄窗对比保留真实滚动位置；窄窗设计展示写作场景，原始与最终窄窗主图展示阅读场景，结构适配规则相同。Windows 150%真实截图及受控X11 125/150%缩放单列，不混为同一平台证据。</p>']
manifest=[]
for row in rows:
    images=paths(row)
    if not all(p.exists() for p in images):raise RuntimeError('Missing comparison '+str(images))
    markup.append('<section><h2>'+html.escape(row[0])+'</h2><div class="grid">')
    for label,path in zip(['原始 · 实际运行','静态设计 · 预期','最终 · 实际运行'],images):
        markup.append('<figure><figcaption>'+label+'</figcaption><a href="'+relative(path)+'"><img src="'+relative(path)+'" loading="lazy"></a><div class="note">'+html.escape(path.name)+'</div></figure>')
    markup.append('</div></section>')
    manifest.append(dict(scenario=row[0],baseline=relative(images[0]),design=relative(images[1]),final=relative(images[2])))
markup.append('<section><h2>Windows 11 · 左屏 · 150% · CPU/WGPU</h2><p>首次原生主窗即位于左屏，创建前设置position/active=false，启动前后前台ID一致。这里只作被动原生视觉核对，交互在专属Xvfb完成。</p><img style="max-width:1240px" src="'+relative(OUT/'final-windows/01-left-screen-split.png')+'"></section>')
(DOC/'review.html').write_text(''.join(markup),encoding='utf8')
(OUT/'comparison-index.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf8')
for page in range((len(rows)+3)//4):
    group=rows[page*4:(page+1)*4];sheet=Image.new('RGB',(1500,380*len(group)+36),'#e5eaf2');draw=ImageDraw.Draw(sheet)
    for col,label in enumerate(['BASELINE / ACTUAL','STATIC DESIGN / EXPECTED','FINAL / ACTUAL']):draw.text((col*500+10,10),label,fill='#293342')
    for i,row in enumerate(group):
        for col,path in enumerate(paths(row)):
            im=Image.open(path).convert('RGB');im.thumbnail((484,330));x=col*500+8;y=i*380+38
            sheet.paste(im,(x,y));draw.text((x,y+336),row[0]+' | '+path.stem,fill='#293342')
    sheet.save(OUT/f'comparison-{page+1}.png')
print(f'{len(rows)} triple comparisons; {DOC/"review.html"}')
