# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Two real instances: clearing never authorizes an old proof completion/snapshot."""
from pathlib import Path
common=Path('/work/scripts/verify-reading-precision-worker-linux.py').read_text()
exec(compile(common[:common.index('\ntry:\n    services.append')], 'two-instance-native-helpers', 'exec'))
ROOTOUT=OUT
shared=ROOTOUT/'shared-reading-state.json'
instances={}

def instance(name):
    global OUT,ENV,app,window
    OUT,ENV,app,window=instances[name]

def launch(name,filename):
    global OUT,ENV,app,window
    OUT=ROOTOUT/name;OUT.mkdir(exist_ok=True)
    ENV=dict(ENV,MDCARD_READING_STATE_PATH=str(shared),MDCARD_READING_REPORT=str(OUT/'reading.json'),MDCARD_UI_REPORT=str(OUT/'ui.json'),MDCARD_SEARCH_REPORT=str(OUT/'search.json'),MDCARD_SETTINGS_PATH=str(OUT/'settings.json'))
    (OUT/'settings.json').write_text(json.dumps(dict(language='en',appearance='Light',reduce_motion=True,font_size=15)))
    start(filename)
    instances[name]=(OUT,ENV,app,window)

def disk():return json.loads(shared.read_text())

def cleared(d):return d.get('cleared_ms',0)>0 and all(not r.get('views') for r in d.get('records',{}).values())

try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(ROOTOUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(ROOTOUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    display=lib.XOpenDisplay(ENV['DISPLAY'].encode());assert display
    launch('old','heavy.md');mode('Read');precise('main/preview');wheel('main/preview',8)
    wait(proof_ready,'First old proof');time.sleep(.7)
    original=state();old_id=original['main_id'];old_position=view('main/preview')['position'];assert old_position['offset'][1]>100
    launch('clearer','tiny.md');key('ctrl+comma');time.sleep(.5)
    run('xdotool','mousemove','--window',window,'800','580','click','--repeat','15','--delay','10','5');time.sleep(.3)
    r=next(c['rect'] for c in state('ui')['controls'] if c['name']=='Clear all reading positions');assert 0<r[1]<r[3]<820,r
    clear_window=window;clear_rect=r
    instance('old');run('xdotool','windowactivate','--sync',window)
    r=view('main/preview')['viewport_rect'];run('xdotool','mousemove','--window',window,str(round(r[2]-40)),str(round((r[1]+r[3])/2)));button(5)
    run('xdotool','windowactivate','--sync',clear_window,'mousemove','--window',clear_window,str(round((clear_rect[0]+clear_rect[2])/2)),str(round((clear_rect[1]+clear_rect[3])/2)))
    before,task,samples=running('anchor-enrichment')
    t0=time.perf_counter_ns();button(1);t1=time.perf_counter_ns()
    (ROOTOUT/'old-running-before-real-clear.json').write_text(json.dumps(before,ensure_ascii=False,indent=2))
    instance('clearer');wait(lambda s:all(not r['views'] for r in s['records']),'Clearer memory empty');shot('01-real-clear')
    instance('old')
    done=wait(lambda s:any(t['task_id']==task['task_id'] and t['finished_monotonic_ns'] for t in s['background_tasks']),'Old proof actually completed')
    time.sleep(1.2);after_disk=disk();assert cleared(after_disk),('Old background resurrected history',after_disk)
    (ROOTOUT/'after-background-completion-disk.json').write_text(json.dumps(after_disk,ensure_ascii=False,indent=2))
    run('xdotool','windowactivate','--sync',window);quick('tiny.md');quick('heavy.md');p=settle('main/preview');assert abs(p['offset'][1])<=2,p
    assert state()['main_id']==old_id
    time.sleep(1.2);shot('02-old-instance-reopen-without-old-history')
    final=disk();assert all(all(abs(p['offset'][1])<=2 for p in r['views'].values()) for r in final['records'].values())
    manuscript_hashes={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in WORK.glob('*.md')}
    instance('old');stop();instance('clearer');key('Escape');stop()
    result=dict(passed=True,binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),old_document_id=old_id,old_offset=old_position['offset'],reopened_offset=p['offset'],clear_input_call_ms=(t1-t0)/1e6,task_before=task,task_after=next(t for t in done['background_tasks'] if t['task_id']==task['task_id']),disk_clear_epoch=after_disk['cleared_ms'],manuscript_sha256=manuscript_hashes,method='Two real MDcard processes share one state file. Actual XTest click clears while other proof computation is running; wait for actual worker exit and merge, then native Quick B→A. No state injection or synthetic timestamps.')
    (ROOTOUT/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2));print(json.dumps(result,indent=2))
except Exception as error:
    try:shot('failure')
    except Exception:pass
    (ROOTOUT/'failure.json').write_text(json.dumps(dict(error=repr(error)),indent=2));raise
finally:
    if display:lib.XCloseDisplay(display)
    for _,_,p,_ in instances.values():
        if p.poll() is None:p.kill();p.wait(timeout=5)
    for p in reversed(services):
        p.terminate()
        try:p.wait(timeout=5)
        except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)
