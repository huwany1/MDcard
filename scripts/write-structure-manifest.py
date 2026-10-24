# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Bind structural-editing sources, fresh GUI/unit evidence and local distribution packages."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tarfile
import uuid
import zipfile

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'evidence/structure'
digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
read=lambda p:json.loads(p.read_text(encoding='utf-8-sig'))
windows=read(OUT/'windows-final/result.json')
linux=read(OUT/'linux-final/result.json')
win_exe=ROOT/'dist/windows/MDcard.exe'
linux_exe=ROOT/'dist/linux/mdcard'
assert digest(win_exe)==windows['exe_sha256'].lower()
assert digest(win_exe)==digest(ROOT/'artifacts/structure/MDcard.exe')
assert digest(linux_exe)==linux['exe_sha256'].lower()
tests={}
for platform in ['windows','linux']:
    log=OUT/('windows-native-tests.log' if platform=='windows' else 'linux-release-tests.log')
    match=re.search(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored',log.read_text(encoding='utf-8-sig'))
    assert match,log
    tests[platform]={'passed':int(match[1]),'failed':0,'ignored':0,'log':str(log.relative_to(ROOT))}
assert tests['windows']['passed']==53 and tests['linux']['passed']==54

package_check={}
with zipfile.ZipFile(ROOT/'dist/MDcard-0.1.0-windows-x64.zip') as archive:
    assert hashlib.sha256(archive.read('MDcard.exe')).hexdigest()==digest(win_exe)
    assert 'docs/STRUCTURAL_EDITING.md' in archive.namelist()
    assert archive.read('locales/ui.json')==(ROOT/'locales/ui.json').read_bytes()
    package_check['windows']={'executable_matches':True,'docs_and_locales_current':True,'entries':len(archive.namelist())}
with tarfile.open(ROOT/'dist/MDcard-0.1.0-linux-x64.tar.gz') as archive:
    entries={m.name.removeprefix('./'):m for m in archive.getmembers()}
    assert hashlib.sha256(archive.extractfile(entries['mdcard']).read()).hexdigest()==digest(linux_exe)
    assert 'docs/STRUCTURAL_EDITING.md' in entries
    assert archive.extractfile(entries['locales/ui.json']).read()==(ROOT/'locales/ui.json').read_bytes()
    for name,entry in entries.items():
        assert entry.mode==(0o755 if entry.isdir() or name=='mdcard' else 0o644),(name,oct(entry.mode))
    package_check['linux']={'executable_matches':True,'docs_and_locales_current':True,'permissions':'directories/program 755, other files 644','entries':len(entries)}
(OUT/'package-check.json').write_text(json.dumps({'verification_id':str(uuid.uuid4()),'checks':package_check},indent=2)+'\n',encoding='utf-8',newline='\n')

source_files=[ROOT/name for name in ['Cargo.toml','Cargo.lock','.gitattributes','.gitignore','README.md','docs/ACCEPTANCE.md','docs/STRUCTURAL_EDITING.md','locales/ui.json','scripts/verify-structure-windows.ps1','scripts/verify-structure-linux.py','scripts/write-structure-manifest.py','scripts/package.ps1','scripts/package-linux.sh']]+list((ROOT/'src').glob('*.rs'))
artifacts=[ROOT/name for name in ['dist/windows/MDcard.exe','dist/linux/mdcard','dist/MDcard-0.1.0-windows-x64.zip','dist/MDcard-0.1.0-linux-x64.tar.gz','artifacts/structure/MDcard.exe','artifacts/structure/structure-tests.exe']]
evidence=[p for folder in ['windows-final','linux-final','master-review'] for p in (OUT/folder).rglob('*') if p.is_file()]
evidence += [OUT/'linux-release-tests.log',OUT/'windows-native-tests.log',OUT/'package-check.json',OUT/'linux-package-contents.txt']
state=subprocess.check_output(['docker','exec','mdcard-dev','sh','-lc','ps -eo args'],text=True)
assert not re.search(r'^(/work/(target|dist)/.*mdcard |Xvfb :109|openbox)',state,re.M),state
manifest={
    'verification_id':str(uuid.uuid4()),'date':'2026-10-19','timezone':'Asia/Shanghai',
    'scope':'Source-preserving visual Markdown tables, block/list movement, shared atomic undo and exact GUI input ordering',
    'base_commit':'b28067a','tests':tests,'fmt_check':True,'clippy_all_targets_deny_warnings':True,
    'gui_ids':{'windows':windows['verification_id'],'linux':linux['verification_id']},
    'sources_sha256':{str(p.relative_to(ROOT)).replace('\\','/'):digest(p) for p in sorted(source_files)},
    'artifacts_sha256':{str(p.relative_to(ROOT)).replace('\\','/'):digest(p) for p in artifacts},
    'evidence_sha256':{str(p.relative_to(ROOT)).replace('\\','/'):digest(p) for p in sorted(evidence)},
    'package_checks':package_check,
    'cleanup':{'test_windows_closed':windows['test_windows_closed'],'xvfb_109_openbox_app_closed':True,'other_containers_untouched':True,'settings_isolated':True},
    'boundaries':['Top-level GFM tables; container tables remain source-editable','Protected code/HTML and cross-block structural selections','Windows 125% DPI and Linux X11/Xvfb/Openbox tested, not all platforms/IMEs','Update CPU timings exclude renderer presentation/input delivery','Undo snapshot budget adapts to document opening size'],
}
(OUT/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps(manifest['artifacts_sha256'],indent=2))
print(f"sources={len(source_files)} evidence={len(evidence)} tests={tests}")
