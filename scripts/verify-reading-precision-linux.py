# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Real X11 mouse/keyboard acceptance; reading JSON is observation only."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
import uuid

ROOT = Path('/work')
OUT = ROOT / 'evidence/reading-precision/implementation' / os.environ.get('MDCARD_READING_RUN', 'stage-a-r1')
OUT.mkdir(parents=True, exist_ok=True)
EXE = Path(os.environ.get('MDCARD_READING_EXE', '/work/artifacts/reading-precision/stage-a-r1-linux/mdcard'))
RENDERER = Path(os.environ.get('MDCARD_READING_RENDERER_DIR', str(EXE.parent/'renderer' if (EXE.parent/'renderer').is_dir() else ROOT/'dist/linux/renderer')))
WORK = ROOT/'artifacts/reading-precision/performance-inputs'
ENV = dict(os.environ, DISPLAY=':155', LANG='C.UTF-8', LIBGL_ALWAYS_SOFTWARE='1', LP_NUM_THREADS='2',
           MDCARD_RENDERER_DIR=str(RENDERER), MDCARD_SETTINGS_PATH=str(OUT/'settings.json'),
           MDCARD_READING_GEOMETRY_REPORT='1', MDCARD_READING_REPORT=str(OUT/'reading.json'), MDCARD_READING_STATE_PATH=str(OUT/'reading-state.json'),
           MDCARD_UI_REPORT=str(OUT/'ui.json'), MDCARD_SEARCH_REPORT=str(OUT/'search.json'),
           XDG_RUNTIME_DIR='/tmp/mdcard-reading-state-x11')
Path(ENV['XDG_RUNTIME_DIR']).mkdir(exist_ok=True)
Path(ENV['XDG_RUNTIME_DIR']).chmod(0o700)
(OUT/'settings.json').write_text(json.dumps(dict(language='en', appearance='Light', reduce_motion=True, font_size=15)))
services, cases, snapshots = [], [], []
app = window = None

def run(*args, **kwargs):
    return subprocess.run(args, env=ENV, check=True, **kwargs)

def state(name='reading'):
    for _ in range(160):
        try:
            return json.loads((OUT/(name+'.json')).read_text())
        except (OSError, ValueError):
            time.sleep(.03)
    raise AssertionError('No complete '+name+' observation')

def wait(predicate, label, name='reading'):
    until = time.monotonic()+20
    while time.monotonic() < until:
        assert app.poll() is None, ('Application exited', app.returncode)
        current = state(name)
        if predicate(current):
            return current
        time.sleep(.06)
    raise AssertionError(label)

def key(*keys):
    run('xdotool', 'key', '--clearmodifiers', *keys)
    time.sleep(.12)

def click(x, y):
    run('xdotool', 'mousemove', '--window', window, str(round(x)), str(round(y)))
    run('xdotool', 'click', '1')
    time.sleep(.25)

def control(name, report='ui'):
    current = wait(lambda s:any(c['name']==name for c in s['controls']), 'Missing '+name, report)
    c = next(c for c in reversed(current['controls']) if c['name']==name)
    r = c['rect']
    click((r[0]+r[2])/2, (r[1]+r[3])/2)

def mode(name):
    control(name)
    wait(lambda s:s['mode']==name, 'Mode '+name, 'ui')
    time.sleep(.4)

def view(name):
    return state()['views'][name]

def settle(name):
    def ready(s):
        v=s['views'].get(name,{})
        p=v.get('position');r=v.get('viewport_rect')
        return p is not None and r is not None and v['pending'] is None and abs(p['viewport'][0]-(r[2]-r[0]))<.75 and abs(p['viewport'][1]-(r[3]-r[1]))<.75
    wait(ready, 'Restore with matching current viewport '+name)
    time.sleep(.35)
    return view(name)['position']

def wheel(name, notches):
    r = view(name)['viewport_rect']
    assert r and len(r)==4
    run('xdotool', 'mousemove', '--window', window, str(round((r[0]+r[2])/2)), str(round((r[1]+r[3])/2)))
    run('xdotool', 'click', '--repeat', str(abs(notches)), '--delay', '65', '5' if notches>0 else '4')
    time.sleep(.7)

