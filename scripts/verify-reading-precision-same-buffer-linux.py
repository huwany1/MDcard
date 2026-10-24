# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Actual same-file/equal-revision buffers independently match original source glyphs."""
from pathlib import Path
common=Path('/work/scripts/verify-reading-precision-linux.py').read_text()
exec(compile(common[:common.index('\ntry:\n    services.append')],'same-buffer-native-helpers','exec'))
WORK=OUT/'workspace';WORK.mkdir(exist_ok=True)
original='# Shared source\n\n'+''.join(f'SHARED-{i:04} 中文 é 👩🏽‍💻 separate immutable row context {i:04} content.\n' for i in range(220))
assert len(original.encode())<64*1024
(WORK/'ordinary.md').write_text('# Initial main\n\nTemporary source')
(WORK/'shared.md').write_text(original)
original_hash=hashlib.sha256((WORK/'shared.md').read_bytes()).hexdigest()
measurements=[]

def proof_ready(s,key):
    v=s['views'].get(key,{});a=((v.get('position') or {}).get('text_anchor') or {}).get('source') or {}
    return a.get('proof') is not None and v.get('proof_pending') is None and v.get('pending') is None

def paste(value):
    subprocess.run(['xclip','-selection','clipboard'],input=value.encode(),env=ENV,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    key('ctrl+v')

def prefix_main_match():
    source='main/source';target='main/preview'
    rect=view(source)['viewport_rect'];click((rect[0]+rect[2])/2,(rect[1]+rect[3])/2);key('ctrl+Home')
    wheel(target,16);before=precise(target);wait(lambda s:proof_ready(s,target),'Actual main preview old evidence')
    old=before['geometry'];prefix='主🙂';expected=old['source_byte']+len(prefix.encode());wanted=old['viewport_y']
    paste(prefix)
    wait(lambda s:s['main_revision']==1 and s['views'][target].get('relocation') is not None and s['views'][target]['relocation'].get('status')=='completed','Main actual preview version match completed')
    after=precise(target);g=after['geometry'];decision=after['relocation']
    assert decision['exact'] and decision['new_source_byte']==expected,(expected,decision)
    assert g['source_byte']==expected and abs(g['viewport_y']-wanted)<=2,(expected,g)
    measurements.append(dict(view=target,old_source_byte=old['source_byte'],expected_source_byte=expected,actual_geometry=g,old_viewport_y=wanted,decision=decision))
    shot('matched-main-preview');return prefix+original,expected,wanted

def prefix_secondary_capture():
    target='secondary/source';rect=view(target)['viewport_rect'];click((rect[0]+rect[2])/2,(rect[1]+rect[3])/2);key('ctrl+Home');paste('S')
    wait(lambda s:s['secondary_revision']==1,'Actual secondary independent source revision')
    wheel(target,20);after=precise(target);wait(lambda s:proof_ready(s,target),'Secondary actual new-content proof ready')
    g=after['geometry'];measurements.append(dict(view=target,actual_geometry=g,expected_source_byte=g['source_byte'],old_viewport_y=g['viewport_y'],decision=None))
    shot('secondary-own-proof');return 'S'+original,g['source_byte'],g['viewport_y']

try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','2400x1400x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    start('ordinary.md');mode('Write');quick('shared.md',True);settle('secondary/source')
    control('tree:shared.md');wait(lambda s:s['main_id']==s['secondary_id'],'Native tree produced same stable document in both buffers');settle('main/source');settle('secondary/source')
    run('xdotool','windowsize',window,'1920','820');time.sleep(.6);mode('Split');precise('main/preview')
    main,main_byte,main_y=prefix_main_match()
    side,side_byte,side_y=prefix_secondary_capture()
    reading=state();assert reading['main_revision']==reading['secondary_revision']==1
    assert reading['main_id']==reading['secondary_id'] and reading['main_buffer_id']!=reading['secondary_buffer_id']
    buffers=state('search');assert buffers['text']==main and buffers['secondary']['text']==side
    assert view('main/preview')['position']['fingerprint']!=view('secondary/source')['position']['fingerprint']
    font_precise('main/preview');precise('secondary/source')
    for _ in range(22):
        s=state()
        for key_name,expected,y,text in [('main/preview',main_byte,main_y,main),('secondary/source',side_byte,side_y,side)]:
            v=s['views'][key_name];g=v['geometry'];assert g['source_byte']==expected and abs(g['viewport_y']-y)<=2
            assert text.encode()[g['source_byte']:g['source_end']].decode()==g['source_text']
            source=v['position']['text_anchor']['source'];assert source['source_fingerprint']==v['position']['fingerprint']
            if key_name=='main/preview':assert v['relocation']['metadata']['buffer_id']==s['main_buffer_id']
            assert v['buffer_id']==s['main_buffer_id' if key_name=='main/preview' else 'secondary_buffer_id']
        time.sleep(.1)
    shot('both-equal-revision-independent-glyphs')
    # Remove only each fixture prefix through real source input; never save dirty buffers.
    for key_name,count in [('secondary/source',1),('main/source',2)]:
        r=view(key_name)['viewport_rect'];click((r[0]+r[2])/2,(r[1]+r[3])/2);key('ctrl+Home')
        key(*(['shift+Right']*count));key('Delete')
    wait(lambda s:s['text']==original and s['secondary']['text']==original and not s['secondary']['dirty'],'Actual prefix removal restores original buffers','search')
    wait(lambda s:not s['dirty'],'Main cached clean after original restored','ui')
    assert hashlib.sha256((WORK/'shared.md').read_bytes()).hexdigest()==original_hash
    stop()
    result=dict(passed=True,binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),document_id=reading['main_id'],main_buffer_id=reading['main_buffer_id'],secondary_buffer_id=reading['secondary_buffer_id'],equal_revisions=[1,1],cases=measurements,stability_seconds=2.2,original_disk_sha256=original_hash,method='Native Quick secondary then real tree same-file main; both real head-prefix pastes each create revision1; main preview completes version match, secondary captures and enriches its own independently edited source. Real font reflow preserves each original Galley glyph/y; secondary source direct editing intentionally follows its caret and is not counted as a completed version match. Cleanup uses native removal; original disk unchanged.')
    (OUT/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k!='cases'},ensure_ascii=False))
except Exception as error:
    try:shot('failure')
    except Exception:pass
    (OUT/'failure.json').write_text(json.dumps(dict(error=repr(error),cases=measurements),ensure_ascii=False,indent=2));raise
finally:
    if app is not None and app.poll() is None:app.kill();app.wait(timeout=5)
    for service in reversed(services):
        service.terminate()
        try:service.wait(timeout=5)
        except subprocess.TimeoutExpired:service.kill();service.wait(timeout=5)
