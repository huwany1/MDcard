#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Verify lazy native code pages and cache reuse through the real scrollbar."""
import argparse,hashlib,json,os,shutil,subprocess,time,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];ap=argparse.ArgumentParser();ap.add_argument('--out',type=Path,default=ROOT/'evidence/performance/code');args=ap.parse_args();OUT=args.out.resolve();OUT.mkdir(parents=True,exist_ok=True)
ID=str(uuid.uuid4());EXE=Path('/tmp')/('mdcard-code-'+ID);shutil.copy2(ROOT/'target/release/mdcard',EXE);SHA=hashlib.file_digest(EXE.open('rb'),'sha256').hexdigest();STATE=Path('/tmp')/('mdcard-code-'+ID+'.json')
ENV=dict(os.environ,DISPLAY=':140',LANG='C.UTF-8',LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',XDG_RUNTIME_DIR='/tmp/mdcard-code-runtime',MDCARD_SETTINGS_PATH=str(OUT/'settings.json'),MDCARD_PERFORMANCE_REPORT=str(STATE));Path(ENV['XDG_RUNTIME_DIR']).mkdir(exist_ok=True);Path(ENV['XDG_RUNTIME_DIR']).chmod(0o700)
def run(*a,**kw):return subprocess.run(a,env=ENV,check=True,**kw)
def state():
    try:return json.loads(STATE.read_text())
    except (OSError,ValueError):return {}
def wait(test,label):
    until=time.monotonic()+20
    while time.monotonic()<until:
        s=state()
        if test(s):return s
        if app and app.poll() is not None:raise AssertionError('Application exited: '+label)
        time.sleep(.005)
    raise AssertionError(label+': '+str(state().get('cache')))
children=[];app=None
try:
    body='/* Comment remains open across many virtual pages.\n'+''.join(f'row {i:05d}: $inside$ is literal source, with "text" and 中文.\n' for i in range(12000))+'*/\nfn main() { let number = 42; }\n'
    fixture=OUT/'long-code.md';fixture.write_text('# Huge code\n\n```rust\n'+body+'```\n');(OUT/'settings.json').write_text(json.dumps(dict(language='en',appearance='Light',gpu_acceleration=True,reduce_motion=True,font_size=15)))
    children.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.3);children.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.3)
    app=subprocess.Popen([str(EXE),str(fixture)],env=ENV,stdout=(OUT/'app.log').open('wb'),stderr=subprocess.STDOUT)
    initial=wait(lambda s:s.get('preview_scrollbar') and s.get('cache',{}).get('code',{}).get('entries',0)>0,'Initial lazy highlighting')
    window=run('xdotool','search','--onlyvisible','--pid',str(app.pid),'--name','MDcard',capture_output=True).stdout.decode().splitlines()[0];run('xdotool','windowsize',window,'1420','950','windowfocus',window);time.sleep(.2)
    initial=state();r=initial['preview_scrollbar'];x=round((r[0]+r[2])/2);top=round(r[1]+5);bottom=round(r[3]-5)
    run('xdotool','mousemove','--window',window,str(x),str(bottom),'click','1')
    end=wait(lambda s:s.get('preview_range',[0,0])[1]>=s.get('preview_blocks',1) and s['cache']['code']['entries']>initial['cache']['code']['entries'] and s['cache']['code']['pending']==0,'End pages highlighted')
    run('xdotool','mousemove','--window',window,str(x),str(top),'click','1');time.sleep(.3)
    final=wait(lambda s:s.get('preview_offset',1e9)<1 and s['cache']['code']['pending']==0,'Return to cached beginning')
    gates=dict(lazy_initial=0<initial['cache']['code']['submissions']<10,bounded_end_work=end['cache']['code']['submissions']<12,return_reuses=final['cache']['code']['submissions']==end['cache']['code']['submissions'],bounded_cache=final['cache']['code']['bytes']<=final['cache']['code']['byte_limit'],bounded_worker=final['cache']['code']['worker_checkpoint_estimated_bytes']<=final['cache']['code']['worker_checkpoint_estimate_limit'],no_false_math=final['cache']['submissions']==0)
    report=dict(verification_id=ID,binary_sha256=SHA,code_lines=body.count('\n'),code_bytes=len(body.encode()),preview_blocks=final['preview_blocks'],initial_cache=initial['cache']['code'],end_cache=end['cache']['code'],warm_cache=final['cache']['code'],gates=gates,passed=all(gates.values()));(OUT/'performance.json').write_text(json.dumps(report,indent=2)+'\n');(OUT/'state.json').write_text(json.dumps(final,indent=2)+'\n');run('import','-window',window,str(OUT/'native-code.png'));print(json.dumps(report,indent=2));assert report['passed'],gates
finally:
    if app and app.poll() is None:app.terminate();app.wait(timeout=15)
    for child in reversed(children):child.terminate();child.wait(timeout=5)
    STATE.unlink(missing_ok=True);EXE.unlink(missing_ok=True)