def quick(filename, secondary=False):
    key('ctrl+p')
    control('quick-query', 'search')
    key('ctrl+a')
    run('xdotool', 'type', '--clearmodifiers', '--delay', '20', filename)
    wait(lambda s:not s.get('query_pending',True) and s.get('quick') and Path(s['quick'][0]['path'])==WORK/filename, 'Quick '+filename, 'search')
    key('shift+Return' if secondary else 'Return')
    wait(lambda s:not s['busy'] and not s['quick_open'], 'Open '+filename, 'search')
    time.sleep(.3)

def shot(name):
    if app is None or app.poll() is not None or window is None:
        raise AssertionError('Screenshot skipped: the owned application window is closed')
    time.sleep(.2)
    run('import', '-window', window, str(OUT/(name+'.png')), timeout=8)
    (OUT/(name+'.json')).write_text(json.dumps(state(), ensure_ascii=False, indent=2))
    snapshots.append(name)

def start(filename='ordinary.md'):
    global app, window
    app = subprocess.Popen([str(EXE), str(WORK/filename)], env=ENV, stdout=(OUT/'app.log').open('ab'), stderr=subprocess.STDOUT)
    window = None
    for _ in range(100):
        time.sleep(.1)
        found = subprocess.run(['xdotool','search','--all','--pid',str(app.pid),'--name','MDcard'],env=ENV,capture_output=True)
        if found.returncode==0:
            window=found.stdout.decode().splitlines()[0]
            break
    assert window, 'No X11 window'
    run('xdotool','windowsize',window,'1240','820','windowactivate','--sync',window)
    wait(lambda s:s.get('main_id') is not None, 'Reading report')
    time.sleep(1)

def stop():
    key('ctrl+q')
    app.wait(timeout=8)
    assert app.returncode==0

def compare(name,before,after,anchor=True):
    error=[abs(before['offset'][i]-after['offset'][i]) for i in range(2)]
    assert max(error)<=2, (name,'offset error',error)
    if anchor:
        assert before['anchor']['byte']==after['anchor']['byte'], (name,'block mismatch',before['anchor'],after['anchor'])
    cases.append(dict(name=name,status='通过',error_logical_px=error,anchor_before=before['anchor']['byte'],anchor_after=after['anchor']['byte']))


def resize_precise(name,width):
    old=view(name);frame=old['geometry']['frame'];r=old['viewport_rect'];old_width=r[2]-r[0]
    run('xdotool','windowsize',window,str(width),'820')
    wait(lambda s:s['views'].get(name,{}).get('geometry') is not None and s['views'][name]['geometry']['frame']>frame+2 and abs((s['views'][name]['viewport_rect'][2]-s['views'][name]['viewport_rect'][0])-old_width)>100,'Actual new viewport '+str(width))
    return precise(name)

def precise(name):
    def ready(s):
        v=s['views'].get(name,{})
        p=v.get('position');r=v.get('viewport_rect');g=v.get('geometry')
        return p is not None and r is not None and g is not None and v['pending'] is None and abs(p['viewport'][0]-(r[2]-r[0]))<.75 and p.get('text_anchor') is not None and abs(g['viewport_y']-p['text_anchor']['viewport_y'])<=2
    wait(ready,'Actual precise glyph ready '+name)
    time.sleep(.3)
    current=view(name);duration=float(os.environ.get('MDCARD_PRECISION_STABILITY_SECONDS','0'))
    if duration:
        original=current['position']['text_anchor'];byte=original['source']['source_byte'];y=original['viewport_y'];samples=[]
        for _ in range(round(duration/.1)):
            now=view(name);g=now.get('geometry');assert g and g['source_byte']==byte
            samples.append(dict(frame=g['frame'],byte=g['source_byte'],viewport_y=g['viewport_y'],compensations=g['compensations']));time.sleep(.1)
        assert max(abs(v['viewport_y']-y) for v in samples)<=2,('Actual stable glyph drift',samples)
        assert samples[0]['compensations']==samples[-1]['compensations'],('Compensation feedback',samples)
        cases.append(dict(name=name+' stable glyph tail',passed=True,duration_seconds=duration,source_byte=byte,maximum_viewport_y_error=max(abs(v['viewport_y']-y) for v in samples),compensations=samples[-1]['compensations'],samples=samples))
    return view(name)

