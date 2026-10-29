# SPDX-License-Identifier: AGPL-3.0-only
"""Assemble local audited deliverables. Never uploads, publishes, or edits App."""
import argparse,hashlib,json,shutil,subprocess,zipfile
from datetime import datetime,timezone
from pathlib import Path
root=Path(__file__).resolve().parent.parent;e=root/'evidence/release-015'
p=argparse.ArgumentParser();p.add_argument('--output',type=Path,default=root/'dist/0.1.5');p.add_argument('--source',action='store_true');p.add_argument('--evidence',action='store_true');a=p.parse_args();out=a.output.resolve();out.mkdir(parents=True,exist_ok=True)
def sha(path):return hashlib.file_digest(Path(path).open('rb'),'sha256').hexdigest()
def read(path):return json.loads(Path(path).read_text(encoding='utf-8-sig'))
def git(*args):return subprocess.check_output(['git',*args],cwd=root,text=True).strip()
old=out/'MDcard-0.1.5-release.json'
allowed={x['name']:x['sha256'] for x in read(old).get('files',[])} if old.exists() else {}
if old.exists():allowed[old.name]=sha(old)
if (out/'SHA256SUMS.txt').exists():allowed['SHA256SUMS.txt']=sha(out/'SHA256SUMS.txt')
for f in out.iterdir():
 if f.name not in allowed or not f.is_file() or sha(f)!=allowed[f.name]:raise RuntimeError('Inspect unmanaged or modified output before assembly: '+str(f))
expected={'MDcard-0.1.5-windows-x64-setup.exe':'ab961fb64ac6cfdd2f4c782d16b81405fc70d69d661f090863f0e31be502199d','mdcard_0.1.5_amd64.deb':'69418eadbff59b379ad853eb6068ce15cc05f4a945e4f1f3e5a259c7404f0e02'}
for name,digest in expected.items():
 source=root/'artifacts/release-015/final-packages'/name
 if sha(source)!=digest:raise RuntimeError('Selected final package identity changed')
 shutil.copy2(source,out/name)
for source in ['docs/releases/0.1.5-INSTALL.md','docs/releases/0.1.5-COSTS.md','docs/releases/0.1.5-ACCEPTANCE.md','RELEASE_NOTES.md']:
 shutil.copy2(root/source,out/Path(source).name)
identity=read(e/'final-clean-build-identity.json');payload=Path(identity['payloadRoot'])
manifests={}
for platform in ['windows','linux']:
 source=payload/platform/'manifest.json';name='MDcard-0.1.5-'+platform+'-payload.json';shutil.copy2(source,out/name);m=read(source)
 manifests[platform]={'manifestSHA256':sha(source),'productSourceCommit':m['productSourceCommit'],'native':next(f for f in m['files'] if f['path'] in ('MDcard.exe','mdcard')),'files':len(m['files'])}
