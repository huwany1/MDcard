# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Native X11 changed-manuscript QA. Expected bytes come from explicit splices,
never from the matcher. This is implementation QA, separate from master corpus."""
from pathlib import Path
common=Path('/work/scripts/verify-reading-precision-linux.py').read_text()
exec(compile(common[:common.index('\ntry:\n    services.append')], 'native-precision-helpers', 'exec'))
RUNROOT=OUT

def fresh(name):
    global OUT,WORK,app,window
    OUT=RUNROOT/name;OUT.mkdir(exist_ok=True)
    WORK=OUT/'workspace';WORK.mkdir(exist_ok=True)
    for variable,file in [('MDCARD_SETTINGS_PATH','settings.json'),('MDCARD_READING_REPORT','reading.json'),('MDCARD_READING_STATE_PATH','reading-state.json'),('MDCARD_UI_REPORT','ui.json'),('MDCARD_SEARCH_REPORT','search.json')]:ENV[variable]=str(OUT/file)
    (OUT/'settings.json').write_text(json.dumps(dict(language='en',appearance='Light',reduce_motion=True,font_size=15)))
    app=window=None

def original(large,plain):
    sections=[]
    for i in range(900 if large else 80):
        body=f'Original paragraph identity {i:04}: 中文组合 e\u0301 👩🏽‍💻 unique original information record-{i:04} and explanatory source content. '
        sections.append(('' if plain else f'## Original section {i:04}\n\n')+body*3+'\n\n')
    return ''.join(sections)

def fast_wheel(name,notches):
    r=view(name)['viewport_rect'];run('xdotool','mousemove','--window',window,str(round(r[2]-35)),str(round((r[1]+r[3])/2)))
    run('xdotool','click','--repeat',str(abs(notches)),'--delay','3','5' if notches>0 else '4');time.sleep(.3)

def capture_middle(name,text):
    for _ in range(60):
        v=precise(name);g=v['geometry'];fraction=g['source_byte']/len(text.encode())
        if .42<=fraction<=.70:return v
        fast_wheel(name,90 if fraction<.42 else -30)
    raise AssertionError(('Native source middle not reached',fraction))

def transform(text,byte,kind,cluster=''):
    # All boundaries and expected offsets are tracked directly through edits.
    raw=text.encode();blocks=[];at=0
    for chunk in raw.splitlines(keepends=True):
        if chunk.strip():blocks.append((at,at+len(chunk)))
        at+=len(chunk)
    if kind=='prefix-insert':
        prefix=('A newly inserted independent introductory paragraph with new information.\n\n'*((len(raw)//180)+1)).encode()
        return (prefix+raw).decode(),byte+len(prefix),dict(kind=kind,inserted_bytes=len(prefix),old_bytes=len(raw),edited_ratio=len(prefix)/len(raw))
    if kind=='prefix-delete':
        end=next(b for a,b in blocks if b>=len(raw)*.30)
        assert byte>end
        return raw[end:].decode(),byte-end,dict(kind=kind,deleted_bytes=end,old_bytes=len(raw),edited_ratio=end/len(raw))
    if kind=='reorder-rename':
        end=next(b for a,b in blocks if b>=len(raw)*.33)
        assert byte>end
        moved=raw[end:]+raw[:end]
        old=b'Original section';new=b'Renamed chapter'
        before=moved[:byte-end];expected=byte-end+before.count(old)*(len(new)-len(old))
        return moved.replace(old,new).decode(),expected,dict(kind=kind,moved_bytes=end,old_bytes=len(raw),moved_ratio=end/len(raw))
    if kind=='local-rewrite':
        start=max(0,byte-75)
        while not (raw[start:start+1].decode('utf8','ignore')):start+=1
        stop=max(start,byte-24)
        while stop>start:
            try:raw[:stop].decode();break
            except UnicodeDecodeError:stop-=1
        replacement=b'A rewritten local explanatory phrase retaining the selected original cluster. '
        return (raw[:start]+replacement+raw[stop:]).decode(),byte+len(replacement)-(stop-start),dict(kind=kind,range=[start,stop],inserted_bytes=len(replacement))
    if kind=='split-merge':
        origin=len(raw[:byte].decode());next_block=text.find('\n\n',origin);after=min(len(text),next_block+6);before=max(text.rfind('\n\n',0,origin)+2,origin-24)
        merge=text.rfind('\n\n',0,max(0,origin-50));assert merge>=0
        edits=[(merge,merge+2,' '),(before,before,'\n\n'),(after,after,'\n\n')]
        changed=text
        for a,b,value in reversed(edits):changed=changed[:a]+value+changed[b:]
        expected=byte+sum(len(value.encode())-len(text[a:b].encode()) for a,b,value in edits if a<origin)
        return changed,expected,dict(kind=kind,independent_scalar_edits=edits,old_target_scalar=origin)
    if kind=='split-middle':
        origin=len(raw[:byte].decode());finish=origin+len(cluster);separator='\n\n' if '\n\n' in text else '\n'
        before=origin-24;after=finish+24;merge=text.rfind(separator,0,origin-50);assert merge>=0
        edits=[(merge,merge+len(separator),' '),(before,before,'\n\n'),(after,after,'\n\n')]
        changed=text
        for a,b,value in reversed(edits):changed=changed[:a]+value+changed[b:]
        expected=byte+sum(len(value.encode())-len(text[a:b].encode()) for a,b,value in edits if a<origin)
        return changed,expected,dict(kind=kind,logical_line_separator=repr(separator),independent_scalar_edits=edits,old_target_scalar=origin)
    if kind=='symmetric-copy':return text+'\n\n'+text,None,dict(kind=kind)
    if kind=='target-delete':
        start=raw.rfind(b'\n\n',0,byte)+2;end=raw.find(b'\n\n',byte)
        if end<0:end=len(raw)
        return (raw[:start]+raw[end:]).decode(),None,dict(kind=kind,deleted_range=[start,end])
    if kind=='empty':return '',None,dict(kind=kind)
    raise AssertionError(kind)

try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(RUNROOT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(RUNROOT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    specifications=[('md-prefix',False,False,'Read','prefix-insert'),('txt-delete',False,True,'Write','prefix-delete'),('large-md-move',True,False,'Read','reorder-rename'),('large-txt-prefix',True,True,'Write','prefix-insert'),('md-rewrite',False,False,'Read','local-rewrite'),('large-md-split-tail',True,False,'Write','split-merge'),('md-ambiguous',False,False,'Read','symmetric-copy'),('txt-deleted',False,True,'Write','target-delete'),('md-empty',False,False,'Read','empty')]
    specifications += [('md-split-middle',False,False,'Read','split-middle'),('txt-lines-split-middle',False,True,'Write','split-middle'),('large-md-split-middle',True,False,'Write','split-middle'),('large-txt-lines-split-middle',True,True,'Read','split-middle')]
    specifications.sort(key=lambda item:item[0]!='large-md-split-tail')
    selected=os.environ.get('MDCARD_RELOCATION_CASES')
    if selected:specifications=[item for item in specifications if item[0] in selected.split(',')]
    for name,large,plain,current_mode,kind in specifications:
        fresh(name);text=original(large,plain);filename='sample.txt' if plain else 'sample.md';file=WORK/filename;file.write_text(text)
        if plain and kind=='split-middle':text=text.replace('\n\n','\n');file.write_text(text)
        start(filename);mode(current_mode)
        if current_mode=='Write':control('source-wrap');control('source-wrap-on')
        vk='main/source' if current_mode=='Write' else 'main/preview'
        before=capture_middle(vk,text)
        if kind=='reorder-rename':
            # Positive rename oracle must retain its selected original text.
            # Real wheel chooses a body target; renamed heading letters are deletions.
            for _ in range(30):
                at=before['geometry']['source_byte'];raw=text.encode();line=raw[raw.rfind(b'\n',0,at)+1:raw.find(b'\n',at)]
                if not line.startswith(b'## '):break
                fast_wheel(vk,1);before=precise(vk)
            assert not line.startswith(b'## '),'No surviving body anchor selected'
        if kind=='split-merge':
            for _ in range(100):
                at=before['geometry']['source_byte'];raw=text.encode();end=raw.find(b'\n\n',at);line=raw[raw.rfind(b'\n',0,at)+1:raw.find(b'\n',at)]
                remaining=len(raw[at:end].decode()) if end>=at else 999
                if not line.startswith(b'## ') and 4<=remaining<=80:break
                fast_wheel(vk,1);before=precise(vk)
            assert 4<=remaining<=80 and not line.startswith(b'## '),'Real native late-paragraph anchor was not reached'
        old_byte=before['geometry']['source_byte'];old_y=before['geometry']['viewport_y'];old_grapheme=before['geometry']['source_text']
        wait(lambda s:s['views'][vk]['position']['text_anchor']['source'].get('proof') is not None and s['views'][vk].get('proof_pending') is None,'Background old-anchor evidence persisted')
        shot('01-before');stop()
        changed,expected,edit=transform(text,old_byte,kind,old_grapheme)
        if expected is not None:assert changed.encode()[expected:expected+len(old_grapheme.encode())]==old_grapheme.encode(), 'Independent edit oracle removed the selected grapheme'
        file.write_text(changed);(OUT/'old.txt').write_text(text);(OUT/'new.txt').write_text(changed)
        start(filename)
        ready=wait(lambda s:(s['views'].get(vk,{}).get('relocation')or{}).get('status')=='completed','Version decision completed')
        decision=ready['views'][vk]['relocation'];assert decision['exact']==(expected is not None),(kind,decision)
        if expected is not None:
            after=precise(vk);g=after['geometry'];assert g['source_byte']==expected,(kind,expected,g)
            assert g['source_text']==old_grapheme
            error=abs(g['viewport_y']-old_y);assert error<=2,(kind,error,g)
            assert not g.get('clamped'),(kind,'Unexpected clipping',g)
            first=g['compensations'];time.sleep(2.1);tail=view(vk);assert tail['geometry']['compensations']==first
        else:
            after=view(vk);error=None;assert decision['tier']!='text'
        shot('02-after');stop()
        assert file.read_text()==changed
        cases.append(dict(name=name,passed=True,edit=edit,old_byte=old_byte,expected_byte=expected,old_grapheme=old_grapheme,old_viewport_y=old_y,error_logical_px=error,relocation=decision,old_sha256=hashlib.sha256(text.encode()).hexdigest(),new_sha256=hashlib.sha256(changed.encode()).hexdigest()))
        (RUNROOT/'progress.json').write_text(json.dumps(cases,ensure_ascii=False,indent=2))
    (RUNROOT/'result.json').write_text(json.dumps(dict(passed=True,binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),cases=cases,platform='Debian12/Xvfb/Openbox/Mesa',note='Real native input, explicit independent edit provenance, actual current galley target and 2.1-second stability for exact cases. No corpus pass-rate claim.'),ensure_ascii=False,indent=2))
except Exception as error:
    try:shot('failure')
    except Exception:pass
    (RUNROOT/'failure.json').write_text(json.dumps(dict(error=repr(error),cases=cases),ensure_ascii=False,indent=2));raise
finally:
    if app is not None and app.poll() is None:app.kill();app.wait(timeout=5)
    for p in reversed(services):
        p.terminate()
        try:p.wait(timeout=5)
        except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)
