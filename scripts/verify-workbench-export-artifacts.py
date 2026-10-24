# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Inspect real export files, independently of the engine's completion events."""
from pathlib import Path
import argparse
import hashlib
import json
import re
import zipfile
import xml.etree.ElementTree as ET
from PIL import Image
from pypdf import PdfReader

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('directories', nargs='+', type=Path)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
reports = []
for directory in args.directories:
    records=json.loads((directory/'result.json').read_text('utf8'))
    platform=directory.name
    checked=[]
    assert len(records)==10, 'Nine formats plus protected timeout target are required'
    for record in records:
        if 'negative' in record:
            assert record['ok'] is True
            assert (directory/'keep.html').read_text('utf8')=='OLD OUTPUT'
            continue
        raw=str(record['output'])
        path=ROOT/raw.removeprefix('/work/') if raw.startswith('/work/') else Path(raw)
        assert hashlib.sha256(path.read_bytes()).hexdigest()==record['sha256']
        fmt=record['format']; result={'format':fmt,'path':str(path),'sha256':record['sha256']}
        if fmt in {'pdf','marp-pdf'}:
            reader=PdfReader(path)
            result['pages']=len(reader.pages)
            result['text_characters']=sum(len(p.extract_text() or '') for p in reader.pages)
            assert result['pages']==(3 if fmt=='marp-pdf' else 2)
            assert result['text_characters']>(30 if fmt=='marp-pdf' else 100)
        elif fmt in {'png','jpeg'}:
            with Image.open(path) as pic:
                result['dimensions']=list(pic.size)
                assert pic.width>500 and pic.height>500
                pic.verify()
        elif fmt in {'docx','epub','marp-pptx'}:
            with zipfile.ZipFile(path) as bundle:
                names=bundle.namelist()
                for name in names:
                    if name.endswith(('.xml','.rels','.opf','.xhtml')):
                        ET.fromstring(bundle.read(name))
                media=[n for n in names if '/media/' in n or n.startswith('EPUB/images/')]
                result['media']=len(media)
                for name in media:
                    if name.lower().endswith(('.png','.jpg','.jpeg')):
                        import io
                        with Image.open(io.BytesIO(bundle.read(name))) as picture: picture.verify()
                if fmt=='docx':
                    assert 'word/document.xml' in names and len(media)>=5
                elif fmt=='epub':
                    assert bundle.read('mimetype')==b'application/epub+zip'
                    assert 'META-INF/container.xml' in names
                    result['image_resources']=sum(n.lower().endswith(('.png','.jpg','.jpeg','.svg')) for n in names)
                    assert result['image_resources']>=5
                else:
                    result['slides']=sum(bool(re.fullmatch(r'ppt/slides/slide\d+\.xml',n)) for n in names)
                    assert result['slides']==3
        else:
            html=path.read_text('utf8')
            result['embedded_images']=html.count('data:image/')
            assert result['embedded_images']>0
            assert not re.search(r'<(?:script|img)\b[^>]*(?:src)\s*=\s*["\']https?://',html,re.I)
        result['passed']=True
        checked.append(result)
    assert len(checked)==9
    reports.append({'directory':str(directory),'platform':platform,'passed':True,'artifacts':checked})
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text(json.dumps(reports,ensure_ascii=False,indent=2)+'\n','utf8')
print(json.dumps(reports,ensure_ascii=False,indent=2))
