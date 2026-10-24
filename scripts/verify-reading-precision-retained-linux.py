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
OUT = ROOT / 'evidence/reading-precision/implementation' / os.environ.get('MDCARD_READING_RUN', 'stage-a-retained-r6')
OUT.mkdir(parents=True, exist_ok=True)
EXE = Path(os.environ.get('MDCARD_READING_EXE', '/work/artifacts/reading-precision/stage-a-r9-linux/mdcard'))
RENDERER = Path(os.environ.get('MDCARD_READING_RENDERER_DIR', str(EXE.parent/'renderer' if (EXE.parent/'renderer').is_dir() else ROOT/'dist/linux/renderer')))
WORK = ROOT / 'artifacts/reading-state' / OUT.name / 'workspace'
subprocess.run(['python3', str(ROOT/'scripts/generate-reading-state-master-fixtures.py'), '--output', str(WORK)], check=True)
ENV = dict(os.environ, DISPLAY=':151', LANG='C.UTF-8', LIBGL_ALWAYS_SOFTWARE='1', LP_NUM_THREADS='2',
           MDCARD_RENDERER_DIR=str(RENDERER), MDCARD_SETTINGS_PATH=str(OUT/'settings.json'),
           MDCARD_READING_REPORT=str(OUT/'reading.json'), MDCARD_READING_STATE_PATH=str(OUT/'reading-state.json'),
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
    for _ in range(40):
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

def start(filename='a.md'):
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

try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT))
    time.sleep(.5)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT))
    time.sleep(.6)
    start()
    mode('Read')
    settle('main/preview')
    wheel('main/preview',34)
    furthest=settle('main/preview')
    wheel('main/preview',-18)
    before=settle('main/preview')
    original_id=state()['main_id']
    quick('b.md');mode('Read');wheel('main/preview',20);b=settle('main/preview')
    quick('a.md');mode('Read');after=settle('main/preview')
    compare('A last position after backward scroll',before,after)
    assert state()['main_id']==original_id
    r=next(r for r in state()['records'] if r['id']==original_id)
    assert not r['read'] and r['furthest']>=furthest['anchor']['approximate']>after['anchor']['approximate']
    shot('01-last-position-independent-of-furthest')
    run('xdotool','windowsize',window,'900','820')
    narrow=settle('main/preview')
    assert narrow['anchor']['byte']==after['anchor']['byte'], ('width changed block',after,narrow)
    cases.append(dict(name='width preserves source block',status='通过',source_byte=narrow['anchor']['byte']))
    shot('02-narrow-same-content-block')
    run('xdotool','windowsize',window,'1240','820')
    settle('main/preview')
    for width in [900,1240,900,1240]:
        run('xdotool','windowsize',window,str(width),'820')
        current=settle('main/preview')
        assert current['anchor']['byte']==after['anchor']['byte'], ('Repeated resize drifted block',width,after,current)
    cases.append(dict(name='repeated width reflow uses current layout rather than stale position',status='通过',source_byte=current['anchor']['byte']))
    before_restart=settle('main/preview')
    stop();start();mode('Read')
    compare('normal restart',before_restart,settle('main/preview'))
    quick('b.md');mode('Read');compare('B restart independent',b,settle('main/preview'))
    for filename in ['large.md','plain.txt','large-plain.txt','utf16.txt','gb18030.txt','mapping.md','mapping-large.md','long-paragraph.md']:
        quick(filename);mode('Read');settle('main/preview');wheel('main/preview',20)
        old=settle('main/preview');quick('empty.md');mode('Read');empty=settle('main/preview');assert empty['offset']==[0,0]
        quick(filename);mode('Read');compare('switch '+filename,old,settle('main/preview'))
        shot('03-'+filename.replace('.','-'))
    quick('large.md');mode('Read');wheel('main/preview',40);large=settle('main/preview');stop();start('large.md');mode('Read');compare('large restart',large,settle('main/preview'))
    shot('04-large-restart')
    stop()
    result=dict(verification_id=str(uuid.uuid4()),passed=True,platform='Debian 12 / X11 / Xvfb / Openbox / Mesa software OpenGL',binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),cases=cases,snapshots=snapshots)
    (OUT/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(result,ensure_ascii=False,indent=2))
except Exception as error:
    failure=dict(passed=False,error=repr(error),cases=cases,binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest())
    if window and app and app.poll() is None:
        try:shot('failure')
        except Exception:pass
    (OUT/'failure.json').write_text(json.dumps(failure,ensure_ascii=False,indent=2)+'\n')
    raise
finally:
    if app and app.poll() is None:
        app.kill();app.wait(timeout=5)
    for process in reversed(services):
        process.terminate()
        try:process.wait(timeout=5)
        except subprocess.TimeoutExpired:process.kill();process.wait(timeout=5)
