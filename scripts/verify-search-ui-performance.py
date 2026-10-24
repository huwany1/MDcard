#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Measure query TextEdit change -> rendered results on the real native GUI."""
import argparse,hashlib,json,os,subprocess,tempfile,time,uuid
from pathlib import Path
ROOT=Path('/work')
parser=argparse.ArgumentParser()
parser.add_argument('--exe',type=Path,default=ROOT/'target/release/mdcard')
parser.add_argument('--output',type=Path,default=ROOT/'evidence/search/linux-ui-performance')
parser.add_argument('--rounds',type=int,default=100)
parser.add_argument('--cpu-workers',type=int,default=0)
args=parser.parse_args();OUT=args.output;OUT.mkdir(parents=True,exist_ok=True)
(OUT/'performance.json').unlink(missing_ok=True)
EXE=args.exe;ENV=dict(os.environ,DISPLAY=':120',LANG='C.UTF-8',LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',XDG_RUNTIME_DIR='/tmp/mdcard-search-performance-runtime',MDCARD_SETTINGS_PATH=str(OUT/'settings.json'),MDCARD_SEARCH_REPORT=str(OUT/'state.json'))
Path(ENV['XDG_RUNTIME_DIR']).mkdir(exist_ok=True);Path(ENV['XDG_RUNTIME_DIR']).chmod(0o700)
def run(*args,**kwargs):return subprocess.run(args,env=ENV,check=True,**kwargs)
def state():
    try:return json.loads((OUT/'state.json').read_text())
    except (OSError,ValueError):return {}
def wait(predicate,message):
    until=time.monotonic()+15
    while time.monotonic()<until:
        s=state()
        if predicate(s):return s
        time.sleep(.003)
    raise AssertionError(message+': '+str(state())[:600])
children=[];load=[];app=None
try:
    with tempfile.TemporaryDirectory(prefix='mdcard-search-ui-') as folder:
        folder=Path(folder);total=0
        for n in range(1600):
            body=f'# Document {n}\nshared-search-term\nneedle-{n:04d}\n'+('中文关键字\n' if n%100==0 else '')+'An ordinary paragraph keeps the corpus realistic.\n'*170
            data=body.encode();total+=len(data);(folder/f'note-{n:04d}.md').write_bytes(data)
        children.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.5)
        children.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.4)
        (OUT/'settings.json').write_text(json.dumps(dict(language='en',appearance='Light',gpu_acceleration=True,reduce_motion=True,font_size=15)))
        app=subprocess.Popen([str(EXE),str(folder/'note-0000.md')],env=ENV,stdout=(OUT/'app.log').open('wb'),stderr=subprocess.STDOUT)
        window=None
        for _ in range(100):
            p=subprocess.run(['xdotool','search','--onlyvisible','--pid',str(app.pid),'--name','MDcard'],env=ENV,capture_output=True)
            if p.returncode==0:window=p.stdout.decode().splitlines()[0];break
            time.sleep(.05)
        assert window
        run('xdotool','windowsize',window,'1240','860','windowfocus',window);time.sleep(.4);run('xdotool','key','--clearmodifiers','ctrl+shift+f')
        initial=wait(lambda s:s.get('index',{}).get('files')==1600 and not s.get('query_pending'),'Index ready')
        for _ in range(args.cpu_workers):
            load.append(subprocess.Popen(['python3','-c','import hashlib\nb=b"x"*1048576\nwhile True: hashlib.sha256(b).digest()'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL))
        records=[];started=time.monotonic()
        queries=['needle-0731','missing-constant','shared-search-term','中文关键字',r'needle-(?:0731|0945)'];regex_enabled=False
        for i in range(args.rounds):
            query=queries[i%len(queries)]
            regex=i%len(queries)==4
            if regex!=regex_enabled:
                s=wait(lambda s:any(c['name']=='regex' for c in s.get('controls',[])),'Regex toggle');r=next(c['rect'] for c in s['controls'] if c['name']=='regex')
                run('xdotool','mousemove','--window',window,str(round((r[0]+r[2])/2)),str(round((r[1]+r[3])/2)),'click','1');regex_enabled=regex;time.sleep(.03)
            s=wait(lambda s:any(c['name']=='find-query' for c in s.get('controls',[])),'Query control');r=next(c['rect'] for c in s['controls'] if c['name']=='find-query')
            run('xdotool','mousemove','--window',window,str(round((r[0]+r[2])/2)),str(round((r[1]+r[3])/2)),'click','1')
            run('xdotool','key','--clearmodifiers','--delay','0','ctrl+a')
            run('xclip','-selection','clipboard','-loops','1',input=query.encode(),stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            run('xdotool','key','--clearmodifiers','--delay','0','ctrl+v')
            result=wait(lambda s:s.get('query')==query and not s.get('query_pending') and not s.get('error'),'Query completed')
            assert result['results']==[1,0,1600,16,2][i%len(queries)],result
            records.append(dict(query=query,visible_ms=result['visible_ms'],worker_ms=result['worker_ms'],engine_ms=result['engine_ms'],matches=result['results'],queue_ms=result.get('queue_ms'),processing_ms=result.get('processing_ms'),poll_ms=result.get('poll_ms')))
            if i==30:(folder/'note-0731.md').write_text('# Changed\nneedle-0731\nshared-search-term\n')
            if i==60:(folder/'created.md').write_text('# Added\nneedle-new\n')
            if i==80:(folder/'created.md').unlink()
            time.sleep(.06)
        run('import','-window',window,str(OUT/'native-1600-docs.png'))
        def stats(key):
            values=sorted(r[key] for r in records)
            return dict(p50_ms=values[len(values)//2],p95_ms=values[int(len(values)*.95)],p99_ms=values[int(len(values)*.99)],max_ms=max(values))
        report=dict(verification_id=str(uuid.uuid4()),date=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),timezone='UTC',exe=str(EXE),exe_sha256=hashlib.file_digest(EXE.open('rb'),'sha256').hexdigest(),documents=1600,bytes=total,rounds=len(records),duration_seconds=time.monotonic()-started,cpu_load_workers=args.cpu_workers,cpu_load='SHA256 1MiB continuous per process',load_processes_alive=all(p.poll() is None for p in load),visible=stats('visible_ms'),worker=stats('worker_ms'),engine=stats('engine_ms'),includes_debounce_and_ui_poll_and_render=True,debounce_ms=0,incremental_mutation_add_remove=True,records=records)
        report['initial_index']=initial.get('index');report['final_index']=state().get('index')
        for name in ('queue_ms','processing_ms','poll_ms'):
            if all(r[name] is not None for r in records):report[name]=stats(name)
        if 'full_verifications' in report['final_index']:
            assert report['final_index']['full_verifications']>report['initial_index']['full_verifications'],report
        (OUT/'performance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k!='records'}))
        assert report['visible']['p95_ms']<50,report['visible']
        run('xdotool','key','Escape','ctrl+q');app.wait(timeout=8);assert app.returncode==0;app=None
finally:
    if app and app.poll() is None:app.kill();app.wait()
    for child in reversed(children):child.terminate();child.wait(timeout=5)
    for child in load:child.terminate();child.wait(timeout=5)
