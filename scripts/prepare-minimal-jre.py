# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Maintenance-only: obtain matching official JDKs, verify published SHA256,
and build small application-specific Java images. JDKs never enter payloads."""
from pathlib import Path
import argparse,concurrent.futures, hashlib, json, tarfile, zipfile, requests
ROOT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--source-root',type=Path,default=ROOT);p.add_argument('--output',type=Path,required=True);a=p.parse_args();ROOT=a.source_root.resolve()
OUT=a.output.resolve();OUT.mkdir(parents=True,exist_ok=True)
SPEC=json.loads((ROOT/'renderer/export-runtime-sources.json').read_text('utf8'))
def prepare(platform):
    record=SPEC[platform]['java'];url=record['url'].replace('-jre_','-jdk_')
    archive=OUT/url.rsplit('/',1)[1]
    s=requests.Session()
    checksum=s.get(url+'.sha256.txt',timeout=60);checksum.raise_for_status();expected=checksum.text.split()[0]
    for attempt in range(8):
        if archive.exists() and hashlib.file_digest(archive.open('rb'),'sha256').hexdigest()==expected:break
        offset=archive.stat().st_size if archive.exists() else 0
        try:
            r=s.get(url,headers={'Range':f'bytes={offset}-'} if offset else {},stream=True,timeout=(30,180));r.raise_for_status()
            with archive.open('ab' if offset and r.status_code==206 else 'wb') as f:
                for b in r.iter_content(1024*1024):f.write(b)
        except requests.RequestException as error:print(platform,'download retry',attempt+1,type(error).__name__,flush=True)
    actual=hashlib.file_digest(archive.open('rb'),'sha256').hexdigest()
    if actual!=expected:raise RuntimeError('Official JDK checksum mismatch')
    dest=OUT/platform;dest.mkdir(exist_ok=True)
    if platform=='windows':
        with zipfile.ZipFile(archive) as z:z.extractall(dest)
    else:
        with tarfile.open(archive) as t:t.extractall(dest,filter='data')
    result={'platform':platform,'url':url,'publishedSHA256':expected,'actualSHA256':actual,'jdkHome':str(next(p for p in dest.iterdir() if p.is_dir()))}
    (OUT/(platform+'.json')).write_text(json.dumps(result,indent=2)+'\n',encoding='utf8');print(json.dumps(result),flush=True)
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:list(pool.map(prepare,['windows','linux']))
