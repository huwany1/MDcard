# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Native ordinary-secondary exact-geometry verification (or frozen pre-fix reproduction)."""
from pathlib import Path
common=Path('/work/scripts/verify-reading-precision-linux.py').read_text()
exec(compile(common[:common.index('\ntry:\n    services.append')],'secondary-native-helpers','exec'))
WORK=OUT/'workspace';WORK.mkdir(exist_ok=True)
text='# Secondary source\n\n'+''.join(f'SIDE-{i:04} 中文 é 👩🏽‍💻 independently marked source row with more readable text.\n' for i in range(220))
assert len(text.encode())<64*1024
(WORK/'ordinary.md').write_text('# Primary source\n\nIndependent primary buffer')
(WORK/'secondary.md').write_text(text)
(WORK/'other.md').write_text('# Other secondary\n\nIndependent text')
reproduce=os.environ.get('MDCARD_SECONDARY_REPRODUCE')=='1'

def snap(name):
    shot(name);return state()

try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    start('ordinary.md');mode('Write');quick('secondary.md',True);settle('secondary/source')
    run('xdotool','windowsize',window,'1440','820');time.sleep(.7)
    r=view('secondary/source')['viewport_rect'];click((r[0]+r[2])/2,(r[1]+r[3])/2);key('ctrl+Home');key('shift+End')
    expected_selection=[len(text.splitlines()[0]),0]
    def selected(s):return next(record for record in s['records'] if record['id']==s['secondary_id'])['selections']['secondary/source']
    assert selected(state())==expected_selection
    wheel('secondary/source',18);settle('secondary/source');before=view('secondary/source');assert before['position']['offset'][1]>500
    source_hash=hashlib.sha256((WORK/'secondary.md').read_bytes()).hexdigest();snap('01-secondary-scrolled')
    rect=before['viewport_rect'];x=round(rect[0]-16);y=round((rect[1]+rect[3])/2)
    run('xdotool','mousemove','--window',window,str(x),str(y),'mousedown','1');time.sleep(.12)
    run('xdotool','mousemove','--window',window,str(x-120),str(y));time.sleep(.2);run('xdotool','mouseup','1');time.sleep(.8)
    settle('secondary/source');after=view('secondary/source');snap('02-secondary-ratio')
    assert abs((after['viewport_rect'][2]-after['viewport_rect'][0])-(rect[2]-rect[0]))>50,('Native panel resize did not change actual source width',rect,after['viewport_rect'])
    if reproduce:
        assert before.get('geometry') is None and before['position'].get('text_anchor') is None
        assert after.get('geometry') is None and after['position'].get('text_anchor') is None
        result=dict(passed=False,confirmed_known_gap=True,binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),before=before,after=after,source_sha256=source_hash,source_bytes=len(text.encode()),reason='Ordinary secondary source captures only coarse anchor/offset, no original grapheme pin or actual Galley geometry before/after real ratio resize. Coarse offsets are not precision evidence.')
    else:
        original=before['geometry'];expected=original['source_byte'];wanted=original['viewport_y'];tail=[]
        for i in range(22):
            current=precise('secondary/source');g=current['geometry'];assert g['source_byte']==expected,(expected,g);assert abs(g['viewport_y']-wanted)<=2
            assert text.encode()[g['source_byte']:g['source_end']].decode()==g['source_text'];tail.append(dict(source_byte=g['source_byte'],viewport_y=g['viewport_y'],compensations=g['compensations']));time.sleep(.1)
        assert max(x['compensations'] for x in tail)==min(x['compensations'] for x in tail)
        def verify_current(label):
            current=precise('secondary/source');g=current['geometry'];assert g['source_byte']==expected and abs(g['viewport_y']-wanted)<=2,(label,g)
            assert selected(state())==expected_selection,(label,selected(state()),expected_selection)
            return dict(label=label,actual_geometry=g,selection=selected(state()))
        selection_checks=[verify_current('ratio after far viewport with head selection')]
        for wrap in ['off','on']:
            control('source-wrap');control('source-wrap-'+wrap);time.sleep(.7);selection_checks.append(verify_current('unfocused toolbar wrap '+wrap))
        quick('other.md',True);settle('secondary/source');quick('secondary.md',True);selection_checks.append(verify_current('secondary other then original'))
        stop();start('ordinary.md');mode('Write');quick('secondary.md',True);selection_checks.append(verify_current('normal restart secondary'))
        result=dict(passed=True,selection_checks=selection_checks,expected_selection=expected_selection,binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),before=before,after=after,tail=tail,source_sha256=source_hash,source_bytes=len(text.encode()),maximum_viewport_y_error=max(abs(v['viewport_y']-wanted) for v in tail),method='Native Quick secondary, actual wheel and pane-ratio drag; exact same original grapheme, current Galley geometry and stable tail. No position injection.')
    assert hashlib.sha256((WORK/'secondary.md').read_bytes()).hexdigest()==source_hash
    stop();(OUT/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k not in ['before','after','tail']},ensure_ascii=False))
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
