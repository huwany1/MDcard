# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Render review plates from the real final PNG files; no resampling of icons."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'preview'
FONT_PATH = Path('C:/Windows/Fonts/segoeui.ttf')
FONT_BOLD = Path('C:/Windows/Fonts/seguisb.ttf')
def font(size=16, bold=False):
    return ImageFont.truetype(str(FONT_BOLD if bold else FONT_PATH), size)
def label(draw, xy, text, size=16, fill='#34415A', bold=False):
    draw.text(xy, text, font=font(size, bold), fill=fill)
def put(canvas, variant, size, xy):
    image = Image.open(ROOT / 'png' / variant / f'mdcard-{size}.png').convert('RGBA')
    canvas.paste(image, xy, image)
def wallpaper(width, height):
    im = Image.new('RGB', (width, height), '#CDD7DF')
    d = ImageDraw.Draw(im)
    d.polygon([(0, height), (0, height*0.4), (width*0.48, 0), (width*0.76, 0), (width*0.3, height)], fill='#E3CBAF')
    d.ellipse((width*0.38, -height*0.6, width*1.15, height*0.9), fill='#718B83')
    d.polygon([(width*0.56,height),(width*0.8,height*0.25),(width,height*0.5),(width,height)], fill='#B28575')
    return im

hero = Image.new('RGB', (1200, 720), '#F4F3EF')
d = ImageDraw.Draw(hero)
label(d, (58,45), 'MDcard', 35, '#1C2230', True)
label(d, (60,95), 'A quiet place for words.', 20, '#707787')
d.rounded_rectangle((576,36,1164,684), 28, fill='#1C1E22')
put(hero,'color',256,(161,189))
put(hero,'white',256,(742,189))
label(d,(205,481),'PRIMARY / BLUE',14,'#566072',True)
label(d,(797,481),'MONO / WHITE',14,'#BBC0CA',True)
label(d,(60,614),'Rounded card. Transparent M. One solid color.',17,'#566072')
label(d,(60,650),'256px icons shown at their native pixel dimensions.',14,'#7C8492')
hero.save(OUT/'hero.png')

