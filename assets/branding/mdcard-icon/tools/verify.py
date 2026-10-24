# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Resource production self-check; independent acceptance is by the master agent."""
from pathlib import Path
from PIL import Image
import hashlib
import io
import json
import struct
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
SIZES = [16,24,32,48,64,128,256,512,1024]
ICO_SIZES = [16,24,32,48,64,128,256]
COLORS = {'color': (45,91,202), 'black': (0,0,0), 'white': (255,255,255)}
checks = []
def check(condition, detail):
    if not condition: raise AssertionError(detail)
    checks.append(detail)

svgs = sorted((ROOT/'svg').rglob('*.svg'))
check(len(svgs)==12, '12 standalone SVG sources including nine optical variants')
for path in svgs:
    document = ET.parse(path)
    for node in document.iter():
        check(node.tag.split('}')[-1] not in {'image','text','script','foreignObject','filter'}, f'{path.relative_to(ROOT)}: no bitmap/font/script/filter element {node.tag}')
        for name,value in node.attrib.items():
            check('href' not in name and not value.lower().startswith(('http:','https:','data:')), f'{path.relative_to(ROOT)}: local attribute {name}')

reference = {}
for variant,color in COLORS.items():
    for size in SIZES:
        path = ROOT/'png'/variant/f'mdcard-{size}.png'
        with Image.open(path) as image:
            check(image.size==(size,size) and image.mode=='RGBA',f'{path.relative_to(ROOT)}: correct dimensions and RGBA')
            extrema=image.getextrema()
            check(all(extrema[i]==(c,c) for i,c in enumerate(color)),f'{path.relative_to(ROOT)}: one exact ink RGB, including antialiased edges')
            alpha=image.getchannel('A')
            check(alpha.getextrema()==(0,255),f'{path.relative_to(ROOT)}: real transparent and opaque pixels')
            border=list(alpha.crop((0,0,size,1)).get_flattened_data())+list(alpha.crop((0,size-1,size,size)).get_flattened_data())+list(alpha.crop((0,0,1,size)).get_flattened_data())+list(alpha.crop((size-1,0,size,size)).get_flattened_data())
            check(not any(border),f'{path.relative_to(ROOT)}: fully transparent outside safety border')
            check(alpha.getpixel((round(size*5/16),round(size*8/16)))==0,f'{path.relative_to(ROOT)}: true transparent M stem')
            if size not in reference: reference[size]=alpha.tobytes()
            else: check(alpha.tobytes()==reference[size],f'{path.relative_to(ROOT)}: alpha identical to color version')
    data=(ROOT/'ico'/f'mdcard-{variant}.ico').read_bytes()
    reserved,typ,count=struct.unpack_from('<HHH',data)
    check((reserved,typ,count)==(0,1,7),f'{variant} ICO: seven real image entries')
    end=6+16*count
    for index,size in enumerate(ICO_SIZES):
        width,height,palette,reserved,planes,bpp,length,offset=struct.unpack_from('<BBBBHHII',data,6+16*index)
        check((width or 256,height or 256)==(size,size),f'{variant} ICO {size}: directory dimensions')
        check((planes,bpp)==(1,32) and offset==end and offset+length<=len(data),f'{variant} ICO {size}: valid contiguous 32bit payload')
        payload=data[offset:offset+length]
        check(payload==(ROOT/'png'/variant/f'mdcard-{size}.png').read_bytes(),f'{variant} ICO {size}: payload exactly matches corrected PNG')
        with Image.open(io.BytesIO(payload)) as image: check(image.size==(size,size),f'{variant} ICO {size}: decoded dimensions')
        end=offset+length
    check(end==len(data),f'{variant} ICO: no trailing data')

html=(ROOT/'preview/index.html').read_text(encoding='utf-8')
check(html.count('data-actual-size=')==36,'HTML: 36 native-size icons across three backgrounds')
check('https://' not in html and 'http://' not in html and '<script' not in html,'HTML: no network resources or scripts')
report={'result':'passed','kind':'production self-check; independent master acceptance is separate','checks_passed':len(checks),'png_count':27,'svg_count':12,'ico_count':3,'native_html_images':36,'checks':checks}
(ROOT/'preview/production-checks.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(f'PASS: {len(checks)} production assertions; 27 PNGs, 12 SVGs, 3 ICOs, 36 HTML native-size icons.')
