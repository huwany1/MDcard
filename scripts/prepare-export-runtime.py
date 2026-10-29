# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Prepare self-contained export runtimes from official sources; record exact hashes.

The locked sources file is the reproducibility boundary. --discover is an explicit
maintainer action using official Adoptium metadata, not application behavior.
"""
import argparse,concurrent.futures,hashlib,json,os,shutil,tarfile,tempfile,urllib.request,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
p=argparse.ArgumentParser();p.add_argument('--proxy');p.add_argument('--platform',choices=['windows','linux','all'],default='all');p.add_argument('--discover',action='store_true');args=p.parse_args()
if args.discover:
 raise SystemExit('Redistribution builds require reviewed pinned unbranded Chromium sources and actual notices. Update export-runtime-sources.json explicitly; automatic Chrome-for-Testing discovery is disabled.')
cache=ROOT/'artifacts/export-runtime';cache.mkdir(parents=True,exist_ok=True)
opener=urllib.request.build_opener(urllib.request.ProxyHandler({'https':args.proxy,'http':args.proxy}) if args.proxy else urllib.request.ProxyHandler())
specpath=ROOT/'renderer/export-runtime-sources.json'
spec=json.loads(specpath.read_text('utf-8')) if specpath.exists() else {}
def sha(p):return hashlib.file_digest(p.open('rb'),'sha256').hexdigest()
def download(record):
 target=cache/record['filename']
 if target.exists() and target.stat().st_size>1024 and (not record.get('sha256') or sha(target)==record['sha256']):return target
 temp=target.with_suffix(target.suffix+'.partial')
 with opener.open(record['url'],timeout=180) as r,temp.open('wb') as f:shutil.copyfileobj(r,f)
 digest=sha(temp)
 if record.get('sha256') and digest!=record['sha256']:raise RuntimeError('SHA256 mismatch: '+record['url'])
 os.replace(temp,target);return target
def extract(archive,destination,strip=0):
 destination.mkdir(parents=True,exist_ok=True);base=destination.resolve()
 def output(name):
  parts=Path(name.replace('\\','/')).parts[strip:]
  if not parts:return None
  target=destination.joinpath(*parts).resolve()
  if not target.is_relative_to(base):raise RuntimeError('Archive traversal: '+name)
  return target
 if zipfile.is_zipfile(archive):
  with zipfile.ZipFile(archive) as z:
   for item in z.infolist():
    target=output(item.filename)
    if target is None:continue
    if item.is_dir():target.mkdir(parents=True,exist_ok=True);continue
    if (item.external_attr>>16)&0o170000==0o120000:raise RuntimeError('ZIP symlink rejected')
    target.parent.mkdir(parents=True,exist_ok=True)
    with z.open(item) as src,target.open('wb') as dst:shutil.copyfileobj(src,dst)
 else:
  with tarfile.open(archive) as tar:
   for item in tar:
    target=output(item.name)
    if target is None:continue
    if item.isdir():target.mkdir(parents=True,exist_ok=True)
    elif item.isfile():
     target.parent.mkdir(parents=True,exist_ok=True)
     with tar.extractfile(item) as src,target.open('wb') as dst:shutil.copyfileobj(src,dst)
     target.chmod(item.mode&0o777)
    elif item.issym():
     resolved=(target.parent/item.linkname).resolve()
     if not resolved.is_relative_to(base):raise RuntimeError('Archive symlink escapes runtime')
     # Official JRE legal directories use internal symlinks. Copy after all members.
    else:raise RuntimeError('Unsupported archive member: '+item.name)
  with tarfile.open(archive) as tar:
   for item in tar:
    if item.issym():
     target=output(item.name);source=(target.parent/item.linkname).resolve()
     if source.is_dir():shutil.copytree(source,target,dirs_exist_ok=True)
     elif source.is_file():shutil.copy2(source,target)
def platform_records(platform):
 key=platform
 if args.discover or key not in spec:
  with opener.open('https://api.adoptium.net/v3/assets/latest/21/hotspot?architecture=x64&image_type=jre&os='+platform,timeout=30) as r:jre=json.load(r)[0]['binary']['package']
  browser_os='win64' if platform=='windows' else 'linux64'
  version=json.loads((ROOT/'renderer/node_modules/playwright-core/browsers.json').read_text())['browsers'][0]['browserVersion']
  # Chrome for Testing publishes HTTPS versioned archives, not independent SHA256 files.
  # First acquisition records a transport-observed hash; subsequent rebuilds verify it.
  spec[key]={'java':{'url':jre['link'],'filename':jre['name'],'sha256':jre['checksum'],'verification':'Adoptium published SHA256'},'chromium':{'url':f'https://cdn.playwright.dev/builds/cft/{version}/{browser_os}/chrome-{browser_os}.zip','filename':f'chrome-{version}-{browser_os}.zip','verification':'HTTPS official Playwright CDN, first-acquisition SHA256 pinned; no upstream checksum published'},'pandoc':{'url':f'https://github.com/jgm/pandoc/releases/download/3.12.1/pandoc-3.12.1-{platform}-'+('x86_64.zip' if platform=='windows' else 'amd64.tar.gz'),'filename':'pandoc-3.12.1-'+platform+('.zip' if platform=='windows' else '.tar.gz'),'verification':'HTTPS official release; GitHub SHA256 verified when available'}}
 return [(platform,kind,record) for kind,record in spec[key].items()]
records=[]
for platform in (['windows','linux'] if args.platform=='all' else [args.platform]):records+=platform_records(platform)
specpath.write_text(json.dumps(spec,indent=2)+'\n','utf-8')
def prepare(item):
 platform,kind,record=item;archive=download(record);record['sha256']=sha(archive)
 destination=ROOT/'renderer/runtime'/platform/kind
 # Rebuilds may reuse verified downloads. Linux JRE archives contain read-only
 # files; on Windows chmod maps those modes to the read-only file attribute.
 # Make only this generated destination writable before replacing its content.
 if destination.exists():
  for existing in destination.rglob('*'):
   if existing.is_file():existing.chmod(0o644)
   elif existing.is_dir():existing.chmod(0o755)
 extract(archive,destination,strip=1)
 if kind=='chromium':
  for name in ['chrome','chrome_sandbox','chrome_crashpad_handler']:
   if (destination/name).exists():(destination/name).chmod(0o755)
 if kind=='java' and platform=='linux':
  for f in (destination/'bin').iterdir():f.chmod(0o755)
 if kind=='pandoc':
  source=next(destination.rglob('pandoc.exe' if platform=='windows' else 'pandoc'))
  if source.parent!=destination:shutil.copy2(source,destination/source.name)
  if platform=='linux':(destination/'pandoc').chmod(0o755)
 print(platform,kind,record['sha256'],flush=True)
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:list(pool.map(prepare,records))
specpath.write_text(json.dumps(spec,indent=2)+'\n','utf-8')
# Development checkout picks its host platform without duplicating archive storage.
platform='windows' if os.name=='nt' else 'linux'
for name in ['chromium','java']:
 source=ROOT/'renderer/runtime'/platform/name
 if source.exists():shutil.copytree(source,ROOT/'renderer/runtime'/name,dirs_exist_ok=True)
print('Prepared complete export runtimes. Pandoc remains optional/configurable; located under runtime/<platform>/pandoc.')
