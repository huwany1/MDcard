#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Visit 220 real formula/diagram nodes using the native preview scrollbar.

This is deliberately an application test: it launches the packaged renderer,
drags the actual egui scrollbar, waits for visible compilation, revisits every
node, and measures the whole native/Node/Chromium process tree.
"""
import argparse,hashlib,json,os,shutil,subprocess,time,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
ap=argparse.ArgumentParser();ap.add_argument('--out',type=Path,default=ROOT/'evidence/performance/media');ap.add_argument('--nodes',type=int,default=220);args=ap.parse_args()
OUT=args.out.resolve();OUT.mkdir(parents=True,exist_ok=True);ID=str(uuid.uuid4());EXE=Path('/tmp')/('mdcard-media-'+ID);shutil.copy2(ROOT/'target/release/mdcard',EXE);SHA=hashlib.file_digest(EXE.open('rb'),'sha256').hexdigest();STATE=Path('/tmp')/('mdcard-media-'+ID+'.json')
ENV=dict(os.environ,DISPLAY=':139',LANG='C.UTF-8',LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',XDG_RUNTIME_DIR='/tmp/mdcard-media-runtime',MDCARD_SETTINGS_PATH=str(OUT/'settings.json'),MDCARD_PERFORMANCE_REPORT=str(STATE),MDCARD_RENDERER_DIR=str(ROOT/'renderer'),MDCARD_PERFORMANCE_GPU_FINISH='1',MDCARD_PERFORMANCE_CONTINUOUS='1')
Path(ENV['XDG_RUNTIME_DIR']).mkdir(exist_ok=True);Path(ENV['XDG_RUNTIME_DIR']).chmod(0o700)
def run(*a,**kw):return subprocess.run(a,env=ENV,check=True,**kw)
def state():
    try:return json.loads(STATE.read_text())
    except (OSError,ValueError):return {}
def wait(test,label,seconds=30):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        s=state()
        if test(s):return s
        if app and app.poll() is not None:raise AssertionError('Application exited: '+label)
        time.sleep(.005)
    raise AssertionError(label+': '+str({k:v for k,v in state().items() if k in ['cache','preview_range','preview_offset']})[:900])
def process_tree():
    processes={}
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():continue
        try:
            data=(entry/'status').read_text();ppid=int(next(l.split()[1] for l in data.splitlines() if l.startswith('PPid:')));rss=int(next((l.split()[1] for l in data.splitlines() if l.startswith('VmRSS:')),0));processes[int(entry.name)]=(ppid,rss)
        except (OSError,StopIteration,ValueError):pass
    selected={app.pid}
    while True:
        added={pid for pid,(parent,_) in processes.items() if parent in selected}-selected
        if not added:break
        selected|=added
    pss=0
    for pid in selected:
        try:pss+=int(next(line.split()[1] for line in Path(f'/proc/{pid}/smaps_rollup').read_text().splitlines() if line.startswith('Pss:')))
        except (OSError,StopIteration,ValueError):pass
    return dict(rss_kib=sum(processes.get(pid,(0,0))[1] for pid in selected),pss_kib=pss,main_rss_kib=processes.get(app.pid,(0,0))[1],processes=len(selected))
children=[];app=None;records=[];began=time.monotonic()
try:
    pieces=[]
    for i in range(args.nodes):
        if i%20==0:node=f'```plantuml\nAlice -> Bob : message {i}\n```\n'
        elif i%20==1:node=f'```flow\nst=>start: Begin {i}\ne=>end: Done\nst->e\n```\n'
        elif i%4==0:node=f'```mermaid\nflowchart LR\nA[Input {i}] --> B[Output {i}]\n```\n'
        else:node=f'$$\nx_{{{i}}}^2 + y^2 = z^2 + \\sum_{{k=1}}^{{{i+1}}} k\n$$\n'
        pieces.append(f'# Media {i:03d}\n\n'+('Long document preview retains its source anchor and compiles visible media only. '*4)+'\n\n'+node+'\n')
    fixture=OUT/'media.md';fixture.write_text(''.join(pieces),encoding='utf-8')
    (OUT/'settings.json').write_text(json.dumps(dict(language='en',appearance='Light',gpu_acceleration=True,reduce_motion=True,font_size=15)))
    children.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.3)
    children.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.3)
    app=subprocess.Popen([str(EXE),str(fixture)],env=ENV,stdout=(OUT/'app.log').open('wb'),stderr=subprocess.STDOUT)
    wait(lambda s:s.get('preview_scrollbar') and s.get('preview_mounted',0)>0,'Preview scrollbar')
    window=run('xdotool','search','--onlyvisible','--pid',str(app.pid),'--name','MDcard',capture_output=True).stdout.decode().splitlines()[0]
    run('xdotool','windowsize',window,'1420','950','windowfocus',window);time.sleep(.2)
    wait(lambda s:s.get('cache',{}).get('completed',0)>0 and not s['cache']['pending'],'Initial renderer')
    s=state();r=s['preview_scrollbar'];x=round((r[0]+r[2])/2);top=round(r[1]+5);bottom=round(r[3]-5)
    def sample(phase,position):
        before=state().get('frame',0);run('xdotool','mousemove','--window',window,str(x),str(position))
        s=wait(lambda s:s.get('frame',0)>before+2,'Frame after scrollbar drag');time.sleep(.015)
        s=wait(lambda s:not s.get('cache',{}).get('pending',1),'Visible media compilation')
        row=dict(phase=phase,position=position,offset=s['preview_offset'],cache=s['cache'],source_anchor=s['source_anchor'],preview_anchor=s['preview_anchor'],mounted=s['preview_mounted'])
        if len(records)%20==0:row.update(process_tree())
        records.append(row);return s
    run('xdotool','mousemove','--window',window,str(x),str(top),'mousedown','1')
    for y in range(top,bottom+1,2):sample('cold-forward',y)
    sample('cold-forward',bottom);run('xdotool','mouseup','1');time.sleep(.15)
    forward=state();forward_tree=process_tree();run('import','-window',window,str(OUT/'native-end.png'))
    run('xdotool','mousemove','--window',window,str(x),str(bottom),'mousedown','1')
    for y in range(bottom,top-1,-2):sample('warm-return',y)
    sample('warm-return',top);run('xdotool','mouseup','1');time.sleep(.2)
    first_warm=state();first_warm_tree=process_tree()
    run('xdotool','mousemove','--window',window,str(x),str(top),'mousedown','1')
    for y in range(top,bottom+1,2):sample('second-warm-forward',y)
    sample('second-warm-forward',bottom);run('xdotool','mouseup','1');time.sleep(.2)
    final=state();final_tree=process_tree();(OUT/'state.json').write_text(json.dumps(final,indent=2)+'\n')
    report=dict(verification_id=ID,binary_sha256=SHA,nodes=args.nodes,platform=final.get('platform'),renderer=final.get('renderer'),cold_cache=forward['cache'],warm_cache=final['cache'],cold_process_tree=forward_tree,first_warm_process_tree=first_warm_tree,warm_process_tree=final_tree,records=records,seconds=time.monotonic()-began,note='RSS sums all native/Node/Chromium descendants; PSS apportions shared pages. Two complete warm revisits follow a cold traversal. OpenGL completion is not scanout/presentation. All viewport visits use the actual native scrollbar track.')
    gates=dict(all_nodes_compiled=forward['cache']['completed']>=args.nodes,no_errors=final['cache']['errors']==0,no_recompile_on_return=final['cache']['submissions']==forward['cache']['submissions'],bounded_cache=final['cache']['bytes']<=final['cache']['byte_limit'],resident_viewport_only=final['cache']['resident_textures']<30,stable_process_tree=final_tree['rss_kib']<=forward_tree['rss_kib']+64*1024,stable_proportional_memory=final_tree['pss_kib']<=first_warm_tree['pss_kib']+32*1024)
    report['gates']=gates;report['passed']=all(gates.values());(OUT/'performance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k!='records'},indent=2))
    assert report['passed'],gates
finally:
    if app and app.poll() is None:app.terminate();app.wait(timeout=15)
    for child in reversed(children):child.terminate();child.wait(timeout=5)
    STATE.unlink(missing_ok=True);EXE.unlink(missing_ok=True)
