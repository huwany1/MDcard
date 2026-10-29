# SPDX-License-Identifier: AGPL-3.0-only
"""Assemble complete offline payloads from verified runtimes and locked dependencies.

Run after prepare-rich-runtime.py, prepare-export-runtime.py, renderer build,
static tracing, and release native builds. Never reads an installed/old application.
No download, initialization, or runtime install occurs in the end-user program.
"""
import argparse,datetime,hashlib,json,os,shutil,subprocess
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--platform',choices=['windows','linux'],required=True)
p.add_argument('--output',type=Path,required=True);p.add_argument('--node-modules',type=Path,required=True)
p.add_argument('--java',type=Path,required=True);p.add_argument('--trace',type=Path,required=True)
p.add_argument('--binary',type=Path,required=True);p.add_argument('--source-commit');a=p.parse_args()
out=a.output.resolve()
if out.exists():raise SystemExit('Use a new output directory; existing payloads are never overwritten')
out.mkdir(parents=True)
version=json.loads((ROOT/'renderer/package.json').read_text())['version']
commit=a.source_commit or subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
records={}
def sha(path):
 with path.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def copy(src,name,reason):
 src=Path(src)
 if not src.is_file():raise FileNotFoundError(src)
 dst=out/name;dst.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(src,dst)
 records[Path(name).as_posix()]=reason
def tree(src,prefix,reason,keep=lambda p:True):
 for f in sorted(Path(src).rglob('*')):
  if f.is_file() and keep(f.relative_to(src)):copy(f,Path(prefix)/f.relative_to(src),reason)
exe='MDcard.exe' if a.platform=='windows' else 'mdcard'
copy(a.binary,exe,'Fresh release native application from frozen product source')
for name in ['worker.mjs','export.mjs','export-docx.mjs','export-fonts.mjs','export-process.mjs']:
 copy(ROOT/'renderer'/name,'renderer/'+name,'Application runtime module')
for name in ['browser.js','katex.css','diagram-font.css','plantuml.jar','fonts/MDcardDiagramSans-Regular.ttf','fonts/MDcardDiagramSansKR-Regular.ttf']:
 copy(ROOT/'renderer/vendor'/name,'renderer/vendor/'+name,'Offline engine, necessary CSS or font')
nm=a.node_modules.resolve();trace=json.loads(a.trace.read_text())
for name in trace['files']:
 name=name.replace('\\','/')
 if not name.startswith('node_modules/'):continue
 rel=name[len('node_modules/'):]
 if rel.startswith('@img/sharp-') or rel.endswith(('.d.ts','.map')) or '/module-test.' in rel or '/serve-index/public/' in rel:continue
 copy(nm/rel,'renderer/node_modules/'+rel,'Locked static import/require closure')
native=['sharp-win32-x64'] if a.platform=='windows' else ['sharp-linux-x64','sharp-libvips-linux-x64']
for name in native:tree(nm/'@img'/name,'renderer/node_modules/@img/'+name,'OS-specific Sharp/libvips runtime',lambda f:f.suffix not in ['.h','.hpp','.a','.lib','.pdb','.o','.obj'])
node=ROOT/'renderer/runtime'/('node.exe' if a.platform=='windows' else 'node')
copy(node,'renderer/runtime/'+node.name,'SHA256-pinned Node 24.21.0')
runtime=ROOT/'renderer/runtime'/a.platform
chrome=runtime/'chromium'
names=(['chrome.exe','chrome.dll','chrome_elf.dll','chrome_wer.dll','d3dcompiler_47.dll','dxcompiler.dll','libEGL.dll','libGLESv2.dll','vk_swiftshader.dll','vulkan-1.dll']+[f.name for f in chrome.glob('*.manifest')] if a.platform=='windows' else ['chrome','chrome_crashpad_handler','libEGL.so','libGLESv2.so','libvk_swiftshader.so','libvulkan.so.1'])
names+=['chrome_100_percent.pak','chrome_200_percent.pak','icudtl.dat','resources.pak','v8_context_snapshot.bin','vk_swiftshader_icd.json','locales/en-US.pak']
for name in names:copy(chrome/name,'renderer/runtime/chromium/'+name,'Pinned headless Chromium executable/library/resource')
tree(a.java,'renderer/runtime/java','Matching jlink Java module closure with all supported locales',lambda f:f.suffix not in ['.jsa','.jfc','.a','.lib','.h'] and f.name not in ['jrt-fs.jar','classlist'] and not f.as_posix().startswith('legal/'))
pandoc='pandoc.exe' if a.platform=='windows' else 'pandoc'
copy(runtime/'pandoc'/pandoc,'renderer/runtime/pandoc/'+pandoc,'Pinned original Pandoc; signed Windows executable preserved')
# Consolidate byte-identical legal texts with an attribution index. All npm
# notices (including unused package notices) remain, and full browser credits.
legal={}
def notice(f,label):
 if f.is_file():
  digest=sha(f);legal.setdefault(digest,{'source':f,'labels':[]})['labels'].append(label)
