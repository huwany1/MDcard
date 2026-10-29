# SPDX-License-Identifier: AGPL-3.0-only
"""External root QA in an isolated --network none runtime-only container.

Accepts only a local .deb and same-payload portable baseline. It does not create
or edit real user data; run only in a disposable acceptance container.
"""
import hashlib,json,os,shutil,statistics,subprocess,tarfile,time
from pathlib import Path
out=Path(os.environ.get('MDCARD_QA_INSTALL_OUTPUT','/qa-output'));out.mkdir(parents=True,exist_ok=True);package=Path('/tmp/mdcard_0.1.5_amd64.deb');archive=Path('/tmp/baseline.tar.gz')
marker=Path('/home/reader/MDcard-data-preservation.txt');marker.write_text('KEEP 中文 document')
def run(args):
 p=subprocess.run(args,capture_output=True,text=True)
 if p.returncode:raise RuntimeError(p.stdout+p.stderr)
 return p
run(['dpkg','-r','mdcard']);samples=[]
for i in range(3):
 begin=time.perf_counter();p=run(['dpkg','-i',str(package)]);samples.append(dict(trial=i+1,seconds=time.perf_counter()-begin,exitCode=p.returncode,stdout=p.stdout,stderr=p.stderr));assert marker.read_text()=='KEEP 中文 document'
 if i<2:run(['dpkg','-r','mdcard']);assert marker.read_text()=='KEEP 中文 document'
repeat=run(['dpkg','-i',str(package)])
(out/'installer-linux.json').write_text(json.dumps(dict(trials=samples,medianSeconds=statistics.median(x['seconds'] for x in samples),userDataPreserved=True,sameVersionReinstall=repeat.returncode,network='none',filesystem='Docker overlay ext4',cache='Warm page cache, fresh app files each trial',packageSHA256=hashlib.file_digest(package.open('rb'),'sha256').hexdigest()),indent=2))
if os.environ.get('MDCARD_QA_INSTALL_ONLY'):
 print('Three protected-package fresh installations and same-version reinstall passed.');raise SystemExit(0)
samples=[]
for i in range(3):
 dest=Path('/tmp')/('baseline-extract-'+str(i));dest.mkdir();begin=time.perf_counter()
 with tarfile.open(archive) as t:t.extractall(dest)
 samples.append(time.perf_counter()-begin);shutil.rmtree(dest)
(out/'baseline-linux.json').write_text(json.dumps(dict(bytes=archive.stat().st_size,sha256=hashlib.file_digest(archive.open('rb'),'sha256').hexdigest(),extractionSeconds=samples,medianExtractionSeconds=statistics.median(samples),filesystem='Same Docker overlay ext4',cache='Warm page cache, fresh destination each trial'),indent=2))
baseline=Path('/home/reader/baseline');baseline.mkdir()
with tarfile.open(archive) as t:t.extractall(baseline)
run(['chown','-R','reader:reader',str(baseline)])
print('Three fresh installs, remove/reinstall and three portable extractions passed.')
