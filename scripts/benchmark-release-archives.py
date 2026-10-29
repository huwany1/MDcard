# SPDX-License-Identifier: AGPL-3.0-only
"""Same-payload portable baseline. Does not benchmark user clicking or network.

ZIP level 6 / tar.gz level 6 match the repository's previous packaging method.
Each output is a local benchmark artifact, not an additional supported format.
"""
import argparse,hashlib,json,shutil,statistics,tarfile,time,zipfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('payload',type=Path);p.add_argument('output',type=Path);p.add_argument('--platform',choices=['windows','linux'],required=True);p.add_argument('--extract-root',type=Path);a=p.parse_args()
a.output.mkdir(parents=True,exist_ok=True);archive=a.output/('baseline.zip' if a.platform=='windows' else 'baseline.tar.gz')
begin=time.perf_counter()
if a.platform=='windows':
 with zipfile.ZipFile(archive,'w',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
  for f in sorted(a.payload.rglob('*')):
   if f.is_file():z.write(f,f.relative_to(a.payload).as_posix())
else:
 with tarfile.open(archive,'w:gz',compresslevel=6) as t:t.add(a.payload,arcname='.')
build=time.perf_counter()-begin;samples=[]
for i in range(3):
 parent=a.extract_root or a.output;parent.mkdir(parents=True,exist_ok=True)
 dest=parent/f'extracted-{i+1}';dest.mkdir();start=time.perf_counter()
 if a.platform=='windows':
  with zipfile.ZipFile(archive) as z:z.extractall(dest)
 else:
  with tarfile.open(archive) as t:
   for m in t.getmembers():
    if Path(m.name).is_absolute() or '..' in Path(m.name).parts or not(m.isfile() or m.isdir()):raise RuntimeError('Unsafe baseline member')
   t.extractall(dest)
 samples.append(time.perf_counter()-start)
 # Only delete this benchmark's explicitly resolved output child.
 assert dest.resolve().parent==parent.resolve();shutil.rmtree(dest)
result=dict(platform=a.platform,archive=str(archive),bytes=archive.stat().st_size,sha256=hashlib.file_digest(archive.open('rb'),'sha256').hexdigest(),buildSeconds=build,extractionSeconds=samples,medianExtractionSeconds=statistics.median(samples),cache='OS page cache warm; fresh destination each trial; no claim of cold disk cache',inputManifestSHA256=hashlib.file_digest((a.payload/'manifest.json').open('rb'),'sha256').hexdigest())
(a.output/'baseline-measurements.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