for name in ['LICENSE','COPYRIGHT','THIRD_PARTY_NOTICES.txt']:notice(ROOT/name,name)
for f in (ROOT/'assets').glob('*'):
 if f.is_file() and ('OFL' in f.name or f.name=='font-sources.json'):notice(f,'assets/'+f.name)
for name in ['Node-LICENSE.txt','PlantUML-MIT-LICENSE.txt','THIRD_PARTY_NOTICES.txt']:notice(ROOT/'renderer'/name,'renderer/'+name)
for f in (ROOT/'renderer/browser-licenses').iterdir():notice(f,'Chromium/'+f.name)
for f in (a.java/'legal').rglob('*'):
 if f.is_file():notice(f,'Java/'+f.relative_to(a.java).as_posix())
for f in nm.rglob('*'):
 if f.is_file() and f.name.lower().startswith(('license','licence','copying','copyright','notice')):notice(f,'npm/'+f.relative_to(nm).as_posix())
for platform in ['windows','linux']:
 for f in (ROOT/'renderer/runtime'/platform/'pandoc').rglob('*'):
  if f.is_file() and f.name.lower().startswith(('copying','copyright','license')):notice(f,'Pandoc/'+f.name)
for digest,item in sorted(legal.items()):copy(item['source'],'licenses/'+digest+'.txt','Attribution: '+'; '.join(item['labels']))
(out/'LICENSES.txt').write_text(f'MDcard {version}, AGPL-3.0-only. Corresponding source: accompanying source archive; product commit {commit}.\nThird-party runtimes retain their original terms.\n\n'+''.join(d+'  '+'; '.join(v['labels'])+'\n' for d,v in sorted(legal.items())),encoding='utf8');records['LICENSES.txt']='License attribution index'
(out/'README.txt').write_text(f'MDcard {version}\nComplete offline package: Node, Java, Chromium, Pandoc, necessary fonts and licenses included.\nCtrl+O open, Ctrl+S save, Ctrl+Shift+S Save As, Ctrl+E views, Ctrl+F search, Ctrl+Alt+E export.\nAutosave requires opt-in; independent crash recovery defaults on.\nSettings/recovery remain in your user profile when uninstalling. See accompanying INSTALL.md.\n',encoding='utf8');records['README.txt']='End-user quick start'
if a.platform=='linux':
 for f in out.rglob('*'):
  if f.is_file():
   f.chmod(0o644)
   with f.open('rb') as handle:magic=handle.read(4)
   if magic==b'\x7fELF':
    # Remove symbols only; previous releases used the same transformation.
    subprocess.run(['strip','--strip-unneeded',str(f)],check=True,capture_output=True);f.chmod(0o755)
 for name in ['renderer/runtime/java/lib/jspawnhelper']: (out/name).chmod(0o755)
files=[dict(path=name,bytes=(out/name).stat().st_size,sha256=sha(out/name),reason=reason) for name,reason in sorted(records.items())]
manifest=dict(schema=2,product='MDcard',version=version,platform=a.platform,architecture='x86_64',productSourceCommit=commit,createdAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),files=files)
(out/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
print(json.dumps(dict(platform=a.platform,files=len(files)+1,payloadBytes=sum(f['bytes'] for f in files)+(out/'manifest.json').stat().st_size,manifestSHA256=sha(out/'manifest.json'))))
