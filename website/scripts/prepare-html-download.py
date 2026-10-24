# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Keep the original offline HTML intact when an HTML host injects security JS.

ZIP is a transport wrapper, not a new MDcard export format.
"""
from pathlib import Path
import zipfile,hashlib,json
site=Path(__file__).resolve().parent.parent
folder=site/'src/assets/exports'
source=folder/'manuscript.html'
target=folder/'manuscript-html.zip'
raw=source.read_bytes()
info=zipfile.ZipInfo('从原稿到作品.html',date_time=(2026,10,24,0,0,0))
info.compress_type=zipfile.ZIP_DEFLATED
info.external_attr=0o100644<<16
with zipfile.ZipFile(target,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as output:output.writestr(info,raw)
with zipfile.ZipFile(target) as package:assert package.read(info.filename)==raw
record={'purpose':'Original offline HTML transport; public HTML preview may receive hosting security injection','html_filename':info.filename,'html_bytes':len(raw),'html_sha256':hashlib.sha256(raw).hexdigest(),'zip_bytes':target.stat().st_size,'zip_sha256':hashlib.sha256(target.read_bytes()).hexdigest(),'canonical_source_sha256':hashlib.sha256((site/'samples/从原稿到作品.md').read_bytes()).hexdigest()}
(folder/'manifest.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n','utf8')
print(json.dumps(record,ensure_ascii=False))
