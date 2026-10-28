# SPDX-License-Identifier: AGPL-3.0-only
"""End-to-end native UI checks on the task's private Xvfb display, with byte assertions."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import time
import traceback
import uuid
from datetime import datetime,timezone

ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'evidence/ui-ux/2026-10-28'
run_id='smoke-'+str(uuid.uuid4())[:8];work=OUT/'validation'/run_id;work.mkdir(parents=True)
helper=ROOT/'scripts/ui-ux-native.py';checks=[]
candidate='evidence/ui-ux/2026-10-28/binaries/candidate-linux'
source=work/'工作札记.md';second=work/'开发笔记.md'
shutil.copy2(OUT/'fixtures/工作札记.md',source);shutil.copy2(OUT/'fixtures/开发笔记.md',second)
original=source.read_bytes();addition='\n\n## UI acceptance\n\nEdited 中文 and English.\n'

def act(*commands):
    subprocess.run([sys.executable,str(helper),json.dumps(commands,ensure_ascii=False)],check=True)

def report(kind='ui'):
    state=json.loads((OUT/'native-state.json').read_text())
    for _ in range(20):
        try:return json.loads(Path(state[kind]).read_text())
        except (OSError,ValueError):time.sleep(.05)
    raise RuntimeError('Diagnostic report unavailable')

def check(name,value,expected=True):
    good=value==expected;checks.append(dict(name=name,actual=value,expected=expected,passed=good))
    if not good:raise AssertionError(f'{name}: {value!r} != {expected!r}')

def wait_for(predicate,seconds=10):
    until=time.monotonic()+seconds
    while time.monotonic()<until:
        if predicate():return
        time.sleep(.15)
    raise TimeoutError('Native state did not reach expected result')

status='failed';failure=None
try:
    act(['key','Escape'],['close'],['launch',candidate,'validation-native/'+run_id,run_id,str(source.relative_to(ROOT))],['control','编辑'],['control','main-source-editor'],['key','ctrl+End'],['paste',addition])
    check('editing_marks_real_document_dirty',report()['dirty'])
    act(['key','ctrl+s'],['wait',.7])
    check('utf8_chinese_exact_save_bytes',source.read_bytes()==original+addition.encode())
    check('save_clears_dirty_after_completion',report()['dirty'],False)
    act(['key','ctrl+z'],['key','ctrl+s'],['wait',.7])
    check('undo_retains_original_bytes',source.read_bytes()==original)
    act(['key','ctrl+shift+z'],['key','ctrl+s'],['wait',.7],['shot','01-save-undo-redo'])
    check('redo_restores_exact_edit',source.read_bytes()==original+addition.encode())
    act(['control','navigation:Files'],['focus-control','navigation:Files'],['key','Tab'])
    check('tab_focus_reaches_outline',any(x['name']=='navigation:Outline' and x.get('focus') for x in report()['controls']))
    act(['key','Return'])
    check('enter_activates_outline',report()['navigation'],1)
    act(['key','shift+Tab'])
    check('shift_tab_returns_to_files',any(x['name']=='navigation:Files' and x.get('focus') for x in report()['controls']))
    act(['key','space'],['shot','02-keyboard-navigation'])
    check('space_activates_files',report()['navigation'],0)
    act(['key','ctrl+f'],['paste','UI acceptance'],['wait',.8],['shot','03-search-results'])
    check('real_find_matches_actual_document',report('search')['results'],1)
    act(['control','result-0-0','search'],['wait',.3])
    check('search_result_selects_source_range',report()['selection'][0]!=report()['selection'][1])
    check('find_did_not_change_disk',source.read_bytes()==original+addition.encode())
    act(['key','ctrl+p'],['key','ctrl+a'],['paste','开发笔记'],['wait',.7])
    check('quick_query_is_explicitly_replaced',report('search')['quick_query'],'开发笔记')
    act(['key','Return'],['wait',.3])
    check('quick_switch_opens_real_second_file',Path(report()['document_path']).name,second.name)
    act(['key','ctrl+p'],['key','ctrl+a'],['paste','工作札记'],['wait',.7],['key','Return'],['wait',.3])
    check('switch_back_opens_exact_main_path',str(Path(report()['document_path'])),str(source))
    check('switch_back_preserves_saved_file',source.read_bytes()==original+addition.encode())
    act(['key','ctrl+p'],['key','ctrl+a'],['paste','开发笔记'],['wait',.7],['key','shift+Return'],['shot','04-secondary-pane'])
    check('second_pane_is_real_buffer',report('search')['secondary'] is not None)
    act(['control','关闭'],['wait',.3])
    check('close_second_pane_keeps_main_file',Path(report()['document_path']).name,source.name)
    act(['open',str(second.relative_to(ROOT))],['wait',.5])
    check('native_open_dialog_opens_selected_file',Path(report()['document_path']).name,second.name)
    act(['open',str(source.relative_to(ROOT))],['wait',.5])
    check('native_reopen_preserves_edited_bytes',report()['document_text_bytes'],len(original+addition.encode()))
    act(['control','编辑'],['control','main-source-editor'])
    check('export_binds_explicitly_focused_main_pane',report()['active_pane'],'main')
    act(['control','分屏'],['click',161,62],['control','导出…'],['shot','05-export-config'],['control','export','export'],['wait',.7],['shot-root','06-native-export-picker'])
    pdf=work/'UI-acceptance.pdf'
    act(['key','ctrl+a'],['paste',str(pdf)],['key','Return'],['wait',1])
    wait_for(lambda:(report('export').get('last') or {}).get('type')=='complete',40)
    pdf=Path(report('export')['last']['output'])
    check('selected_pdf_output_stays_in_test_directory',pdf.parent.resolve()==work.resolve())
    check('selected_output_is_pdf',pdf.suffix.lower(),'.pdf')
    check('native_pdf_export_has_pdf_header',pdf.read_bytes().startswith(b'%PDF-'))
    check('native_pdf_export_is_nonempty',pdf.stat().st_size>5000)
    check('export_snapshot_keeps_source_exact',source.read_bytes()==original+addition.encode())
    check('export_does_not_mark_document_dirty',report()['dirty'],False)
    act(['shot','07-export-complete'],['key','Escape'],['control','main-source-editor'],['key','ctrl+comma'],['key','Escape'])
    check('modal_escape_returns_editor_focus',any(x['name']=='main-source-editor' and x.get('focus') for x in report()['controls']))
    act(['close'])
    status='passed'
except Exception as error:
    failure=traceback.format_exc();print(failure,flush=True)
    try:act(['shot-root','failed-'+run_id])
    except Exception:pass
finally:
    result=dict(run_id=run_id,status=status,recorded_utc=datetime.now(timezone.utc).isoformat(),platform='Debian 12 / X11 dedicated Xvfb :141',candidate_sha256=hashlib.sha256((ROOT/candidate).read_bytes()).hexdigest(),checks=checks,failure=failure,source=str(source),source_sha256=hashlib.sha256(source.read_bytes()).hexdigest())
    (work/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2))
    print(json.dumps(dict(run_id=run_id,status=status,checks=len(checks),result=str(work/'result.json')),ensure_ascii=False),flush=True)
if status!='passed':sys.exit(1)
