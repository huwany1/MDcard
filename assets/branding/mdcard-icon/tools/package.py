# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Create the final file hash inventory and a deterministic icon-only ZIP."""
from pathlib import Path
import argparse
import hashlib
import zipfile

ROOT=Path(__file__).resolve().parents[1]
REPO=ROOT.parents[2]
parser=argparse.ArgumentParser()
parser.add_argument('--output',type=Path,default=REPO/'artifacts/icon-design/MDcard-icon-kit.zip')
DEST=parser.parse_args().output.resolve()
legal={name:REPO/name for name in ('LICENSE','COPYRIGHT','docs/LICENSING.md','docs/THIRD_PARTY.md')}
def resources():
    return sorted(p for p in ROOT.rglob('*') if p.is_file() and 'node_modules' not in p.parts and '__pycache__' not in p.parts and 'preview' not in p.relative_to(ROOT).parts and p.name not in {'.gitignore','manifest.sha256'})
lines=[f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(ROOT).as_posix()}' for p in resources()]
(ROOT/'manifest.sha256').write_text('\n'.join(lines)+'\n',encoding='utf-8',newline='\n')
files=resources()+[ROOT/'manifest.sha256']
DEST.parent.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile(DEST,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as archive:
    for p in sorted(files):
        name='MDcard-icon-kit/'+p.relative_to(ROOT).as_posix()
        info=zipfile.ZipInfo(name,date_time=(2026,10,24,0,0,0))
        info.compress_type=zipfile.ZIP_DEFLATED
        info.external_attr=0o100644<<16
        archive.writestr(info,p.read_bytes(),compress_type=zipfile.ZIP_DEFLATED,compresslevel=9)
    for name,p in sorted(legal.items()):
        info=zipfile.ZipInfo('MDcard-icon-kit/'+name,date_time=(2026,10,24,0,0,0))
        info.external_attr=0o100644<<16
        archive.writestr(info,p.read_bytes(),compress_type=zipfile.ZIP_DEFLATED,compresslevel=9)
    legal_manifest=''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {name}\n' for name,p in sorted(legal.items()))
    info=zipfile.ZipInfo('MDcard-icon-kit/legal-manifest.sha256',date_time=(2026,10,24,0,0,0))
    info.external_attr=0o100644<<16
    archive.writestr(info,legal_manifest,compress_type=zipfile.ZIP_DEFLATED,compresslevel=9)
print(f'Packaged {len(files)} files: {DEST}')
print(f'SHA256 {hashlib.sha256(DEST.read_bytes()).hexdigest()}')
