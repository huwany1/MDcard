# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Reproduce the public Windows900-prefix/CtrlZ rendering-version failure on actual X11."""
from pathlib import Path
common=Path('/work/scripts/verify-reading-precision-linux.py').read_text()
exec(compile(common[:common.index('\ntry:\n    services.append')],'large-small-native-helpers','exec'))
WORK=OUT/'workspace';WORK.mkdir(exist_ok=True)
public=ROOT/'evidence/reading-precision/master/precision-b-final-r16-inapp-lifecycle-read/search.json'
original=json.loads(public.read_text(encoding='utf-8-sig'))['text']
assert hashlib.sha256(original.encode()).hexdigest()=='3b553b03812695acfabc2c9ce5ef53bc6ea933b3d2b8b8328a3cce093ca26161'
prefix='新增审核前言：文档身份与阅读位置分别记录，正文编辑不应把另一个视口拉回光标。\n\n'*900
raw=original.encode();heading=raw.index('## 操作章节 0009'.encode());expected=heading+3
(WORK/'ordinary.md').write_text(original)
try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    start('ordinary.md');mode('Split');settle('main/source');precise('main/preview')
    r=view('main/source')['viewport_rect'];click((r[0]+r[2])/2,(r[1]+r[3])/2);key('ctrl+Home')
    subprocess.run(['xclip','-selection','clipboard'],input=prefix.encode(),env=ENV,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);key('ctrl+v')
    wait(lambda s:s['main_revision']==1,'Actual paste revision')
    wait(lambda s:s['text']==prefix+original,'Actual large source','search');key('ctrl+s');wait(lambda s:not s['dirty'],'Actual source save','ui')
    control('navigation:Outline')
    # Real outline navigation reaches the exact same original heading glyph.
    # It is not a private position command; source keyboard focus remains active.
    old=expected+len(prefix.encode())
    for attempt in range(5):
        control('outline:'+str(len(prefix.encode())+heading))
        time.sleep(.7)
        before=precise('main/preview');g=before['geometry']
        if g['source_byte']==old:break
    assert g['source_byte']==old and g['source_text']=='操',(old,g)
    wait(lambda s:((s['views']['main/preview'].get('position')or{}).get('text_anchor')or{}).get('source',{}).get('proof') is not None and s['views']['main/preview'].get('proof_pending') is None,'Old actual heading proof')
    shot('01-large-before-native-undo');key('ctrl+z')
    wait(lambda s:s['main_revision']>=2,'Actual source undo revision');wait(lambda s:s['text']==original,'Actual small source after undo','search')
    selection=state('ui')['selection'];focus=state('ui')['focused'];after=precise('main/preview');g=after['geometry'];decision=after['relocation']
    assert g['source_byte']==expected and g['source_text']=='操' and g['text']=='操',(expected,g,decision)
    assert decision['exact'] and decision['new_source_byte']==expected,(expected,decision)
    assert abs(g['viewport_y']-before['geometry']['viewport_y'])<=2
    tail=[]
    for _ in range(22):
        v=precise('main/preview');q=v['geometry'];assert q['source_byte']==expected and q['text']==q['source_text']=='操';assert abs(q['viewport_y']-before['geometry']['viewport_y'])<=2
        tail.append(q);time.sleep(.1)
    assert max(q['compensations'] for q in tail)==min(q['compensations'] for q in tail)
    assert state('ui')['selection']==selection and state('ui')['focused']==focus
    shot('02-ordinary-after-native-undo');key('ctrl+s');wait(lambda s:not s['dirty'],'Original source saved','ui');assert (WORK/'ordinary.md').read_text()==original;stop()
    result=dict(passed=True,binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),source_before_bytes=len((prefix+original).encode()),source_after_bytes=len(original.encode()),old_source_byte=old,expected_source_byte=expected,before=before,after=after,tail=tail,stable_seconds=2.2,original_source_sha256=hashlib.sha256(original.encode()).hexdigest(),method='Public actual failed input; native900-prefix paste/save crosses ordinary->large, real outline navigation to same操heading, source CtrlZ crosseslarge->ordinary. Exact actual rendered glyph must match independent original byte and source text; no source-position injection.')
    (OUT/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k not in ['before','after','tail']},ensure_ascii=False))
except Exception as error:
    try:shot('failure')
    except Exception:pass
    (OUT/'failure.json').write_text(json.dumps(dict(error=repr(error)),ensure_ascii=False,indent=2));raise
finally:
    if app is not None and app.poll() is None:app.kill();app.wait(timeout=5)
    for service in reversed(services):
        service.terminate()
        try:service.wait(timeout=5)
        except subprocess.TimeoutExpired:service.kill();service.wait(timeout=5)
