# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Normal-runtime full-update cohort: native input; synchronous diagnostics off.

Only the same asynchronous RAII update observer is enabled in both binaries.
No control/state injection. Native Ctrl+E changes fresh Split mode to Read.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path('/work')
OUT = ROOT/'evidence/reading-precision/implementation'/os.environ['MDCARD_ENTRY_RUN']
OUT.mkdir(parents=True, exist_ok=False)
EXE = Path(os.environ['MDCARD_READING_EXE'])
WORK = ROOT/'artifacts/reading-precision/performance-inputs'
FILES = [os.environ['MDCARD_ENTRY_FIRST'], os.environ['MDCARD_ENTRY_SECOND']]
ENV = {k:v for k,v in os.environ.items() if not k.startswith('MDCARD_')}
ENV.update(DISPLAY=':154', LANG='C.UTF-8', LIBGL_ALWAYS_SOFTWARE='1', LP_NUM_THREADS='2', MDCARD_RENDERER_DIR=str(ROOT/'dist/linux/renderer'), MDCARD_READING_STATE_PATH=str(OUT/'reading-state.json'), XDG_RUNTIME_DIR='/tmp/mdcard-precision-entry-x11')
Path(ENV['XDG_RUNTIME_DIR']).mkdir(exist_ok=True)
Path(ENV['XDG_RUNTIME_DIR']).chmod(0o700)
apps, services, inputs = [], [], []

def run(*args):
    return subprocess.run(args, env=ENV, check=True, capture_output=True)

def read(path):
    until = time.monotonic()+5
    while time.monotonic()<until:
        try:return json.loads(path.read_text())
        except (OSError,ValueError):time.sleep(.01)
    raise AssertionError('No complete observation '+str(path))

def wait(predicate, message):
    until = time.monotonic()+20
    while time.monotonic()<until:
        if predicate():return
        assert all(a['process'].poll() is None for a in apps)
        time.sleep(.05)
    raise AssertionError(message)

def position(a):
    return read(OUT/'reading-state.json')['records'][str((WORK/a['file']).resolve())]['views']['main/preview']

def ready(a):
    try:return position(a)['fingerprint']!=0
    except (OSError,KeyError,AssertionError):return False

def active(a):
    run('xdotool','windowraise',a['window'],'windowactivate','--sync',a['window'])
    time.sleep(.15)
    assert run('xdotool','getactivewindow').stdout.decode().strip()==a['window']

def start(name, filename):
    settings=OUT/(name+'-settings.json')
    settings.write_text(json.dumps(dict(language='en',appearance='Light',reduce_motion=True,font_size=15)))
    env=dict(ENV,MDCARD_SETTINGS_PATH=str(settings),MDCARD_FULL_UPDATE_REPORT=str(OUT/(name+'-full-update.json')))
    process=subprocess.Popen([str(EXE),str(WORK/filename)],env=env,stdout=(OUT/(name+'-app.log')).open('wb'),stderr=subprocess.STDOUT)
    window=None
    for _ in range(100):
        time.sleep(.1)
        result=subprocess.run(['xdotool','search','--all','--pid',str(process.pid),'--name','MDcard'],env=ENV,capture_output=True)
        if result.returncode==0:window=result.stdout.decode().splitlines()[0];break
    assert window,'No actual X11 window'
    app=dict(name=name,file=filename,window=window,process=process)
    apps.append(app)
    run('xdotool','windowsize',window,'1240','820')
    active(app)
    time.sleep(.7)
    run('xdotool','key','--clearmodifiers','ctrl+e')
    wait(lambda:ready(app),'Read position is persistable after native Ctrl+E')
    time.sleep(.5)
    run('import','-window',window,str(OUT/(name+'-before.png')))
    return app

