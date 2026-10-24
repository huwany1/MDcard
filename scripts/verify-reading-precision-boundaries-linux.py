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
WORK = OUT
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

import struct,zlib
def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data)&0xffffffff)
width,height=240,3200
png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,2,0,0,0))+chunk(b'IDAT',zlib.compress((b'\0'+bytes([48,112,220])*width)*height))+chunk(b'IEND',b'')
(WORK/'tall.png').write_bytes(png)
file=WORK/'nontext.md';file.write_text('# Tall image\n\n![tall](tall.png)\n\n## After image\n\n'+'Body after tall image. 中文🙂\n\n'*40)
digest=hashlib.sha256(file.read_bytes()).hexdigest()
try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    start('nontext.md');mode('Read');settle('main/preview')
    wheel('main/preview',20)
    before=settle('main/preview');assert before['offset'][1]>1200,before
    assert before['text_anchor'] is None,('Image-only viewport must be block-level',before)
    assert view('main/preview')['geometry'] is None
    shot('01-image-only-before');stop()
    start('nontext.md');mode('Read');after=settle('main/preview')
    error=[abs(x-y) for x,y in zip(before['offset'],after['offset'])];assert max(error)<=2,('Image restart error',error,before,after)
    assert after['text_anchor'] is None,('Temporary top text overwrote image history',after)
    assert view('main/preview')['geometry'] is None
    samples=[]
    for _ in range(25):
        v=view('main/preview');samples.append(dict(offset=v['position']['offset'],pending=v['pending'],text_anchor=v['position']['text_anchor'],layout_pending=v['layout_pending']));time.sleep(.1)
    assert all(s['text_anchor'] is None and max(abs(x-y) for x,y in zip(s['offset'],before['offset']))<=2 for s in samples)
    shot('02-image-only-restart');stop();assert hashlib.sha256(file.read_bytes()).hexdigest()==digest
    result=dict(passed=True,binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),platform='Debian12/Xvfb/Openbox/Mesa',case='Real 240x3200 local PNG image-only viewport normal restart',before=before,after=after,error_logical_px=error,stability_seconds=2.5,samples=samples,manuscript_hash_unchanged=True,snapshots=snapshots)
    (OUT/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2));print(json.dumps(dict(passed=True,error_logical_px=error)))
except Exception as error:
    try:shot('failure')
    except Exception:pass
    (OUT/'failure.json').write_text(json.dumps(dict(error=repr(error)),ensure_ascii=False,indent=2));raise
finally:
    if app is not None and app.poll() is None:app.kill();app.wait(timeout=5)
    for p in reversed(services):
        p.terminate()
        try:p.wait(timeout=5)
        except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)
