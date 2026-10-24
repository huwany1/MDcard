# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Package the native app and local renderer. System Chromium + Java >=17 required."""
import argparse,datetime,hashlib,json,os,shutil,tarfile,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
parser=argparse.ArgumentParser();parser.add_argument('--windows-exe',default=str(ROOT/'artifacts/rich/MDcard.exe'));parser.add_argument('--linux-exe',default=str(ROOT/'artifacts/rich/mdcard'));parser.add_argument('--module',choices=['rich','search'],default='rich');args=parser.parse_args()
stamp=datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ');base=ROOT/'dist'/args.module/stamp
def sha(path):return hashlib.file_digest(path.open('rb'),'sha256').hexdigest()
spec=json.loads((ROOT/'renderer/runtime-sources.json').read_text())
assert sha(ROOT/'renderer/runtime/node.exe')==spec['nodeWindows']['sha256']
assert sha(ROOT/'renderer/vendor/plantuml.jar')==spec['plantuml']['sha256']
records=[]
for platform,exe in [('windows',Path(args.windows_exe)),('linux',Path(args.linux_exe))]:
 stage=base/platform;stage.mkdir(parents=True,exist_ok=False);renderer=stage/'renderer';renderer.mkdir()
 shutil.copy2(exe,stage/('MDcard.exe' if platform=='windows' else 'mdcard'))
 for name in ['LICENSE','README.md','THIRD_PARTY_NOTICES.txt']:shutil.copy2(ROOT/name,stage/name)
 for folder in ['docs','examples','locales']:shutil.copytree(ROOT/folder,stage/folder)
 for name in ['OFL.txt','NotoSansJP-OFL.txt','NotoSansKR-OFL.txt','NotoSansTC-OFL.txt','NotoEmoji-OFL.txt','font-sources.json']:shutil.copy2(ROOT/'assets'/name,stage/name)
 for name in ['worker.mjs','package.json','package-lock.json','runtime-sources.json','THIRD_PARTY_NOTICES.txt','Node-LICENSE.txt','PlantUML-MIT-LICENSE.txt']:shutil.copy2(ROOT/'renderer'/name,renderer/name)
 shutil.copytree(ROOT/'renderer/vendor',renderer/'vendor')
 (renderer/'runtime').mkdir();shutil.copy2(ROOT/'renderer/runtime'/('node.exe' if platform=='windows' else 'node'),renderer/'runtime'/('node.exe' if platform=='windows' else 'node'))
 (renderer/'node_modules').mkdir()
 for dependency in ['playwright','playwright-core']:shutil.copytree(ROOT/'renderer/node_modules'/dependency,renderer/'node_modules'/dependency)
 manifest=dict(version='0.1.0-'+args.module,createdAt=stamp,platform=platform,dependencies={'node':'bundled 24.21.0','browser':'system Edge/Chrome on Windows; Chromium/Chrome on Linux','java':'system Java >=17 (PlantUML only)'},files=[dict(path=str(p.relative_to(stage)).replace(os.sep,'/'),bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(stage.rglob('*')) if p.is_file()])
 (stage/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
 extension='.zip' if platform=='windows' else '.tar.gz'
 archive=ROOT/'dist'/f'MDcard-0.1.0-{args.module}-{platform}-x64{extension}'
 if platform=='windows':
  with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=6) as z:
   for p in sorted(stage.rglob('*')):
    if p.is_file():z.write(p,p.relative_to(stage).as_posix())
 else:
  def modes(info):info.mode=0o755 if info.isdir() or info.name.lstrip('./') in ['mdcard','renderer/runtime/node'] else 0o644;info.uid=0;info.gid=0;info.uname='';info.gname='';return info
  with tarfile.open(archive,'w:gz',compresslevel=6) as tar:tar.add(stage,arcname='.',filter=modes)
 records.append(dict(platform=platform,stage=str(stage),archive=str(archive),sha256=sha(archive),bytes=archive.stat().st_size))
output=ROOT/'evidence'/args.module/'packages';output.mkdir(parents=True,exist_ok=True);(output/'result.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8');print(json.dumps(records,indent=2))
