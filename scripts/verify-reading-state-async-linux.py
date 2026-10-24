# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Release external image/renderer work while its block is above a live viewport."""
import hashlib
import http.server
import json
import os
from pathlib import Path
import struct
import subprocess
import threading
import time
import uuid
import zlib

ROOT=Path('/work')
OUT=ROOT/'evidence/reading-state/implementation'/os.environ.get('MDCARD_READING_ASYNC_RUN','linux-async-r5')
OUT.mkdir(parents=True,exist_ok=True)
EXE=Path(os.environ.get('MDCARD_READING_EXE','/work/artifacts/reading-state/linux-r6/mdcard'))
RENDERER=Path(os.environ.get('MDCARD_READING_RENDERER_DIR',str(EXE.parent/'renderer' if (EXE.parent/'renderer').is_dir() else ROOT/'dist/linux/renderer')))
ENV=dict(os.environ,DISPLAY=':153',LANG='C.UTF-8',LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',XDG_RUNTIME_DIR='/tmp/mdcard-reading-async-x11')
Path(ENV['XDG_RUNTIME_DIR']).mkdir(exist_ok=True);Path(ENV['XDG_RUNTIME_DIR']).chmod(0o700)
services=[]
app=None
window=None
current=None
release=threading.Event()
requested=threading.Event()
width,height=640,1100
def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data)&0xffffffff)
png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,2,0,0,0))+chunk(b'IDAT',zlib.compress((b'\0'+bytes([48,112,220])*width)*height))+chunk(b'IEND',b'')
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        requested.set()
        if not release.wait(25):return
        self.send_response(200);self.send_header('Content-Type','image/png');self.send_header('Content-Length',str(len(png)));self.end_headers();self.wfile.write(png)
    def log_message(self,*args):pass
server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler);server.daemon_threads=True
threading.Thread(target=server.serve_forever,daemon=True).start()

def run(*args,**kwargs):return subprocess.run(args,env=ENV,check=True,**kwargs)
def state(name='reading'):
    for _ in range(40):
        try:return json.loads((current/(name+'.json')).read_text())
        except (ValueError,OSError):time.sleep(.03)
    raise AssertionError('No '+name)
def wait(predicate,message,name='reading'):
    until=time.monotonic()+20
    while time.monotonic()<until:
        assert app.poll() is None
        s=state(name)
        if predicate(s):return s
        time.sleep(.04)
    raise AssertionError(message)
def position():return state()['views']['main/preview']['position']
def shot(name):
    run('import','-window',window,str(current/(name+'.png')))
    (current/(name+'.json')).write_text(json.dumps(state(),ensure_ascii=False,indent=2))
def start(kind,source,renderer):
    global current,app,window
    current=OUT/kind;current.mkdir(exist_ok=True)
    file=current/'async.md';file.write_text(source)
    (current/'settings.json').write_text(json.dumps(dict(language='en',appearance='Light',reduce_motion=True,font_size=15)))
    env=dict(ENV,MDCARD_SETTINGS_PATH=str(current/'settings.json'),MDCARD_READING_STATE_PATH=str(current/'reading-state.json'),MDCARD_READING_REPORT=str(current/'reading.json'),MDCARD_UI_REPORT=str(current/'ui.json'),MDCARD_RICH_REPORT=str(current/'rich.json'),MDCARD_RENDERER_DIR=str(renderer))
    app=subprocess.Popen([str(EXE),str(file)],env=env,stdout=(current/'app.log').open('wb'),stderr=subprocess.STDOUT)
    window=None
    for _ in range(100):
        time.sleep(.1);found=subprocess.run(['xdotool','search','--all','--pid',str(app.pid),'--name','MDcard'],env=env,capture_output=True)
        if found.returncode==0:window=found.stdout.decode().splitlines()[0];break
    assert window
    run('xdotool','windowsize',window,'1240','820','windowactivate','--sync',window);time.sleep(.8)
    ui=state('ui');r=next(c['rect'] for c in reversed(ui['controls']) if c['name']=='Read')
    run('xdotool','mousemove','--window',window,str(round((r[0]+r[2])/2)),str(round((r[1]+r[3])/2)),'click','1')
    wait(lambda s:s['views'].get('main/preview',{}).get('position') is not None and s['views']['main/preview']['pending'] is None,'Read initialized');time.sleep(.4)
    return file