locks=['packaging/build-inputs.json','renderer/runtime-sources.json','renderer/export-runtime-sources.json','renderer/package-lock.json','scripts/release-tools/package-lock.json','Cargo.lock','rust-toolchain.toml']
inputs={name:{'sha256':sha(root/name)} for name in locks}
for name in locks[:3]:inputs[name]['content']=read(root/name)
(out/'MDcard-0.1.5-build-inputs.json').write_text(json.dumps(inputs,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
snapshot=git('rev-parse','HEAD');frozen='f2bebfb46278b99e880f31383fd5afe5c23fa9bb';packaging='7a429572108bcc3de9e0987fceef8171e5c55e1f'
if git('diff','--name-only',frozen,snapshot,'--','Cargo.toml','Cargo.lock','build.rs','src','assets','locales','vendor','renderer'):raise RuntimeError('Product freeze no longer matches source snapshot')
if git('diff','--name-only',packaging,snapshot,'--','packaging','scripts/build-release.ps1','scripts/package-deb.py','scripts/stage-release.py','scripts/prepare-export-runtime.py','scripts/prepare-rich-runtime.py','scripts/prepare-release-jre.py'):raise RuntimeError('Packaging freeze no longer matches source snapshot')
if a.source:
 branch='codex/release-0.1.5-frozen'
 try:previous=git('rev-parse','--verify','refs/heads/'+branch)
 except subprocess.CalledProcessError:previous=None
 if previous and previous not in (snapshot,read(old).get('sourceSnapshotCommit') if old.exists() else None):raise RuntimeError('Unmanaged existing frozen source branch')
 subprocess.run(['git','branch','-f',branch,snapshot],cwd=root,check=True)
 subprocess.run(['git','archive','--format=zip','--prefix=MDcard-0.1.5/','--output='+str(out/'MDcard-0.1.5-source.zip'),snapshot],cwd=root,check=True)
 subprocess.run(['git','-c','pack.threads=2','bundle','create',str(out/'MDcard-0.1.5-source.bundle'),'refs/heads/'+branch],cwd=root,check=True)
 subprocess.run(['git','bundle','verify',str(out/'MDcard-0.1.5-source.bundle')],cwd=root,check=True,capture_output=True)
if a.evidence:
 with zipfile.ZipFile(out/'MDcard-0.1.5-evidence.zip','w',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
  for f in sorted(e.rglob('*')):
   if f.is_file() and f.suffix!='.ppm':z.write(f,'evidence/release-015/'+f.relative_to(e).as_posix())
guest=e/'windows-clean-guest/guest-evidence/result.json';guest_passed=guest.exists() and read(guest).get('passed') is True
files=[{'name':f.name,'bytes':f.stat().st_size,'sha256':sha(f)} for f in sorted(out.iterdir()) if f.is_file() and f.name not in ('SHA256SUMS.txt','MDcard-0.1.5-release.json')]
release={'schema':1,'product':'MDcard','version':'0.1.5','distribution':'Local artifacts only; no public publication','status':'accepted-pending-master-review' if guest_passed else 'candidate-clean-Windows-guest-pending','createdUTC':datetime.now(timezone.utc).isoformat(),'originalLatestMainCommit':'3fc25d54f7c5964f2a1a7445737d54621f3a12c8','productBuildCommit':frozen,'packagingCommit':packaging,'linuxPackagingBuildCommit':'2a883e9','sourceSnapshotCommit':snapshot,'sourceSnapshotProductAndPackagingTreesMatchFreeze':True,'payloads':manifests,'rawCleanBuildIdentity':identity,'cleanBuild':{'completed':True,'independentClone':identity['sourceRoot'],'log':'evidence/release-015/clean-source-rebuild-final-r2.log','sameProductNativeAndRuntimeInputForBothPlatforms':True,'finalControlPackagingLogs':['windows-rollback-final-r4-build.log','linux-rollback-final-build.log']},'windowsCleanGuestPassed':guest_passed,'measuredCosts':read(e/'final-cost-model.json'),'installedSpace':{'windows':read(e/'final-windows-installed-space.json'),'linux':read(e/'final-linux-installed-space.json')},'runtimeApplicationFirstUseExtraDownloadBytes':0,'linuxSystemDependencyExtraDownloadOnTestedBaseBytes':0,'linuxSystemDependencyOtherHosts':'Use apt --simulate on the actual host; not universally zero','knownLimitations':['Unsigned Windows installer; SmartScreen interaction not browser-download tested','Warm OS cache; fixed bandwidth download estimates separated from measured machine/GUI stages','Windows fast-network A/B can be slower than ZIP baseline','Linux different-content SIGINT recovery verified through explicit preserved-backup command, not automatic full rollback','Upgrade backup requires temporary full previous-payload space and additional wait','Debian12/Openbox/X11/software Mesa tested; no Ubuntu/Wayland/other driver claim','No arbitrary power-loss or disk-corruption transaction guarantee'],'files':files}
(out/'MDcard-0.1.5-release.json').write_text(json.dumps(release,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
checks=[f'{sha(f)}  {f.name}' for f in sorted(out.iterdir()) if f.is_file() and f.name!='SHA256SUMS.txt']
(out/'SHA256SUMS.txt').write_text('\n'.join(checks)+'\n',encoding='utf-8')
print(json.dumps({'output':str(out),'status':release['status'],'sourceSnapshotCommit':snapshot,'files':len(checks)},ensure_ascii=False))
