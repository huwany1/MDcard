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

import ctypes,ctypes.util
ENV.pop('MDCARD_READING_GEOMETRY_REPORT',None)
lib=ctypes.CDLL(ctypes.util.find_library('X11'));xtst=ctypes.CDLL(ctypes.util.find_library('Xtst'))
lib.XOpenDisplay.argtypes=[ctypes.c_char_p];lib.XOpenDisplay.restype=ctypes.c_void_p
lib.XResizeWindow.argtypes=[ctypes.c_void_p,ctypes.c_ulong,ctypes.c_uint,ctypes.c_uint]
lib.XFlush.argtypes=[ctypes.c_void_p];lib.XSync.argtypes=[ctypes.c_void_p,ctypes.c_int]
lib.XCloseDisplay.argtypes=[ctypes.c_void_p]
xtst.XTestFakeButtonEvent.argtypes=[ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_ulong]
display=None
try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    start('one-mib.md');mode('Read');precise('main/preview');wheel('main/preview',18);precise('main/preview')
    display=lib.XOpenDisplay(ENV['DISPLAY'].encode());assert display
    rect=view('main/preview')['viewport_rect'];run('xdotool','mousemove','--window',window,str(round((rect[0]+rect[2])/2)),str(round((rect[1]+rect[3])/2)))
    before=view('main/preview');samples=[];last=time.perf_counter_ns();start_ns=last
    lib.XResizeWindow(display,int(window),850,820);lib.XFlush(display)
    pending=None
    while (time.perf_counter_ns()-start_ns)<3_000_000_000:
        now=time.perf_counter_ns();v=view('main/preview');samples.append(dict(monotonic_ns=now,interval_ns=now-last,pending=v.get('pending') is not None,frame=(v.get('geometry') or {}).get('frame')));last=now
        if v.get('pending') is not None:pending=v;break
        time.sleep(.001)
    assert pending is not None,'Natural pending restore was not observed'
    pending_observed_ns=time.perf_counter_ns()
    injection_begin=time.perf_counter_ns();xtst.XTestFakeButtonEvent(display,5,1,0);xtst.XTestFakeButtonEvent(display,5,0,0);lib.XSync(display,0);injection_end=time.perf_counter_ns()
    (OUT/'before-wheel-observed.json').write_text(json.dumps(pending,ensure_ascii=False,indent=2))
    cancelled=None
    while (time.perf_counter_ns()-injection_begin)<1_000_000_000:
        now=time.perf_counter_ns();v=view('main/preview');samples.append(dict(monotonic_ns=now,interval_ns=now-last,pending=v.get('pending') is not None,reason=v['reason'],frame=(v.get('geometry') or {}).get('frame')));last=now
        if v['pending'] is None and v['reason']=='user-scroll' and any(e['had_pending'] and e['raw_wheel'] and e['frame']>(before.get('geometry') or {}).get('frame',0) for e in v.get('cancel_events',[])):cancelled=v;observed_ns=time.perf_counter_ns();break
        time.sleep(.001)
    assert cancelled is not None,'Real wheel did not cancel pending restore'
    upper=(observed_ns-injection_begin)/1e6;assert upper<=100,('Frozen cancellation budget exceeded',upper)
    tail=[]
    for _ in range(20):
        v=view('main/preview');tail.append(dict(offset=v['position']['offset'],pending=v['pending'],reason=v['reason']));time.sleep(.1)
    assert all(v['pending'] is None for v in tail)
    assert max(abs(v['offset'][1]-tail[-1]['offset'][1]) for v in tail[4:])<=2,'Late restore pulled viewport after wheel'
    shot('01-cancelled');stop()
    result=dict(passed=True,binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),platform='Debian12/Xvfb/Openbox/Mesa',event='Real XTest wheel while actual layout restore pending',pending_observed_monotonic_ns=pending_observed_ns,pending_to_injection_ms=(injection_begin-pending_observed_ns)/1e6,cancel_events=cancelled['cancel_events'],injection_begin_monotonic_ns=injection_begin,injection_end_monotonic_ns=injection_end,observed_cancel_monotonic_ns=observed_ns,cancellation_observed_upper_bound_ms=upper,requested_poll_interval_ms=1,maximum_observed_sample_gap_ms=max(s['interval_ns'] for s in samples)/1e6,samples=samples,stability_seconds=2,tail=tail,boundary='End-to-end native-input to read-only report observation wall-clock upper bound. The current-pass raw-wheel cancel event proves had_pending=true. Stage A layout restore cancellation only; not Stage B matching CPU stop latency or physical-display latency.')
    (OUT/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2));print(json.dumps(result,indent=2))
except Exception as error:
    try:shot('failure-observation')
    except Exception:pass
    (OUT/'failure.json').write_text(json.dumps(dict(error=repr(error)),ensure_ascii=False,indent=2));raise
finally:
    if display:lib.XCloseDisplay(display)
    if app is not None and app.poll() is None:app.kill();app.wait(timeout=5)
    for p in reversed(services):
        p.terminate()
        try:p.wait(timeout=5)
        except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)