sheet=Image.new('RGB',(1280,1040),'#F0F1F3')
d=ImageDraw.Draw(sheet)
label(d,(32,22),'MDcard / Native-size proof',26,'#1C2230',True)
label(d,(32,62),'Each icon below is pasted at 1 source pixel = 1 plate pixel. Open at 100% for the size check.',16)
sizes=[16,24,32,48,64,256]
xs=[70,135,215,315,445,665]
for row,(name,bg,mono,ink) in enumerate([('LIGHT', '#FDFCFA','black','#273244'),('DARK','#1C1E22','white','#E0E4EB'),('COMPLEX DESKTOP BACKGROUND','#CDD7DF','black','#273244')]):
    top=105+row*300
    if row==2: sheet.paste(wallpaper(1216,280),(32,top))
    else: d.rounded_rectangle((32,top,1248,top+280),16,fill=bg)
    label(d,(48,top+13),f'{name} / primary + contextual monochrome',14,ink,True)
    for size,x in zip(sizes,xs):
        if size==256:
            put(sheet,'color',size,(x,top+16))
            put(sheet,mono,size,(x+270,top+16))
            label(d,(x,top+260),'256px / color',12,ink)
            label(d,(x+270,top+260),f'256px / {mono}',12,ink)
        else:
            y=top+75+(64-size)//2
            put(sheet,'color',size,(x,y)); put(sheet,mono,size,(x,top+170+(64-size)//2))
            label(d,(x-2,top+45),f'{size}px',13,ink)
label(d,(32,1015),'This is an asset review plate, not a screenshot of an installed application.',13,'#6B7484')
sheet.save(OUT/'native-sizes.png')

# The alpha plate is deliberately magnified and clearly marked as diagnostic only.
diag=Image.new('RGB',(760,330),'#FFFFFF'); dd=ImageDraw.Draw(diag)
label(dd,(20,15),'16px optical icon / 12x nearest-neighbor diagnostic',20,'#1C2230',True)
label(dd,(20,49),'Enlarged diagnostic only. The native 16px proof is on native-sizes.png.',14)
for index,variant in enumerate(['color','black','white']):
    x=40+index*240; y=100
    for cy in range(0,192,12):
        for cx in range(0,192,12):
            dd.rectangle((x+cx,y+cy,x+cx+11,y+cy+11),fill='#D7DCE4' if (cx//12+cy//12)%2 else '#F3F5F8')
    image=Image.open(ROOT/'png'/variant/'mdcard-16.png').convert('RGBA').resize((192,192),Image.Resampling.NEAREST)
    diag.paste(image,(x,y),image)
    label(dd,(x,302),variant.upper(),13,'#536078',True)
diag.save(OUT/'optical-16px-enlarged.png')

def actual_row(variant):
    cells = []
    for size in sizes:
        cells.append(f'<figure><img class="actual" src="../png/{variant}/mdcard-{size}.png" width="{size}" height="{size}" data-actual-size="{size}" alt="{variant} {size}px"><figcaption>{size}px</figcaption></figure>')
    return '<div class="actual-row">' + ''.join(cells) + '</div>'
sections = []
for name, cls, mono in [('浅色背景','light','black'),('深色背景','dark','white'),('复杂桌面背景（图形模拟）','complex','black')]:
    sections.append(f'<section class="panel {cls}"><h2>{name}</h2><p>彩色主版本</p>{actual_row("color")}<p>适配背景的单色版本：{mono}</p>{actual_row(mono)}</section>')
html = '''<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>MDcard 图标 / 实际尺寸验收</title>
<style>
*{box-sizing:border-box}body{margin:0;background:#EEF0F3;color:#222B3A;font:15px/1.65 system-ui,"Segoe UI",sans-serif}main{max-width:1240px;margin:36px auto;padding:0 24px}h1{font-size:32px;letter-spacing:-.03em;margin-bottom:6px}h2{font-size:20px;margin:0 0 10px}p{margin:8px 0 16px}a{color:#2D5BCA}.hero{display:block;width:100%;height:auto;margin:28px 0;border-radius:20px}.note{padding:18px 22px;background:#FFF;border-left:4px solid #2D5BCA;border-radius:6px}.panel{margin:24px 0;padding:24px;border-radius:18px;overflow-x:auto}.light{background:#FDFCFA;color:#282B31}.dark{background:#1C1E22;color:#E9EAEE}.complex{background-color:#CDD7DF;background-image:linear-gradient(130deg,transparent 0 42%,#718B83 42% 67%,transparent 67%),linear-gradient(35deg,#E3CBAF 0 35%,transparent 35% 78%,#B28575 78%);color:#182235}.actual-row{display:grid;grid-template-columns:72px 80px 96px 112px 144px 288px;align-items:center;gap:16px;min-width:980px;margin:14px 0 26px}figure{margin:0;min-height:96px;display:flex;align-items:center;justify-content:center;flex-direction:column;gap:12px}img.actual{display:block;max-width:none;flex:none;image-rendering:auto}figcaption{font-size:12px}.support{display:flex;gap:24px;flex-wrap:wrap}.support img{height:auto;max-width:100%}.links{display:flex;gap:20px;flex-wrap:wrap}footer{margin:36px 0;color:#596576}
</style></head><body><main>
<h1>MDcard · 一张卡片，一个 M</h1><p>平面几何、真实负空间、单一强调蓝。图标源文件与多尺寸资源验收页。</p>
<img class="hero" src="hero.png" width="1200" height="720" alt="主版本与单色版本展示；本展示图会随页面宽度缩放">
<div class="note"><strong>实际尺寸检查规则</strong><br>下方各个图标的图片像素与 CSS 尺寸逐一相等（1 源像素 = 1 CSS 像素），不做 CSS 放大。浏览器缩放请设为 100%。物理显示像素还取决于操作系统缩放与屏幕 DPR；本页不把 CSS 像素视作所有显示器上的物理像素。窄窗口请横向滚动卡片。上方展示图可缩放，下方逐个图标才用于实际尺寸检查。</div>
''' + ''.join(sections) + '''
<section class="panel light"><h2>16px 光学校正 · 放大诊断</h2><p>下图中的 16px 图标使用最近邻放大 12 倍，仅用于检查透明边缘、笔画和负空间，不能替代上方 16px 原尺寸检查。</p><img src="optical-16px-enlarged.png" width="760" height="330" style="max-width:100%;height:auto" alt="明确标注12倍放大的16px诊断图"></section>
<div class="links"><a href="../DESIGN.md">设计与交接说明</a><a href="../ACCEPTANCE.md">IC-01～IC-08 验收表</a><a href="../manifest.sha256">SHA256 清单</a><a href="../svg/mdcard-color.svg">彩色 SVG</a><a href="../ico/mdcard-color.ico">Windows ICO</a></div>
<footer>全部资源为本地文件；无网络字体、脚本或请求。本页是资源预览，复杂背景为图形模拟；没有接入或安装应用。</footer>
</main></body></html>
'''
(OUT/'index.html').write_text(html, encoding='utf-8', newline='\n')
print('Wrote hero, native-size, labelled enlarged diagnostic PNGs, and offline HTML.')
