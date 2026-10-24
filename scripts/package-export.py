# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Self-contained native packages: no system Node/Java/browser/Marp required."""
import argparse,concurrent.futures,datetime,hashlib,json,os,shutil,subprocess,tarfile,tomllib,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
p=argparse.ArgumentParser();p.add_argument('--windows-exe',default=str(ROOT/'artifacts/export/MDcard.exe'));p.add_argument('--linux-exe',default=str(ROOT/'artifacts/export/mdcard'));p.add_argument('--reuse-stage');args=p.parse_args()
version=tomllib.loads((ROOT/'Cargo.toml').read_text('utf8'))['package']['version']
source_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
stamp=datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ');base=Path(args.reuse_stage).resolve() if args.reuse_stage else ROOT/'dist/export'/stamp
if not base.resolve().is_relative_to((ROOT/'dist/export').resolve()):raise RuntimeError('Staging must remain under dist/export')
def sha(path):return hashlib.file_digest(path.open('rb'),'sha256').hexdigest()
def copy(source,destination):
 source,destination=Path(source),Path(destination)
 if destination.exists() and source.stat().st_size==destination.stat().st_size and source.stat().st_mtime_ns==destination.stat().st_mtime_ns:return str(destination)
 return shutil.copy2(source,destination)
def tree(source,destination,**kwargs):return shutil.copytree(source,destination,dirs_exist_ok=bool(args.reuse_stage),copy_function=copy,**kwargs)
spec=json.loads((ROOT/'renderer/export-runtime-sources.json').read_text('utf8'))
records=[]
for platform,exe in [('windows',Path(args.windows_exe)),('linux',Path(args.linux_exe))]:
 stage=base/platform;stage.mkdir(parents=True,exist_ok=bool(args.reuse_stage));renderer=stage/'renderer';renderer.mkdir(exist_ok=bool(args.reuse_stage))
 copy(exe,stage/('MDcard.exe' if platform=='windows' else 'mdcard'))
 for name in ['LICENSE','README.md','THIRD_PARTY_NOTICES.txt']:copy(ROOT/name,stage/name)
 for name in ['docs','examples','locales']:tree(ROOT/name,stage/name)
 for name in ['OFL.txt','NotoSansJP-OFL.txt','NotoSansKR-OFL.txt','NotoSansTC-OFL.txt','NotoEmoji-OFL.txt','font-sources.json']:copy(ROOT/'assets'/name,stage/name)
 for name in ['worker.mjs','export.mjs','export-docx.mjs','export-fonts.mjs','export-process.mjs','package.json','package-lock.json','runtime-sources.json','export-runtime-sources.json','THIRD_PARTY_NOTICES.txt','Node-LICENSE.txt','PlantUML-MIT-LICENSE.txt']:copy(ROOT/'renderer'/name,renderer/name)
 tree(ROOT/'renderer/vendor',renderer/'vendor')
 (renderer/'runtime').mkdir(exist_ok=bool(args.reuse_stage));copy(ROOT/'renderer/runtime'/('node.exe' if platform=='windows' else 'node'),renderer/'runtime'/('node.exe' if platform=='windows' else 'node'))
 for kind in ['java','chromium','pandoc']:
  source=ROOT/'renderer/runtime'/platform/kind
  if not source.exists():raise RuntimeError('Missing complete runtime '+str(source))
  tree(source,renderer/'runtime'/kind)
  cached=ROOT/'artifacts/export-runtime'/spec[platform][kind]['filename']
  if sha(cached)!=spec[platform][kind]['sha256']:raise RuntimeError('Runtime archive checksum mismatch '+kind)
 # Locked dual-platform Sharp binaries live in one checkout. Strip foreign binaries per package.
 def ignored(folder,names):
  relative=Path(folder).relative_to(ROOT/'renderer/node_modules').as_posix()
  if relative=='@img':return [n for n in names if n.startswith('sharp-') and (('win32' in n)!=(platform=='windows'))]
  if relative=='.':return ['.cache','.bin']
  return []
 tree(ROOT/'renderer/node_modules',renderer/'node_modules',ignore=ignored)
 print('Copied',platform,flush=True)
 paths=[p for p in sorted(stage.rglob('*'))if p.is_file() and p!=stage/'manifest.json']
 def record(p):return dict(path=p.relative_to(stage).as_posix(),bytes=p.stat().st_size,sha256=sha(p))
 with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:files=list(executor.map(record,paths))
 print('Hashed',platform,len(files),'files',flush=True)
 manifest=dict(version=version,sourceCommit=source_commit,createdAt=stamp,platform=platform,runtimes=spec[platform],dependencies={'nativePipeline':'bundled Node + Chromium + JRE + Marp; no system tools','pandoc':'bundled, path configurable','linuxSystem':'glibc 2.36+, standard desktop libraries libnss3/libatk/libxkbcommon/libgbm/libasound; see docs'},files=files)
 (stage/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n','utf8')
 # Path concatenation is explicit to avoid interpreter-specific mixed path operators.
 archive=ROOT/'dist'/('MDcard-'+version+'-'+platform+'-x64'+('.zip' if platform=='windows' else '.tar.gz'))
 if platform=='windows':
  with zipfile.ZipFile(archive,'w',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
   for file in sorted(stage.rglob('*')):
    if file.is_file():z.write(file,file.relative_to(stage).as_posix())
 else:
  def permissions(info):
   name=info.name.lstrip('./');executable=name=='mdcard' or name=='renderer/runtime/node' or name.startswith('renderer/runtime/java/bin/') or name.endswith('/jspawnhelper') or name in ['renderer/runtime/pandoc/pandoc','renderer/runtime/pandoc/bin/pandoc','renderer/runtime/chromium/chrome','renderer/runtime/chromium/chrome_crashpad_handler','renderer/runtime/chromium/chrome_sandbox']
   info.mode=0o755 if info.isdir() or executable or '.so' in name else 0o644;info.uid=0;info.gid=0;info.uname='';info.gname='';return info
  with tarfile.open(archive,'w:gz',compresslevel=6) as tar:tar.add(stage,arcname='.',filter=permissions)
 records.append(dict(platform=platform,stage=str(stage),archive=str(archive),sha256=sha(archive),bytes=archive.stat().st_size))
evidence=ROOT/'evidence/export/packages';evidence.mkdir(parents=True,exist_ok=True);(evidence/'result.json').write_text(json.dumps(records,indent=2)+'\n','utf8');print(json.dumps(records,indent=2))
