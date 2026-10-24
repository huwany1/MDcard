#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Real native GUI benchmark. Run inside the existing mdcard-dev container.

Timing names are literal: OS injection -> observed native revision, update CPU,
Tree-sitter duration and X11 frame intervals are separate measurements. This
script never substitutes a microbenchmark for presentation FPS.
"""
import argparse, hashlib, json, os, subprocess, time, uuid, shutil
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser();parser.add_argument('--out',type=Path,default=ROOT/'evidence/performance/linux');parser.add_argument('--smoke',action='store_true');parser.add_argument('--mixed',action='store_true');parser.add_argument('--gpu-finish',action='store_true');parser.add_argument('--strict',action='store_true',help='Fail measured source/parse/mount budgets; use Windows verifier for true injection-to-GPU and continuous scrollbar cadence');args=parser.parse_args()
OUT=args.out.resolve();OUT.mkdir(parents=True,exist_ok=True)
EXE=Path('/tmp')/f'mdcard-large-bench-{uuid.uuid4()}';shutil.copy2(ROOT/'target/release/mdcard',EXE);BINARY_HASH=hashlib.file_digest(EXE.open('rb'),'sha256').hexdigest();DISPLAY=':137'
STATE=Path('/tmp')/f'mdcard-large-report-{uuid.uuid4()}.json'
ENV=dict(os.environ,DISPLAY=DISPLAY,LANG='C.UTF-8',LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',XDG_RUNTIME_DIR='/tmp/mdcard-large-runtime',MDCARD_SETTINGS_PATH=str(OUT/'settings.json'),MDCARD_PERFORMANCE_REPORT=str(STATE))
if args.gpu_finish:ENV.update(MDCARD_PERFORMANCE_GPU_FINISH='1',MDCARD_PERFORMANCE_CONTINUOUS='1')
Path(ENV['XDG_RUNTIME_DIR']).mkdir(exist_ok=True);Path(ENV['XDG_RUNTIME_DIR']).chmod(0o700)
def run(*a,**kw):return subprocess.run(a,env=ENV,check=True,**kw)
def state():
    try:return json.loads(STATE.read_text())
    except (OSError,ValueError):return {}
def wait(test,label,seconds=20):
    deadline=time.monotonic()+seconds
    while time.monotonic()<deadline:
        s=state()
        if test(s):return s
        time.sleep(.001)
    raise AssertionError(label+': '+json.dumps(state())[:1800])
def stats(v):
    v=sorted(v)
    return dict(samples=len(v),p50_ms=v[len(v)//2] if v else None,p95_ms=v[min(len(v)-1,int(len(v)*.95))] if v else None,max_ms=max(v) if v else None)
children=[];app=None;records=[];started=time.monotonic()
try:
    corpus=''.join(f'# Heading {i:05d}\n\n中文长文测试；每个段落都有精确的源码锚点。 {i:05d} marker-{i:05d}\n\n' for i in range(3000))
    fixture=OUT/'long.md';fixture.write_text(corpus,encoding='utf-8')
    (OUT/'settings.json').write_text(json.dumps(dict(language='en',appearance='Light',gpu_acceleration=True,reduce_motion=True,font_size=15)))
    (OUT/'state.json').unlink(missing_ok=True)
    children.append(subprocess.Popen(['Xvfb',DISPLAY,'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.25)
    children.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.25)
    launch=time.monotonic();app=subprocess.Popen([str(EXE),str(fixture)],env=ENV,stdout=(OUT/'app.log').open('wb'),stderr=subprocess.STDOUT)
    s=wait(lambda s:s.get('active') and s.get('laid_out_lines',0)>0,'Native source ready');process_to_report=(time.monotonic()-launch)*1000
    wait(lambda s:s.get('preview_mounted',0)>0,'Preview ready')
    window=run('xdotool','search','--onlyvisible','--pid',str(app.pid),'--name','MDcard',capture_output=True).stdout.decode().splitlines()[0]
    run('xdotool','windowsize',window,'1420','950','windowfocus',window);time.sleep(.2)
    s=wait(lambda s:s.get('source_rect'),'Source rect');r=s['source_rect'];x=round(r[0]+55);y=round(r[1]+25)
    run('xdotool','mousemove','--window',window,str(x),str(y),'click','1');time.sleep(.03)
    insertion=state()['selection'][0];expected=corpus
    values=['x'] if not args.mixed else ['a','中','，','。','(',')','[',']','*','_','$','-','#','\n']
    for i in range(5 if args.smoke else 80):
        value=values[i%len(values)];before=state()['revision'];at=time.monotonic()
        if value=='\n':run('xdotool','key','--clearmodifiers','--delay','0','Return')
        elif value.isascii():run('xdotool','type','--clearmodifiers','--delay','0',value)
        else:
            run('xclip','-selection','clipboard','-loops','1',input=value.encode(),stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            run('xdotool','key','--clearmodifiers','--delay','0','ctrl+v')
        result=wait(lambda s:s.get('revision',0)>before,'Key visible')
        at_char=result['selection'][0];expected=expected[:at_char-len(value)]+value+expected[at_char-len(value):]
        records.append((time.monotonic()-at)*1000);time.sleep(.035)
    run('xdotool','key','--clearmodifiers','--delay','0','ctrl+a','ctrl+c');time.sleep(.08)
    copied=run('xclip','-selection','clipboard','-o',capture_output=True).stdout.decode();assert copied==expected,'Ctrl+A/C differs from exact expected source'
    run('xdotool','key','--clearmodifiers','--delay','0','ctrl+End');time.sleep(.15)
    end=state();assert end['selection'][0]>len(corpus)-100,'Ctrl+End global navigation'
    run('xdotool','key','--clearmodifiers','--delay','0','ctrl+Home');time.sleep(.1)
    run('import','-window',window,str(OUT/'native.png'))
    final=wait(lambda s:s.get('source_rect'),'Final report');(OUT/'state.json').write_text(json.dumps(final,indent=2)+'\n');rss=Path(f'/proc/{app.pid}/status').read_text();rss_kib=int(next(line.split()[1] for line in rss.splitlines() if line.startswith('VmRSS:')))
    report=dict(verification_id=str(uuid.uuid4()),binary_sha256=BINARY_HASH,platform=final.get('platform'),renderer=final.get('renderer'),fixture_characters=len(corpus),fixture_lines=corpus.count('\n'),mixed_input=args.mixed,process_launch_to_first_report_ms=process_to_report,disk_open_to_interactive_update_ms=final.get('disk_open_to_interactive_update_ms'),disk_open_to_preview_update_ms=final.get('disk_open_to_preview_update_ms'),os_injection_to_revision_report=stats(records),os_injection_samples_ms=records,input_event_to_update_finish=stats(final.get('input_event_to_update_finish_ms',[])),update_cpu=stats(final.get('update_cpu_ms',[])),incremental_parse=stats(final.get('incremental_parse_ms',[])),gpu_completion=final.get('paint'),rss_kib=rss_kib,source_window=final.get('source_window'),laid_out_lines=final.get('laid_out_lines'),preview_mounted=final.get('preview_mounted'),preview_blocks=final.get('preview_blocks'),ctrl_a_copy_complete=True,ctrl_end_complete=True,note='No presentation FPS claim: Xvfb software rendering; OS-to-report includes automation process and report publication. See raw state.json for all unfiltered native samples.',duration_seconds=time.monotonic()-started)
    parse_samples=final.get('incremental_parse_ms',[])
    report['gates']={
        'source_interactive_update_under_200ms':0 < (report['disk_open_to_interactive_update_ms'] or 1e9) < 200,
        'ordinary_incremental_parse_under_5ms':len(parse_samples)>=(5 if args.smoke else 80) and max(parse_samples)<5,
        'bounded_source_layout':0 < final.get('laid_out_lines',0) < 200,
        'bounded_preview_mounts':0 < final.get('preview_mounted',0) < 100,
        'exact_full_document_copy':True,
    }
    report['passed']=all(report['gates'].values())
    report['measurement_limits']='Linux strict gates cover the named source-update/parse/mount budgets. OS injection-to-report includes publication, and update-to-GPU excludes the OS queue; neither proves complete input latency or presented FPS. The Windows QPC verifier covers injection-to-GPU and real scrollbar GPU-completion cadence.'
    (OUT/'performance.json').write_text(json.dumps(report,indent=2)+'\n')
    display={k:v for k,v in report.items() if k not in ['os_injection_samples_ms','gpu_completion']}
    if final.get('paint'):display['gpu_completion']={key:stats(final['paint'].get(key,[])) for key in ['intervals_ms','input_to_complete_ms','frame_to_complete_ms']}
    print(json.dumps(display,indent=2))
    if args.strict and not report['passed']:raise AssertionError('Measured performance budgets failed; see performance.json gates and all raw samples')
    # Close the dirty fixture via a discard dialog; always reap every process.
    run('xdotool','key','--clearmodifiers','Escape','ctrl+q');time.sleep(.08)
finally:
    if app and app.poll() is None:app.terminate();app.wait(timeout=10)
    for child in reversed(children):child.terminate();child.wait(timeout=5)
    STATE.unlink(missing_ok=True)
    EXE.unlink(missing_ok=True)
