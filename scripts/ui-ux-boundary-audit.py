# SPDX-License-Identifier: AGPL-3.0-only
"""Evidence for unchanged business implementations plus the explicit UI diff boundary.

The function hash checks support, rather than replace, review of every mixed-file hunk.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
from datetime import datetime,timezone

ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'evidence/ui-ux/2026-10-28'
base=json.loads((OUT/'baseline-manifest.json').read_text(encoding='utf8'))
allowed={'src/theme.rs','src/main.rs','src/search_ui.rs','src/export_ui.rs','src/recovery.rs','src/structure_ui.rs','locales/ui.json'}
changed=set(subprocess.check_output(['git','diff','--name-only','HEAD'],cwd=ROOT,text=True).splitlines())
assert changed<=allowed,f'Unexpected tracked change: {changed-allowed}'
protected=[]
for name,digest in base['source_sha256'].items():
    if name in allowed:continue
    actual=hashlib.sha256((ROOT/name).read_bytes()).hexdigest()
    assert actual==digest,f'Protected source changed: {name}'
    protected.append(dict(file=name,sha256=actual,unchanged=True))

def function(text,name):
    match=re.search(r'(?m)^    (?:pub(?:\([^)]*\))?\s+)?fn\s+'+re.escape(name)+r'\s*\(',text)
    if not match:raise AssertionError('Missing function '+name)
    start=match.start();i=match.end();depth=0;begun=False
    while i<len(text):
        if text.startswith('//',i):
            i=text.find('\n',i)
            if i<0:break
            continue
        if text.startswith('/*',i):
            end=text.find('*/',i+2);i=len(text) if end<0 else end+2;continue
        raw=re.match(r'r(#+)?"',text[i:])
        if raw:
            end=text.find('"'+(raw[1] or ''),i+raw.end());i=len(text) if end<0 else end+1+len(raw[1] or '');continue
        if text[i]=='"':
            i+=1
            while i<len(text):
                if text[i]=='\\':i+=2
                elif text[i]=='"':i+=1;break
                else:i+=1
            continue
        char=re.match(r"'(?:[^'\\]|\\.)'",text[i:])
        if char:i+=char.end();continue
        if text[i]=='{':depth+=1;begun=True
        elif text[i]=='}':
            depth-=1
            if begun and depth==0:return text[start:i+1]
        i+=1
    raise AssertionError('Function boundary not found: '+name)

names={
 'src/main.rs':['flush_rich_edits','open_export','persist_preferences','apply_preferences_result','can_split_ui','refresh_outline','status_key','new','open_path','open_path_encoding','commit_document','save','request','discard_pending_action','perform','shortcuts','choose_workspace','set_workspace','sync_file_root','open_search','show_encoding'],
 'src/export_ui.rs':['open_for','start','poll','report'],
 'src/search_ui.rs':['open_global_search','search_overlays','search_query_overlays','search_send','dispatch_search_open','search_open_target','update_committed_documents','poll_global_search','dispatch_pending_query','apply_search_preview','request_secondary_close','cancel_secondary_close','save_secondary'],
 'src/recovery.rs':['commit','discover','claim','delete','clear_drafts','operation','discard_drafts','recovery_status','recovery_report'],
 'src/structure_ui.rs':['apply_rich_source','restore_edit_focus','prepare_source_history','source_selection','execute_structure','structure_shortcuts','restore_history','replay_edit_input','replay_text_widget','navigate_cell','cell_id'],
}
checks=[]
for file,functions in names.items():
    before=subprocess.check_output(['git','show','HEAD:'+file],cwd=ROOT).decode().replace('\r\n','\n')
    after=(ROOT/file).read_text(encoding='utf8')
    for name in functions:
        old=function(before,name);new=function(after,name)
        assert old==new,f'Business function changed: {file}::{name}'
        checks.append(dict(file=file,function=name,normalized_source_sha256=hashlib.sha256(new.encode()).hexdigest(),unchanged=True))
renderer_clean=subprocess.run(['git','diff','--exit-code','HEAD','--','renderer'],cwd=ROOT,stdout=subprocess.DEVNULL).returncode==0
assert renderer_clean
patch=subprocess.check_output(['git','diff','HEAD','--','src','locales'],cwd=ROOT)
(OUT/'presentation-only.diff').write_bytes(patch)
result=dict(recorded_utc=datetime.now(timezone.utc).isoformat(),base_commit=base['base_commit'],changed_tracked_files=sorted(changed),protected_files=protected,unchanged_business_functions=checks,renderer_tracked_diff_clean=renderer_clean,diff_sha256=hashlib.sha256(patch).hexdigest(),review='Every mixed-file hunk is mapped in docs/ui-ux-2026-10-28/business-boundary-audit.md; hash evidence is not a semantic proof by itself.')
(OUT/'business-boundary-audit.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf8')
print(json.dumps(dict(protected_files=len(protected),business_functions=len(checks),changed_files=sorted(changed))))
