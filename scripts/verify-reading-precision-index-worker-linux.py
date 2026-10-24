# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Real native-input cancellation of naturally running precision computation."""
from pathlib import Path
common=Path('/work/scripts/verify-reading-precision-cancellation-linux.py').read_text()
exec(compile(common[:common.index('\ntry:\n    services.append')], 'worker-native-helpers', 'exec'))
WORK=OUT/'workspace';WORK.mkdir(exist_ok=True)
text=''.join(f'Paragraph record {i:05}: '+('Original 中文 e\u0301 👩🏽‍💻 information with immutable neighbor context. '*7)+'\n\n' for i in range(20000))
assert 10*1024*1024<len(text.encode())<13*1024*1024
(WORK/'heavy.md').write_text(text);(WORK/'tiny.md').write_text('# A newly opened small source\n\nIndependent small body with normal text.\n')
ENV.pop('MDCARD_READING_GEOMETRY_REPORT',None)
measurements=[]
lib.XWarpPointer.argtypes=[ctypes.c_void_p,ctypes.c_ulong,ctypes.c_ulong,ctypes.c_int,ctypes.c_int,ctypes.c_uint,ctypes.c_uint,ctypes.c_int,ctypes.c_int]

def button(number):
    xtst.XTestFakeButtonEvent(display,number,1,0);lib.XFlush(display)
    if number==1:time.sleep(.001)
    xtst.XTestFakeButtonEvent(display,number,0,0);lib.XSync(display,0)

def running(kind):
    until=time.perf_counter_ns()+12_000_000_000;samples=[];last=time.perf_counter_ns()
    while time.perf_counter_ns()<until:
        now=time.perf_counter_ns();s=state();samples.append(dict(monotonic_ns=now,interval_ns=now-last));last=now
        for t in reversed(s.get('background_tasks',[])):
            if t['kind']==kind and t['running'] and t['progress_checkpoints']>=100 and t['cancel_requested_monotonic_ns']==0:return s,t,samples
        time.sleep(.001)
    raise AssertionError(('No natural running computation observed',kind,s.get('background_tasks')))

def cancel_measure(kind,event):
    before,task,samples=running(kind);observed=time.perf_counter_ns()
    if event=='tree':
        lib.XWarpPointer(display,0,int(window),0,0,0,0,round((tree_rect[0]+tree_rect[2])/2),round((tree_rect[1]+tree_rect[3])/2));lib.XSync(display,0)
    injection=time.perf_counter_ns();button(5 if event=='wheel' else 1);input_end=time.perf_counter_ns()
    # Save raw evidence only after actual input, never before its delivery.
    (OUT/(kind+'-'+event+'-before.json')).write_text(json.dumps(before,ensure_ascii=False,indent=2))
    until=injection+1_000_000_000;finished=None;last=input_end
    while time.perf_counter_ns()<until:
        now=time.perf_counter_ns();s=state();samples.append(dict(monotonic_ns=now,interval_ns=now-last));last=now
        t=next((t for t in s['background_tasks'] if t['task_id']==task['task_id']),None)
        if t and t['finished_monotonic_ns'] and t['cancel_requested_while_running']:
            finished=t;end=time.perf_counter_ns();break
        time.sleep(.001)
    assert finished,('Native input did not stop running old computation',task,s['background_tasks'])
    assert finished['progress_checkpoints_at_cancel']>=100
    assert finished['cancel_to_worker_exit_wall_ms']<=100
    upper=(end-injection)/1e6;assert upper<=100,('Native-to-observed-worker-exit exceeded budget',upper)
    measurement=dict(kind=kind,event=event,native_input_start_monotonic_ns=injection,task_before=task,task_after=finished,pending_to_injection_ms=(injection-observed)/1e6,input_call_ms=(input_end-injection)/1e6,native_to_observed_worker_exit_upper_bound_ms=upper,poll_requested_ms=1,maximum_sample_gap_ms=max(x['interval_ns'] for x in samples)/1e6,samples=samples)
    measurements.append(measurement);return s

def proof_ready(s):
    v=s['views']['main/preview'];p=v.get('position') or {};a=p.get('text_anchor') or {}
    return (a.get('source') or {}).get('proof') is not None and v.get('proof_pending') is None

def launch_fast():
    global app,window
    app=subprocess.Popen([str(EXE),str(WORK/'heavy.md')],env=ENV,stdout=(OUT/'app.log').open('ab'),stderr=subprocess.STDOUT)
    for _ in range(1200):
        found=subprocess.run(['xdotool','search','--all','--pid',str(app.pid),'--name','MDcard'],env=ENV,capture_output=True)
        if found.returncode==0:window=found.stdout.decode().splitlines()[0];break
        time.sleep(.01)
    else:raise AssertionError('No new X11 window')
    run('xdotool','windowsize',window,'1240','820','windowactivate','--sync',window)

try:
    (WORK/'heavy.md').write_text(''.join(f'# h{i:06}\n\n' for i in range(400000)))
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    display=lib.XOpenDisplay(ENV['DISPLAY'].encode());assert display
    start('tiny.md');mode('Read')
    controls=state('ui')['controls'];tree_rect=next(c['rect'] for c in controls if c['name']=='tree:tiny.md');heavy=next(c['rect'] for c in controls if c['name']=='tree:heavy.md')
    run('xdotool','mousemove','--window',window,str(round((heavy[0]+heavy[2])/2)),str(round((heavy[1]+heavy[3])/2)));time.sleep(.04);button(1)
    cancel_measure('source-index','tree')
    new=wait(lambda s:any((r.get('path')or'').endswith('/tiny.md')and r['id']==s['main_id']for r in s['records']),'New small source after cancelled index')
    elapsed=(time.perf_counter_ns()-measurements[-1]['native_input_start_monotonic_ns'])/1e6
    shot('01-index-cancelled-small-source');stop()
    (OUT/'result.json').write_text(json.dumps(dict(passed=True,binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),platform='Debian12/Xvfb/Openbox/Mesa',natural_dense_document_bytes=(WORK/'heavy.md').stat().st_size,measurements=measurements,new_document_available_wall_ms=elapsed,note='Actual source-index computation was running before real tree click. Atomic token checkpoint stop and new current document independently observed. No artificial product delay.'),ensure_ascii=False,indent=2))
except Exception as error:
    try:shot('failure')
    except Exception:pass
    (OUT/'failure.json').write_text(json.dumps(dict(error=repr(error),measurements=measurements),ensure_ascii=False,indent=2));raise
finally:
    if display:lib.XCloseDisplay(display)
    if app is not None and app.poll() is None:app.kill();app.wait(timeout=5)
    for p in reversed(services):
        p.terminate()
        try:p.wait(timeout=5)
        except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)
