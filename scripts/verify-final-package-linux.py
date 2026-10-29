# SPDX-License-Identifier: AGPL-3.0-only
"""Final .deb lifecycle acceptance in a disposable runtime-only QA container."""
import hashlib,json,os,shutil,signal,statistics,subprocess,tarfile,tempfile,time
from pathlib import Path
out=Path('/qa-output/linux-final-package');out.mkdir(exist_ok=True)
package=Path('/tmp/mdcard-final.deb');app=None;display=None;checks={}
def sha(p):
 with Path(p).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def run(args,ok=True):
 p=subprocess.run(args,capture_output=True,text=True)
 if ok and p.returncode:raise RuntimeError(p.stdout+p.stderr)
 return dict(argv=args,exitCode=p.returncode,stdout=p.stdout,stderr=p.stderr)
def save():
 (out/'result.json').write_text(json.dumps(dict(packageSHA256=sha(package),checks=checks),indent=2)+'\n')
def hashes(root):return {str(p):sha(p) for p in root.rglob('*') if p.is_file()}
exe=Path('/opt/mdcard/mdcard');binary_hash=sha(exe)
preserved=Path('/home/reader/qa/installed-linux-lifecycle-v4');before=hashes(preserved)
try:
 # Xvfb is test equipment. The application runs as the actual normal reader user.
 display=subprocess.Popen(['Xvfb',':99','-screen','0','1400x1100x24'],stdout=(out/'xvfb.log').open('w'),stderr=subprocess.STDOUT);time.sleep(.5)
 doc=out/'guard.md';doc.write_text('# Running installation guard 中文\n');os.chown(out,1000,1000);os.chown(doc,1000,1000)
 app=subprocess.Popen(['runuser','-u','reader','--','env','-i','HOME=/home/reader','DISPLAY=:99','LANG=C.UTF-8','LIBGL_ALWAYS_SOFTWARE=1','LP_NUM_THREADS=2','XDG_CONFIG_HOME=/home/reader/qa/guard-profile/config','XDG_STATE_HOME=/home/reader/qa/guard-profile/state','PATH=',str(exe),str(doc)],stdout=(out/'app.log').open('w'),stderr=subprocess.STDOUT);time.sleep(2);assert app.poll() is None
 checks['runningUpgradeRefused']=run(['dpkg','-i',str(package)],False)
 checks['runningRemoveRefused']=run(['dpkg','-r','mdcard'],False)
 assert checks['runningUpgradeRefused']['exitCode']!=0 and checks['runningRemoveRefused']['exitCode']!=0
 assert app.poll() is None and sha(exe)==binary_hash and doc.read_text()=='# Running installation guard 中文\n'
 checks['activeEditorAndDocumentPreserved']=True;save()
 app.terminate();app.wait(timeout=10);app=None
 run(['dpkg','--configure','-a'])
 # A lower-version synthetic package is a QA fixture, never a historical .deb.
 fixture=Path(tempfile.mkdtemp(prefix='mdcard-previous-fixture-'));(fixture/'DEBIAN').mkdir();(fixture/'opt/mdcard').mkdir(parents=True)
 (fixture/'DEBIAN/control').write_text('Package: mdcard\nVersion: 0.1.4~qa-fixture\nArchitecture: amd64\nMaintainer: MDcard QA <qa@localhost>\nDescription: SYNTHETIC QA lower-version installer fixture only\n')
 (fixture/'opt/mdcard/QA-FIXTURE.txt').write_text('QA ONLY. Not a historical release. No old runtime compatibility claim.\n')
 old=Path('/tmp/mdcard-previous-qa.deb');run(['dpkg-deb','--root-owner-group','--build',str(fixture),str(old)]);checks['syntheticPreviousInstall']=run(['dpkg','-i',str(old)]);checks['upgradeFromFixture']=run(['dpkg','-i',str(package)]);assert sha(exe)==binary_hash
 checks['upgradeFixtureScope']='dpkg lower-version ownership/version upgrade only; not a historical full-runtime .deb';checks['userDataAfterUpgrade']=hashes(preserved)==before;assert checks['userDataAfterUpgrade'];save()
 # SIGINT during a reinstall must leave data intact and a documented repair path.
 interrupted=subprocess.Popen(['dpkg','-i',str(package)],stdout=(out/'interrupt.log').open('w'),stderr=subprocess.STDOUT);time.sleep(.08)
 if interrupted.poll() is None:interrupted.send_signal(signal.SIGINT)
 code=interrupted.wait(timeout=60);checks['interruptExitCode']=code;checks['previousExecutableAfterInterrupt']=exe.exists() and sha(exe)==binary_hash;checks['userDataAfterInterrupt']=hashes(preserved)==before
 checks['repairConfigure']=run(['dpkg','--configure','-a'],False);checks['repairReinstall']=run(['dpkg','-i',str(package)]);assert sha(exe)==binary_hash and hashes(preserved)==before;save()
 # Three same-environment fresh installations; no compression/build overlaps.
 samples=[];run(['dpkg','-r','mdcard'])
 for i in range(3):
  begin=time.perf_counter();result=run(['dpkg','-i',str(package)]);samples.append(dict(trial=i+1,seconds=time.perf_counter()-begin,exitCode=result['exitCode']))
  assert sha(exe)==binary_hash and hashes(preserved)==before
  if i<2:run(['dpkg','-r','mdcard'])
 checks['freshInstallTrials']=samples;checks['medianInstallSeconds']=statistics.median(x['seconds'] for x in samples);checks['sameVersionReinstall']=run(['dpkg','-i',str(package)])
 checks['purge']=run(['dpkg','--purge','mdcard']);checks['userDataAfterPurge']=hashes(preserved)==before;checks['ownedExecutableRemoved']=not exe.exists();assert checks['userDataAfterPurge'] and checks['ownedExecutableRemoved']
 checks['finalInstall']=run(['dpkg','-i',str(package)]);assert sha(exe)==binary_hash
 manifest=json.loads(Path('/opt/mdcard/manifest.json').read_text());errors=[]
 for entry in manifest['files']:
  file=Path('/opt/mdcard')/entry['path']
  if not file.is_file() or file.stat().st_size!=entry['bytes'] or sha(file)!=entry['sha256']:errors.append(entry['path'])
 assert not errors;checks['finalManifestErrors']=errors;checks['sourceCommit']=manifest['productSourceCommit'];checks['finalInstalledLogicalBytes']=sum(p.stat().st_size for p in Path('/opt/mdcard').rglob('*') if p.is_file());checks['finalInstalledDu']=run(['du','-s','-B1','/opt/mdcard']);checks['passed']=True;save();print(json.dumps(checks,indent=2))
finally:
 if app and app.poll() is None:app.terminate();app.wait(timeout=10)
 if display and display.poll() is None:display.terminate();display.wait(timeout=10)
