# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Parse and decode fresh package exports independently of converter exit codes."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import xml.etree.ElementTree as ET
import zipfile

from PIL import Image
from pypdf import PdfReader

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--artifacts', type=Path, default=ROOT / 'artifacts/export-packages')
parser.add_argument('--output', type=Path, default=ROOT / 'evidence/bug-audit/master-review/export-artifacts.json')
args = parser.parse_args()
FORMATS = ['html', 'pdf', 'png', 'jpeg', 'docx', 'epub', 'marp-html', 'marp-pdf', 'marp-pptx']
checks = []
for platform in ['windows', 'linux']:
    output = args.artifacts / platform
    results = json.loads((output / 'result.json').read_text('utf8'))
    complete = [item for item in results if item.get('type') == 'complete']
    assert sorted(item['format'] for item in complete) == sorted(FORMATS)
    assert all(item['exitCode'] == 0 and item['pathEmpty'] for item in complete)
    assert (output / 'keep.html').read_text('utf8') == 'OLD OUTPUT'
    for item in complete:
        filename = 'bundled.' + item['format'].replace('marp-', 'slides.')
        with (output / filename).open('rb') as stream:
            assert hashlib.file_digest(stream, 'sha256').hexdigest() == item['sha256']
    html = (output / 'bundled.html').read_text('utf8')
    assert '<script' not in html.lower() and 'data:image/' in html and 'footnote' in html
    sources = re.findall(r'\bsrc=[\"\x27]([^\"\x27]*)', html, flags=re.I)
    assert all(value.startswith(('data:', '#')) for value in sources), sources
    pdf = PdfReader(output / 'bundled.pdf')
    assert len(pdf.pages) >= 2 and len(pdf.outline) >= 1
    assert 'Nested' in ''.join(page.extract_text() or '' for page in pdf.pages)
    slide_pdf = PdfReader(output / 'bundled.slides.pdf')
    assert len(slide_pdf.pages) == 3
    with zipfile.ZipFile(output / 'bundled.docx') as docx:
        assert docx.testzip() is None
        document = docx.read('word/document.xml')
        ET.fromstring(document)
        assert b'bookmarkStart' in document and b'TOC' in document and b'w:type="page"' in document
        docx_media = [name for name in docx.namelist() if name.startswith('word/media/')]
        assert len(docx_media) >= 4
    with zipfile.ZipFile(output / 'bundled.epub') as epub:
        assert epub.testzip() is None
        assert epub.read('mimetype') == b'application/epub+zip'
        nav = next(name for name in epub.namelist() if name.endswith('nav.xhtml'))
        ET.fromstring(epub.read(nav))
        assert b'Nested' in epub.read(nav) and b'Third level' in epub.read(nav)
        epub_media = [name for name in epub.namelist() if name.endswith(('.png', '.svg'))]
        assert len(epub_media) >= 4
    with zipfile.ZipFile(output / 'bundled.slides.pptx') as pptx:
        assert pptx.testzip() is None
        slides = [name for name in pptx.namelist() if re.fullmatch(r'ppt/slides/slide\d+\.xml', name)]
        assert len(slides) == 3
        for name in slides:
            ET.fromstring(pptx.read(name))
    sizes = []
    for extension in ['png', 'jpeg']:
        with Image.open(output / ('bundled.' + extension)) as image:
            sizes.append(image.size)
            image.verify()
    assert sizes[0] == sizes[1] and sizes[0][0] > 100 and sizes[0][1] > 100
    checks.append(dict(platform=platform, passed=True, formats=FORMATS, pdf_pages=len(pdf.pages),
                       marp_pdf_pages=len(slide_pdf.pages), pptx_slides=len(slides), docx_media=len(docx_media),
                       epub_media=len(epub_media), image_size=sizes[0], timeout_keeps_target=True,
                       html_resource_src_all_embedded=True))
    print('Artifact structures verified:', platform, flush=True)
destination = args.output
destination.parent.mkdir(parents=True, exist_ok=True)
destination.write_text(json.dumps(checks, indent=2) + '\n', 'utf8')
print(json.dumps(checks, indent=2))
