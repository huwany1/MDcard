# SPDX-License-Identifier: AGPL-3.0-only
"""Different-content, valid full-payload synthetic previous-package interruption.

Only README/manifest fixture metadata changes; executable and full runtime stay
valid. This is explicitly not a historical 0.1.4 application release.
"""
import hashlib,json,os,shutil,signal,subprocess,tempfile,time
from pathlib import Path
out=Path(os.environ.get('MDCARD_QA_DIFFERENT_OUTPUT','/qa-output/final-linux-different-upgrade'));out.mkdir(exist_ok=True)
package=Path('/tmp/mdcard-final.deb');app=Path('/opt/mdcard');checks={}
def sha(p):return hashlib.file_digest(Path(p).open('rb'),'sha256').hexdigest()
def run(args,ok=True):
 p=subprocess.run(args,capture_output=True,text=True)
 if ok and p.returncode:raise RuntimeError(p.stdout+p.stderr)
 return dict(argv=args,exitCode=p.returncode,stdout=p.stdout,stderr=p.stderr)
def hashes(root):return {str(f.relative_to(root)):sha(f) for f in root.rglob('*') if f.is_file()}
def save():(out/'result.json').write_text(json.dumps(checks,indent=2)+'\n')
fixture=Path(tempfile.mkdtemp(prefix='mdcard-full-valid-old-qa-'));root=fixture/'opt/mdcard'
try:
 run(['dpkg-deb','--control',str(package),str(fixture/'DEBIAN')]);control=fixture/'DEBIAN/control';control.write_text(control.read_text().replace('Version: 0.1.5','Version: 0.1.4~qa-different-content'))
 shutil.copytree(app,root)
 for name in ['usr/bin/mdcard','usr/share/applications/mdcard.desktop','usr/share/icons/hicolor/scalable/apps/mdcard.svg','usr/share/doc/mdcard/copyright']:
  target=fixture/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2('/'+name,target)
 old_readme=root/'README.txt';old_readme.write_text('SYNTHETIC QA VALID OLD RESOURCE. Not a historical release.\n'+old_readme.read_text())
 manifest=root/'manifest.json';data=json.loads(manifest.read_text());data['qaFixture']='Same valid 0.1.5 executable/runtime, old README content; dpkg 0.1.4~qa metadata only'
 entry=next(e for e in data['files'] if e['path']=='README.txt');entry.update(bytes=old_readme.stat().st_size,sha256=sha(old_readme));manifest.write_text(json.dumps(data,indent=2)+'\n');manifest.chmod(0o644)
 checksum=fixture/'usr/share/mdcard/payload.sha256';checksum.parent.mkdir(parents=True,exist_ok=True);checksum.write_text(''.join(e['sha256']+'  '+e['path']+'\n' for e in data['files'])+sha(manifest)+'  manifest.json\n');checksum.chmod(0o644)
 old=Path('/tmp/mdcard-full-valid-old-qa.deb');run(['dpkg-deb','--root-owner-group','-Znone','--build',str(fixture),str(old)])
 checks.update(packageSHA256=sha(package),fixtureSHA256=sha(old),fixtureScope=data['qaFixture'],fixtureInstall=run(['dpkg','-i',str(old)]))
 before=hashes(app);old_hash=sha(app/'README.txt');checks['beforeVersionCommand']=run([str(app/'mdcard'),'--version']);save()
 document=Path('/home/reader/qa/final-linux-gui/installed-3/state/keep-interruption.md');document.parent.mkdir(parents=True,exist_ok=True);document.write_text('Keep user document 中文\n');document_before=sha(document)
 log=(out/'interrupt.log').open('w');proc=subprocess.Popen(['dpkg','-i',str(package)],stdout=log,stderr=subprocess.STDOUT)
 observed=False;begin=time.monotonic()
 while proc.poll() is None and time.monotonic()-begin<60:
  target=app/'README.txt'
  if target.exists() and sha(target)!=old_hash:observed=True;checks['newReadmeObservedSHA256']=sha(target);break
  time.sleep(.001)
 checks['differentOldResourceActuallyReplaced']=observed;assert observed
 checks['signalSecondsAfterDpkgStart']=time.monotonic()-begin;proc.send_signal(signal.SIGINT);checks['interruptExitCode']=proc.wait(timeout=180);log.close()
 after=hashes(app);checks['oldPayloadFiles']=len(before);checks['oldPayloadMismatches']=[p for p,h in before.items() if after.get(p)!=h];checks['oldREADMERecovered']=sha(app/'README.txt')==old_hash;checks['userDocumentPreserved']=sha(document)==document_before;checks['afterVersionCommand']=run([str(app/'mdcard'),'--version'],False);checks['automaticRestore']=not checks['oldPayloadMismatches'];save()
 backup=Path('/var/lib/mdcard/installer-backup')
 if checks['oldPayloadMismatches'] and (backup/'complete').exists():
  checks['explicitRecoveryCommand']=run(['sh',str(backup/'restore.sh'),'--mdcard-restore']);after=hashes(app);checks['manualPayloadMismatches']=[p for p,h in before.items() if after.get(p)!=h];checks['oldREADMERecovered']=sha(app/'README.txt')==old_hash;checks['afterVersionCommand']=run([str(app/'mdcard'),'--version'],False)
 checks['repairConfigure']=run(['dpkg','--configure','-a'],False);checks['repairFinalReinstall']=run(['dpkg','-i',str(package)]);checks['finalPackageVersion']=run(['dpkg-query','-W','-f=${Version}','mdcard']);checks['passed']=not checks.get('manualPayloadMismatches',checks['oldPayloadMismatches']) and checks['oldREADMERecovered'] and checks['userDocumentPreserved'] and checks['afterVersionCommand']['exitCode']==0;save();print(json.dumps(checks,indent=2))
finally:
 shutil.rmtree(fixture)