def wheel(a, notches):
    active(a)
    # Both paired builds use the same 1240x820 native client coordinates.
    # The screenshots retain the actual document view and route for review.
    run('xdotool','mousemove','--window',a['window'],'900','400')
    before=position(a)
    started=time.monotonic_ns()
    run('xdotool','click','--repeat',str(abs(notches)),'--delay','65','5' if notches>0 else '4')
    completed=time.monotonic_ns()
    time.sleep(1.0)
    after=position(a)
    assert after['offset'][1]>100,'Physical wheel did not move actual persisted viewport'
    assert (after['offset'][1]>before['offset'][1]) if notches>0 else (after['offset'][1]<before['offset'][1])
    inputs.append(dict(app=a['name'],native_notches=notches,delay_ms=65,native_begin_monotonic_ns=started,native_end_monotonic_ns=completed,native_input_seconds=(completed-started)/1e9,before=before,after=after))
    run('import','-window',a['window'],str(OUT/(a['name']+'-after-'+str(len(inputs))+'.png')))

def summary(values):
    values=sorted(values)
    return dict(count=len(values),p50=values[len(values)//2] if values else None,p95=values[min(len(values)-1,int(len(values)*.95))] if values else None,maximum=max(values) if values else None)

try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT))
    time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT))
    time.sleep(.6)
    hashes={file:hashlib.sha256((WORK/file).read_bytes()).hexdigest() for file in FILES}
    a=start('a',FILES[0]);b=start('b',FILES[1])
    wheel(a,100);wheel(b,55);wheel(a,-25)
    timing={}
    for app in apps:
        report=read(OUT/(app['name']+'-full-update.json'))
        assert report['queue_dropped_samples']==0
        samples=report['samples'];frames={};wheel_frames=set()
        for sample in samples:
            frames[sample['frame']]=frames.get(sample['frame'],0)+sample['elapsed_wall_ms']
            if sample['wheel_input']:wheel_frames.add(sample['frame'])
        timing[app['name']]=dict(file=app['file'],all_invocations_wall_ms=summary([s['elapsed_wall_ms'] for s in samples]),wheel_invocations_wall_ms=summary([s['elapsed_wall_ms'] for s in samples if s['wheel_input']]),all_egui_frame_update_sums_wall_ms=summary(list(frames.values())),wheel_egui_frame_update_sums_wall_ms=summary([frames[f] for f in wheel_frames]),scope=report['scope'],dropped_samples=report['queue_dropped_samples'])
        assert timing[app['name']]['wheel_invocations_wall_ms']['count']>30
    for app in apps:
        active(app);run('xdotool','key','--clearmodifiers','ctrl+q')
        app['process'].wait(timeout=8)
        assert app['process'].returncode==0
    final_observer_integrity={}
    for app in apps:
        path=OUT/(app['name']+'-full-update.json')
        value=read(path)
        assert value['flush_complete'] is True
        assert value['submitted_samples']==value['processed_samples']==value['total_samples']==len(value['samples'])
        assert value['queue_dropped_samples']==value['retention_dropped_samples']==0
        final_observer_integrity[app['name']]=dict(submitted_samples=value['submitted_samples'],processed_samples=value['processed_samples'],flush_complete=True,raw_sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    for file,value in hashes.items():assert hashlib.sha256((WORK/file).read_bytes()).hexdigest()==value
    result=dict(passed=True,binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),platform='Debian12/X11/Xvfb/Openbox/Mesa software GL',diagnostics='UI/reading/rich synchronous diagnostics OFF. Only identical asynchronous full-update observer ON, its own queue/writer outside sample. Product reading-state persistence remains ON.',timing=timing,timing_scope="Live pre-close summary; final complete raw validated after close",final_observer_integrity=final_observer_integrity,native_inputs=inputs,manuscript_sha256_unchanged=hashes,measurement='Function wall per App::update invocation; separately sums invocations sharing one egui frame. Not thread CPU, framework render, physical display latency or FPS.')
    (OUT/'result.json').write_text(json.dumps(result,indent=2))
    print(json.dumps(dict(passed=True,timing=timing),indent=2))
except Exception as error:
    (OUT/'failure.json').write_text(json.dumps(dict(error=repr(error),native_inputs=inputs),indent=2))
    raise
finally:
    for app in apps:
        if app['process'].poll() is None:app['process'].kill();app['process'].wait(timeout=5)
    for process in reversed(services):
        process.terminate()
        try:process.wait(timeout=5)
        except subprocess.TimeoutExpired:process.kill();process.wait(timeout=5)