def font_precise(name):
    old=view(name)['position']['layout'];key('ctrl+comma');current=wait(lambda s:any(c['name']=='reading-font-size' for c in s['controls']),'Font slider','ui')
    time.sleep(1);current=state('ui')
    rect=next(c['rect'] for c in reversed(current['controls']) if c['name']=='reading-font-size')
    preferences=json.loads((OUT/'settings.json').read_text());actual_x=rect[0]+(25 if preferences['font_size']>=19 else 90);actual_y=(rect[1]+rect[3])/2
    # Hold across real UI frames. An instantaneous XTest click can complete
    # between egui frames and never activate this drag-based Slider.
    run('xdotool','mousemove','--window',window,str(round(actual_x)),str(round(actual_y)),'mousedown','1');time.sleep(.6)
    pressed=state('ui');shot('font-modal-'+name.replace('/','-'));run('xdotool','mouseup','1');time.sleep(.3)
    (OUT/('font-track-'+name.replace('/','-')+'.json')).write_text(json.dumps(dict(reported_rect=rect,native_click=[actual_x,actual_y],pressed_control=next(c for c in reversed(pressed['controls']) if c['name']=='reading-font-size'),settings_before=preferences,settings_after=json.loads((OUT/'settings.json').read_text()),note='Read-only response, then native mouse press held for .6 seconds. No state injection.'),indent=2))
    key('Escape')
    wait(lambda s:s['views'].get(name,{}).get('position') is not None and s['views'][name]['position']['layout']!=old,'Actual font layout changed')
    return precise(name)

def advance_fraction(name):
    target=float(os.environ.get('MDCARD_PRECISION_TARGET_FRACTION','0'))
    if not target:return
    file=WORK/os.environ.get('MDCARD_PRECISION_FILE','ordinary.md');raw=file.read_text();length=len(raw.encode());start=len(raw.split('\n\n',1)[0].encode())+2;end=raw.rfind('\n\n## Tail');end=len(raw[:end].encode()) if end>=0 else length
    steps=[]
    for _ in range(150):
        v=view(name);g=v.get('geometry');fraction=(g['source_byte']-start)/max(end-start,1) if g else 0
        if fraction>=target:
            cases.append(dict(name=name+' actual original long paragraph position',passed=True,source_byte=g['source_byte'],original_block_range=[start,end],fraction_within_original_block=fraction,target_fraction=target,manuscript_sha256=hashlib.sha256(file.read_bytes()).hexdigest(),native_steps=steps));return
        r=v['viewport_rect'];run('xdotool','mousemove','--window',window,str(round((r[0]+r[2])/2)),str(round((r[1]+r[3])/2)),'click','--repeat','100','--delay','1','5');time.sleep(.25)
        after=view(name);steps.append(dict(source_byte=(after.get('geometry') or {}).get('source_byte'),offset=after.get('position',{}).get('offset')))
    raise AssertionError('Native wheel did not reach requested original-paragraph fraction')