def wheel():
    r=state()['views']['main/preview']['viewport_rect']
    run('xdotool','mousemove','--window',window,str(round((r[0]+r[2])/2)),str(round((r[1]+r[3])/2)),'click','--repeat','5','--delay','65','5');time.sleep(.7)
def stable_after(before):
    s=wait(lambda s:abs(s['views']['main/preview']['position']['offset'][1]-before['offset'][1])>100,'Real async height change did not compensate')
    samples=[]
    for _ in range(30):
        p=position();samples.append(dict(y=p['offset'][1],byte=p['anchor']['byte']));time.sleep(.1)
    assert all(p['byte']==before['anchor']['byte'] for p in samples), ('Async load changed source block',before,samples)
    tail=[p['y'] for p in samples[-15:]];assert max(tail)-min(tail)<1,('Sustained anchor drift',tail)
    return dict(offset_before=before['offset'],offset_after=position()['offset'],source_byte=before['anchor']['byte'],tail_drift_logical_px=max(tail)-min(tail),samples=samples)
def stop():
    run('xdotool','key','--clearmodifiers','ctrl+q');app.wait(timeout=8);assert app.returncode==0

try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    body=''.join(f'ASYNC-TARGET-{i:03d} 中文🙂 paragraph after delayed content. '+('Reading anchor compensation. '*4)+'\n\n' for i in range(70))
    file=start('image',f'# Async image\n\n![delayed](http://127.0.0.1:{server.server_port}/image.png)\n\n'+body,RENDERER)
    assert requested.wait(8),'Image was not requested'
    wheel();before=position();assert before['anchor']['context'].startswith('ASYNC-TARGET-'),before
    digest=hashlib.sha256(file.read_bytes()).hexdigest();shot('01-before-release');release.set()
    image_result=stable_after(before);shot('02-after-release');assert hashlib.sha256(file.read_bytes()).hexdigest()==digest;stop()
    # Delay loading the actual packaged worker module. Formula rendering itself
    # still uses the unmodified packaged Node/Chromium/KaTeX pipeline.
    renderer=OUT/'delayed-renderer';renderer.mkdir(exist_ok=True)
    (renderer/'runtime').symlink_to(RENDERER/'runtime',target_is_directory=True)
    gate=renderer/'release';started=renderer/'started'
    (renderer/'worker.mjs').write_text("import fs from 'node:fs';\nfs.writeFileSync("+json.dumps(str(started))+",'started');\nwhile(!fs.existsSync("+json.dumps(str(gate))+")) await new Promise(r=>setTimeout(r,20));\nprocess.argv[1]="+json.dumps(str(RENDERER/'worker.mjs'))+";\nawait import("+json.dumps(str(RENDERER/'worker.mjs'))+");\n")
    formula='$$\n\\begin{aligned}\n'+' \\\\\n'.join(f'a_{{{i}}} &= \\frac{{x+{i}}}{{y+{i}}}' for i in range(18))+'\n\\end{aligned}\n$$\n\n'
    file=start('formula','# Async formula\n\n'+formula+body,renderer)
    wait(lambda s:s['counters']['submissions']>0,'Formula was not submitted','rich')
    assert started.exists(),'Delayed renderer was not started'
    wheel();before=position();assert before['anchor']['context'].startswith('ASYNC-TARGET-'),before
    digest=hashlib.sha256(file.read_bytes()).hexdigest();shot('01-before-release');gate.write_text('release')
    wait(lambda s:s['counters']['completed']>0 and s['counters']['errors']==0,'Actual formula did not render','rich')
    formula_result=stable_after(before);shot('02-after-release');assert hashlib.sha256(file.read_bytes()).hexdigest()==digest;stop()
    result=dict(verification_id=str(uuid.uuid4()),passed=True,platform='Debian 12 / X11 / Xvfb / Openbox / Mesa',binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),image=image_result,formula=formula_result,manuscript_hashes_unchanged=True,precision='Same source paragraph/block; not text/line exactness')
    (OUT/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(json.dumps(result,ensure_ascii=False,indent=2))
except Exception as error:
    (OUT/'failure.json').write_text(json.dumps(dict(error=repr(error)),ensure_ascii=False,indent=2))
    if app and app.poll() is None:
        try:shot('failure')
        except Exception:pass
    raise
finally:
    release.set();server.shutdown();server.server_close()
    if app and app.poll() is None:app.kill();app.wait(timeout=5)
    for p in reversed(services):
        p.terminate()
        try:p.wait(timeout=5)
        except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)
