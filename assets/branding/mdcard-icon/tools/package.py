# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Create the final file hash inventory and a deterministic icon-only ZIP."""
from pathlib import Path
import hashlib
import zipfile

ROOT=Path(__file__).resolve().parents[1]
REPO=ROOT.parents[2]
DEST=REPO/'artifacts/icon-design/2026-10-24/MDcard-icon-kit.zip'
def resources():
    return sorted(p for p in ROOT.rglob('*') if p.is_file() and 'node_modules' not in p.parts and '__pycache__' not in p.parts and p.name!='manifest.sha256')
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
print(f'Packaged {len(files)} files: {DEST}')
print(f'SHA256 {hashlib.sha256(DEST.read_bytes()).hexdigest()}')
