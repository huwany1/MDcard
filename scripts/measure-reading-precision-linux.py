# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Precision-stage paired baseline/new timings with identical native inputs."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
import threading
import uuid

ROOT=Path('/work')
OUT=ROOT/'evidence/reading-precision/implementation'/os.environ.get('MDCARD_READING_MEASURE_RUN','linux-concurrency-r4')
OUT.mkdir(parents=True,exist_ok=True)
EXE=Path(os.environ.get('MDCARD_READING_EXE','/work/artifacts/reading-state/linux-r4/mdcard'))
RENDERER=Path(os.environ.get('MDCARD_READING_RENDERER_DIR',str(EXE.parent/'renderer' if (EXE.parent/'renderer').is_dir() else ROOT/'dist/linux/renderer')))
WORK=ROOT/'artifacts/reading-precision/performance-inputs'
FIRST=os.environ.get('MDCARD_READING_MEASURE_FIRST','a.md')
SECOND=os.environ.get('MDCARD_READING_MEASURE_SECOND','b.md')
ENV=dict(os.environ,DISPLAY=':154',LANG='C.UTF-8',LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',MDCARD_RENDERER_DIR=str(RENDERER),MDCARD_READING_STATE_PATH=str(OUT/'reading-state.json'),XDG_RUNTIME_DIR='/tmp/mdcard-reading-multi-x11')
Path(ENV['XDG_RUNTIME_DIR']).mkdir(exist_ok=True)
Path(ENV['XDG_RUNTIME_DIR']).chmod(0o700)
services=[]
apps=[]
store=OUT/'reading-state.json'
results=[]
observations=[]
final_observer_integrity={}
seen_writes=set()
watch_stop=threading.Event()
def sample_external():
    while not watch_stop.wait(.05):
        resident={}
        for a in list(apps):
            try:
                status=Path('/proc')/str(a['process'].pid)/'status'
                rss=next(int(line.split()[1]) for line in status.read_text().splitlines() if line.startswith('VmRSS:'))*1024
                resident[str(a['process'].pid)]=rss
            except (OSError,StopIteration):pass
            try:
                value=json.loads((OUT/(a['name']+'-reading.json')).read_text());w=value['writes'];key=(a['process'].pid,w['writes'])
                if w['writes']>0 and key not in seen_writes:
                    seen_writes.add(key);observations.append(dict(kind='successful-disk-transaction',pid=a['process'].pid,name=a['name'],duration_ms=w['last_duration_ms'],write_count=w['writes'],last_success_ms=w['last_success_ms']))
            except (OSError,ValueError,KeyError):pass
        if resident:observations.append(dict(kind='process-rss',monotonic=time.monotonic(),rss_bytes=resident,total_bytes=sum(resident.values())))
watch=threading.Thread(target=sample_external,daemon=True);watch.start()

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
    env=dict(ENV,MDCARD_SETTINGS_PATH=str(settings),MDCARD_UI_REPORT=str(OUT/(name+'-ui.json')),MDCARD_READING_REPORT=str(OUT/(name+'-reading.json')),MDCARD_RICH_REPORT=str(OUT/(name+'-rich.json')),MDCARD_FULL_UPDATE_REPORT=str(OUT/(name+'-full-update.json')))
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
    before=reading(a);started=time.monotonic();native_begin_unix_ms=time.time_ns()/1e6;run('xdotool','click','--repeat',str(abs(notches)),'--delay','65','5' if notches>0 else '4');native_end_unix_ms=time.time_ns()/1e6;native_duration=time.monotonic()-started;after_input=reading(a);time.sleep(1)
    after=wait(a,lambda s:s['writes']['writes']>before['writes']['writes'] and not s['dirty'],'scroll persisted')
    return dict(duration_seconds=time.monotonic()-started,native_input_duration_seconds=native_duration,native_begin_unix_ms=native_begin_unix_ms,native_end_unix_ms=native_end_unix_ms,observed_writes_through_input_return=after_input['writes']['writes']-before['writes']['writes'],successful_writes=after['writes']['writes']-before['writes']['writes'],offset=after['views']['main/preview']['position']['offset'],metrics=after['metrics'],disk_write=after['writes'])
def position(a):return reading(a)['views']['main/preview']['position']
def stop(a):
    key(a,'ctrl+q');a['process'].wait(timeout=8);assert a['process'].returncode==0
    path=OUT/(a['name']+'-full-update.json');value=read(path)
    assert value['flush_complete'] is True
    assert value['submitted_samples']==value['processed_samples']==value['total_samples']==len(value['samples'])
    assert value['queue_dropped_samples']==value['retention_dropped_samples']==0
    final_observer_integrity[a['name']]=dict(submitted_samples=value['submitted_samples'],processed_samples=value['processed_samples'],flush_complete=True,raw_sha256=hashlib.sha256(path.read_bytes()).hexdigest())
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
    metrics=first.pop('metrics');rich=read(OUT/'a-rich.json')
    (OUT/'first-scroll-metrics.json').write_text(json.dumps(metrics,indent=2))
    (OUT/'external-observations.json').write_text(json.dumps(observations,indent=2))
    timing={name:summary(metrics.get(name,[])) for name in ['position_capture_cpu_ms','submit_cpu_ms','index_build_ms','geometry_wall_ms','plan_wall_ms','hot_frame_wall_ms','galley_index_wall_ms']}
    timing['whole_update_scroll_cpu_ms']=summary(rich['scrollCpuMs'])
    timing['gui_main_section_wall_ms']=timing['whole_update_scroll_cpu_ms']
    timing['source_editor_wall_ms']=summary(rich['phaseCpuMs'].get('sourceEditor',[]))
    timing['actual_layout_wall_ms']={name:summary(values) for name,values in metrics.get('layout_wall_ms',{}).items()}
    timing['main_commit_wall_ms']=summary(metrics.get('main_commit_wall_ms',[]))
    timing['disk_transaction_wall_ms']=summary([v['duration_ms'] for v in observations if v['kind']=='successful-disk-transaction'])
    timing['boundary']=metrics['note']+' Raw *_cpu_ms fields are Instant.elapsed function wall-clock intervals, not thread CPU counters. Legacy whole_update_scroll_cpu_ms is only the GUI MAIN SECTION, excluding entry prework and tail cleanup/diagnostic I/O; it cannot prove the whole-update budget. RSS is sampled /proc/pid/status VmRSS of the app processes, not a private allocator peak nor renderer descendants. No value is GPU/display latency or FPS.'
    full_path=OUT/'a-full-update.json'
    if full_path.exists():
        full=read(full_path);assert full['queue_dropped_samples']==0,'Full-update observer queue lost samples'
        timing['full_app_update_wall_ms']=summary([v['elapsed_wall_ms'] for v in full['samples']])
        timing['full_app_update_wheel_wall_ms']=summary([v['elapsed_wall_ms'] for v in full['samples'] if v['wheel_input']])
        frames={};wheel_frames=set()
        for sample in full['samples']:
            frames[sample['frame']]=frames.get(sample['frame'],0)+sample['elapsed_wall_ms']
            if sample['wheel_input']:wheel_frames.add(sample['frame'])
        timing['same_egui_frame_update_sum_wall_ms']=summary(list(frames.values()))
        timing['same_egui_wheel_frame_update_sum_wall_ms']=summary([frames[f] for f in wheel_frames])
        timing['full_app_update_scope']=full['scope']
    result=dict(verification_id=str(uuid.uuid4()),passed=True,platform='Debian 12 / X11 / Xvfb / Openbox / Mesa',binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),cases=results,final_observer_integrity=final_observer_integrity,continuous_scroll=first,other_scroll=[second,third],timing=timing,diagnostic_reports='Synchronous diagnostic JSON writes enabled; diagnostic report writes are not counted in successful_writes',peak_observed_app_rss_bytes=max((v['total_bytes'] for v in observations if v['kind']=='process-rss'),default=0),sampler_interval_seconds=.05,manuscript_hashes_unchanged=True)
    (OUT/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(json.dumps(result,ensure_ascii=False,indent=2))
except Exception as error:
    (OUT/'failure.json').write_text(json.dumps(dict(error=repr(error),cases=results),ensure_ascii=False,indent=2));raise
finally:
    watch_stop.set();watch.join(timeout=2)
    for app in apps:
        if app['process'].poll() is None:app['process'].kill();app['process'].wait(timeout=5)
    for p in reversed(services):
        p.terminate()
        try:p.wait(timeout=5)
        except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)
