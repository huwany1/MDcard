# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Two native MDcard processes, interleaved writes, lock failure, and actual timings."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
import uuid

ROOT=Path('/work')
OUT=ROOT/'evidence/reading-state/implementation'/os.environ.get('MDCARD_READING_MEASURE_RUN','linux-concurrency-r4')
OUT.mkdir(parents=True,exist_ok=True)
EXE=Path(os.environ.get('MDCARD_READING_EXE','/work/artifacts/reading-state/linux-r4/mdcard'))
RENDERER=Path(os.environ.get('MDCARD_READING_RENDERER_DIR',str(EXE.parent/'renderer' if (EXE.parent/'renderer').is_dir() else ROOT/'dist/linux/renderer')))
WORK=ROOT/'artifacts/reading-state/linux-r3/workspace'
FIRST=os.environ.get('MDCARD_READING_MEASURE_FIRST','a.md')
SECOND=os.environ.get('MDCARD_READING_MEASURE_SECOND','b.md')
ENV=dict(os.environ,DISPLAY=':152',LANG='C.UTF-8',LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',MDCARD_RENDERER_DIR=str(RENDERER),MDCARD_READING_STATE_PATH=str(OUT/'reading-state.json'),XDG_RUNTIME_DIR='/tmp/mdcard-reading-multi-x11')
Path(ENV['XDG_RUNTIME_DIR']).mkdir(exist_ok=True)
Path(ENV['XDG_RUNTIME_DIR']).chmod(0o700)
services=[]
apps=[]
store=OUT/'reading-state.json'
results=[]

def run(*args,**kwargs):return subprocess.run(args,env=ENV,check=True,**kwargs)
def read(path):
    for _ in range(40):
        try:return json.loads(path.read_text())
        except (OSError,ValueError):time.sleep(.025)
    raise AssertionError('No observation '+str(path))
def reading(a):return read(OUT/(a['name']+'-reading.json'))
def wait(a,predicate,message):
    until=time.monotonic()+15
    while time.monotonic()<until:
        assert a['process'].poll() is None
        s=reading(a)
        if predicate(s):return s
        time.sleep(.05)
    raise AssertionError(message)
def active(a):
    until=time.monotonic()+5
    actual=None
    while time.monotonic()<until:
        subprocess.run(['xdotool','windowraise',a['window'],'windowactivate','--sync',a['window']],env=ENV,capture_output=True)
        time.sleep(.15)
        observed=subprocess.run(['xdotool','getactivewindow'],env=ENV,capture_output=True)
        actual=observed.stdout.decode().strip() if observed.returncode==0 else None
        if actual==a['window']:return
    raise AssertionError(('Native foreground mismatch',actual,a['window']))
def key(a,*keys):active(a);run('xdotool','key','--clearmodifiers',*keys);time.sleep(.15)
def start(name,filename):
    settings=OUT/(name+'-settings.json');settings.write_text(json.dumps(dict(language='en',appearance='Light',reduce_motion=True,font_size=15)))
    env=dict(ENV,MDCARD_SETTINGS_PATH=str(settings),MDCARD_UI_REPORT=str(OUT/(name+'-ui.json')),MDCARD_READING_REPORT=str(OUT/(name+'-reading.json')),MDCARD_RICH_REPORT=str(OUT/(name+'-rich.json')))
    p=subprocess.Popen([str(EXE),str(WORK/filename)],env=env,stdout=(OUT/(name+'-app.log')).open('ab'),stderr=subprocess.STDOUT)
    window=None
    for _ in range(100):
        time.sleep(.1);found=subprocess.run(['xdotool','search','--all','--pid',str(p.pid),'--name','MDcard'],env=env,capture_output=True)
        if found.returncode==0:window=found.stdout.decode().splitlines()[0];break
    assert window
    a=dict(name=name,filename=filename,window=window,process=p);apps.append(a)
    run('xdotool','windowsize',window,'1240','820');active(a);time.sleep(.7)
    ui=read(OUT/(name+'-ui.json'));c=next(c for c in reversed(ui['controls']) if c['name']=='Read');r=c['rect']
    run('xdotool','mousemove','--window',window,str(round((r[0]+r[2])/2)),str(round((r[1]+r[3])/2)),'click','1')
    wait(a,lambda s:s['views'].get('main/preview',{}).get('position') is not None and s['views']['main/preview']['pending'] is None,'preview ready');time.sleep(.5)
    return a
def wheel(a,notches):
    active(a);r=reading(a)['views']['main/preview']['viewport_rect']
    run('xdotool','mousemove','--window',a['window'],str(round((r[0]+r[2])/2)),str(round((r[1]+r[3])/2)))
    before=reading(a);started=time.monotonic();run('xdotool','click','--repeat',str(abs(notches)),'--delay','65','5' if notches>0 else '4');time.sleep(1)
    after=wait(a,lambda s:s['writes']['writes']>before['writes']['writes'] and not s['dirty'],'scroll persisted')
    return dict(duration_seconds=time.monotonic()-started,successful_writes=after['writes']['writes']-before['writes']['writes'],offset=after['views']['main/preview']['position']['offset'],metrics=after['metrics'],disk_write=after['writes'])
def position(a):return reading(a)['views']['main/preview']['position']
def stop(a):key(a,'ctrl+q');a['process'].wait(timeout=8);assert a['process'].returncode==0
def summary(values):
    values=sorted(values)
    return dict(count=len(values),p50=values[len(values)//2] if values else None,p95=values[min(len(values)-1,int(len(values)*.95))] if values else None,maximum=max(values) if values else None)

try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    source_hashes={name:hashlib.sha256((WORK/name).read_bytes()).hexdigest() for name in [FIRST,SECOND]}
    a=start('a',FIRST);b=start('b',SECOND)
    first=wheel(a,100);ap=position(a)
    second=wheel(b,55);bp=position(b)
    third=wheel(a,-25);ap=position(a)
    disk=read(store)
    assert len([r for r in disk['records'].values() if not r['deleted']])>=2
    for app,p in [(a,ap),(b,bp)]:
        record=disk['records'][str((WORK/app['filename']).resolve())]
        error=[abs(x-y) for x,y in zip(record['views']['main/preview']['offset'],p['offset'])]
        assert max(error)<.001, ('Serialized f32 offset differs',error)
        assert record['views']['main/preview']['anchor']['byte']==p['anchor']['byte']
    results.append(dict(name='two independent processes interleave updates without whole-table loss',passed=True,process_ids=[a['process'].pid,b['process'].pid],a_offset=ap['offset'],b_offset=bp['offset']))
    stop(a);stop(b)
    a=start('a-restart',FIRST);b=start('b-restart',SECOND)
    for app,p in [(a,ap),(b,bp)]:
        current=position(app);error=[abs(x-y) for x,y in zip(p['offset'],current['offset'])];assert max(error)<=2
        assert current['anchor']['byte']==p['anchor']['byte'];results.append(dict(name='restart '+app['filename'],passed=True,error_logical_px=error,source_byte=current['anchor']['byte']))
    # Hold the same OS flock from this separate controller process. The real GUI
    # must keep moving while persistence reports the fault and retains the bytes.
    time.sleep(.8);previous=store.read_bytes();old_position=position(a)
    lock=store.with_suffix('.json.lock').open('r+b');fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    active(a);r=reading(a)['views']['main/preview']['viewport_rect'];run('xdotool','mousemove','--window',a['window'],str(round((r[0]+r[2])/2)),str(round((r[1]+r[3])/2)),'click','--repeat','8','--delay','65','4')
    failed=wait(a,lambda s:s['writes']['error'] is not None,'lock failure should be inspectable')
    assert position(a)['offset']!=old_position['offset'];assert store.read_bytes()==previous
    (OUT/'lock-failure.json').write_text(json.dumps(failed,ensure_ascii=False,indent=2))
    fcntl.flock(lock,fcntl.LOCK_UN);lock.close()
    wait(a,lambda s:s['writes']['error'] is None and not s['dirty'],'retry after lock release')
    results.append(dict(name='real OS lock failure keeps memory and previous disk record, retries after release',passed=True,error=failed['writes']['error']))
    for name,digest in source_hashes.items():assert hashlib.sha256((WORK/name).read_bytes()).hexdigest()==digest
    stop(a);stop(b)
    metrics=first.pop('metrics');rich=read(OUT/'a-rich.json');timing=dict(position_capture_cpu_ms=summary(metrics['position_capture_cpu_ms']),submit_cpu_ms=summary(metrics['submit_cpu_ms']),index_build_ms=summary(metrics['index_build_ms']),whole_update_scroll_cpu_ms=summary(rich['scrollCpuMs']),boundary=metrics['note']+' Whole update is CPU only, not GPU or display presentation latency.')
    result=dict(verification_id=str(uuid.uuid4()),passed=True,platform='Debian 12 / X11 / Xvfb / Openbox / Mesa',binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),cases=results,continuous_scroll=first,other_scroll=[second,third],timing=timing,diagnostic_reports='Synchronous diagnostic JSON writes enabled; diagnostic report writes are not counted in successful_writes',manuscript_hashes_unchanged=True)
    (OUT/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(json.dumps(result,ensure_ascii=False,indent=2))
except Exception as error:
    (OUT/'failure.json').write_text(json.dumps(dict(error=repr(error),cases=results),ensure_ascii=False,indent=2));raise
finally:
    for app in apps:
        if app['process'].poll() is None:app['process'].kill();app['process'].wait(timeout=5)
    for p in reversed(services):
        p.terminate()
        try:p.wait(timeout=5)
        except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)
