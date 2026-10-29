# SPDX-License-Identifier: AGPL-3.0-only
"""Rebuild both minimal JREs on Windows from matching SHA256-pinned JDKs.

JDKs and jlink are maintenance inputs only. The application receives the module
closure, locale data and licenses, with no first-use download or preparation.
"""
import concurrent.futures,hashlib,json,shutil,subprocess,tarfile,urllib.request,zipfile
from pathlib import Path
root=Path(__file__).resolve().parents[1];spec=json.loads((root/'packaging/build-inputs.json').read_text());base=root/'artifacts/release-inputs';base.mkdir(parents=True,exist_ok=True)
def prepare(item):
 platform,record=item;archive=base/record['url'].rsplit('/',1)[1]
 def sha(p):
  with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
 if not archive.exists() or sha(archive)!=record['sha256']:
  temporary=archive.with_suffix(archive.suffix+'.partial')
  with urllib.request.urlopen(record['url'],timeout=180) as src,temporary.open('wb') as dst:shutil.copyfileobj(src,dst)
  if sha(temporary)!=record['sha256']:raise RuntimeError('JDK checksum mismatch')
  temporary.replace(archive)
 dest=base/('jdk-'+platform)
 if not dest.exists():
  dest.mkdir()
  if zipfile.is_zipfile(archive):
   with zipfile.ZipFile(archive) as z:
    for i in z.infolist():
     if Path(i.filename).is_absolute() or '..' in Path(i.filename).parts:raise RuntimeError('Unsafe JDK member')
    z.extractall(dest)
  else:
   with tarfile.open(archive) as t:
    for member in t:
     f=dest/member.name
     if Path(member.name).is_absolute() or '..' in Path(member.name).parts:raise RuntimeError('Unsafe JDK member')
     if member.isdir():f.mkdir(parents=True,exist_ok=True)
     elif member.isfile() or member.issym() or member.islnk():
      f.parent.mkdir(parents=True,exist_ok=True)
      with t.extractfile(member) as src,f.open('wb') as dst:shutil.copyfileobj(src,dst)
 return platform,next(f for f in dest.iterdir() if f.is_dir())
jdks=dict(concurrent.futures.ThreadPoolExecutor(2).map(prepare,spec['jdk'].items()))
for platform,jdk in jdks.items():
 output=base/('minimal-'+platform)
 if output.exists():continue
 subprocess.run([str(jdks['windows']/'bin/jlink.exe'),'--module-path',str(jdk/'jmods'),'--add-modules',spec['jlink']['modules'],'--include-locales='+spec['jlink']['locales'],*spec['jlink']['options'],'--output',str(output)],check=True)
 print(platform,output)