try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    start(os.environ.get('MDCARD_PRECISION_FILE','ordinary.md'))
    if os.environ.get('MDCARD_PRECISION_SOURCE_ONLY')!='1':
        mode('Read');precise('main/preview');wheel('main/preview',18)
        assert view('main/preview')['position']['offset'][1]>100,'Expected real preview scroll'
        advance_fraction('main/preview')
        before=precise('main/preview');shot('01-preview-before')
        byte=before['position']['text_anchor']['source']['source_byte'];y=before['geometry']['viewport_y']
        for i,width in enumerate([850,1240,850,1240]):
            after=resize_precise('main/preview',width)
            assert after['geometry']['source_byte']==byte,('Stable original grapheme changed',byte,after['geometry'])
            error=abs(after['geometry']['viewport_y']-y)
            assert error<=2,('Actual viewport y',error)
            cases.append(dict(name='preview width '+str(width),passed=True,source_byte=byte,viewport_y_error=error))
            shot('02-width-'+str(i))
        if os.environ.get('MDCARD_PRECISION_FONT')=='1':
            after=font_precise('main/preview');assert after['geometry']['source_byte']==byte
            error=abs(after['geometry']['viewport_y']-y);assert error<=2
            cases.append(dict(name='preview actual font change',passed=True,source_byte=byte,viewport_y_error=error));shot('02-preview-font')
        if os.environ.get('MDCARD_PRECISION_SPLIT')=='1':
            for current_mode in ['Split','Read']:
                mode(current_mode);after=precise('main/preview');assert after['geometry']['source_byte']==byte
                error=abs(after['geometry']['viewport_y']-y);assert error<=2
                cases.append(dict(name='preview '+current_mode,passed=True,source_byte=byte,viewport_y_error=error));shot('02-preview-mode-'+current_mode)
    mode('Write')
    if os.environ.get('MDCARD_PRECISION_WRAP')=='on':
        control('source-wrap');control('source-wrap-on')
        wait(lambda s:s['views']['main/source'].get('position') is not None and s['views']['main/source']['position']['layout'].endswith(':true'),'Source wrapping on')
    precise('main/source');wheel('main/source',14)
    assert view('main/source')['position']['offset'][1]>100,'Expected real source scroll'
    advance_fraction('main/source')
    before=precise('main/source');shot('03-source-before');byte=before['geometry']['source_byte'];y=before['geometry']['viewport_y']
    for i,width in enumerate([850,1240]):
        after=resize_precise('main/source',width)
        assert after['geometry']['source_byte']==byte
        error=abs(after['geometry']['viewport_y']-y);assert error<=2
        cases.append(dict(name='source width '+str(width),passed=True,source_byte=byte,viewport_y_error=error));shot('04-source-width-'+str(i))
    if os.environ.get('MDCARD_PRECISION_FONT')=='1':
        after=font_precise('main/source');assert after['geometry']['source_byte']==byte
        error=abs(after['geometry']['viewport_y']-y);assert error<=2
        cases.append(dict(name='source actual font change',passed=True,source_byte=byte,viewport_y_error=error));shot('04-source-font')
    if os.environ.get('MDCARD_PRECISION_SPLIT')=='1':
        for current_mode in ['Split','Write']:
            mode(current_mode);after=precise('main/source');assert after['geometry']['source_byte']==byte
            error=abs(after['geometry']['viewport_y']-y);assert error<=2
            cases.append(dict(name='source '+current_mode,passed=True,source_byte=byte,viewport_y_error=error));shot('04-source-mode-'+current_mode)
    if os.environ.get('MDCARD_PRECISION_SIDEBAR')=='1':
        for shown in [False,True,False,True]:
            frame=view('main/source')['geometry']['frame'];old_width=view('main/source')['viewport_rect'][2]-view('main/source')['viewport_rect'][0]
            key('ctrl+b')
            wait(lambda s:s['views'].get('main/source',{}).get('geometry') is not None and s['views']['main/source']['geometry']['frame']>frame+2 and abs((s['views']['main/source']['viewport_rect'][2]-s['views']['main/source']['viewport_rect'][0])-old_width)>100,'Actual sidebar viewport')
            after=precise('main/source');assert after['geometry']['source_byte']==byte
            error=abs(after['geometry']['viewport_y']-y);assert error<=2
            cases.append(dict(name='source sidebar '+str(shown),passed=True,source_byte=byte,viewport_y_error=error));shot('05-source-sidebar-'+str(len(cases)))
    stop()
    result=dict(passed=True,binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),cases=cases,snapshots=snapshots,platform='Debian12/Xvfb/Openbox/Mesa',note='Actual native mouse/keyboard and current-pass galley geometry. Headless drawing tests are reported separately.')
    (OUT/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2));print(json.dumps(result,indent=2))
except Exception as error:
    try:shot('failure')
    except Exception:pass
    (OUT/'failure.json').write_text(json.dumps(dict(error=repr(error),cases=cases),ensure_ascii=False,indent=2));raise
finally:
    if app is not None and app.poll() is None:app.kill();app.wait(timeout=5)
    for p in reversed(services):
        p.terminate()
        try:p.wait(timeout=5)
        except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)
