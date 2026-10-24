#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Real native pointer/keyboard verification on a private X11 display."""
import hashlib,json,os,subprocess,time,uuid
from pathlib import Path
ROOT=Path('/work');OUT=Path(os.environ.get('MDCARD_RICH_OUTPUT',str(ROOT/'evidence/rich/linux')));OUT.mkdir(parents=True,exist_ok=True)
EXE=Path(os.environ.get('MDCARD_RICH_EXE','/work/target/release/mdcard'))
ENV=dict(os.environ,DISPLAY=os.environ.get('MDCARD_RICH_DISPLAY',':113'),LANG='C.UTF-8',LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',MDCARD_RENDERER_DIR='/work/renderer',MDCARD_SETTINGS_PATH=str(OUT/'settings.json'),MDCARD_RICH_REPORT=str(OUT/'report.json'),XDG_RUNTIME_DIR='/tmp/mdcard-rich-runtime')
Path(ENV['XDG_RUNTIME_DIR']).mkdir(exist_ok=True);Path(ENV['XDG_RUNTIME_DIR']).chmod(0o700)
app=None;children=[];window=None
def run(*args,**kw):return subprocess.run(args,env=ENV,check=True,**kw)
def key(*keys):run('xdotool','key','--clearmodifiers',*keys);time.sleep(.2)
def click(x,y):run('xdotool','mousemove','--window',window,str(round(x)),str(round(y)),'click','1');time.sleep(.2)
def report():
 for _ in range(25):
  try:return json.loads((OUT/'report.json').read_text())
  except (OSError,json.JSONDecodeError):time.sleep(.02)
 raise RuntimeError('No current native report')
def rect(name):return next(c['rect'] for c in reversed(report()['controls']) if c['name']==name)
def control(name):r=rect(name);click((r[0]+r[2])/2,(r[1]+r[3])/2)
def paste(value):run('xclip','-selection','clipboard','-loops','1',input=value.encode(),stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);key('ctrl+v');time.sleep(.5)
def shot(name):time.sleep(.4);run('import','-window',window,str(OUT/f'{name}.png'))
def saved(path):key('ctrl+s');return path.read_bytes()
def launch(path,language='en',appearance='Light'):
 global app,window
 (OUT/'settings.json').write_text(json.dumps(dict(language=language,appearance=appearance,gpu_acceleration=True,reduce_motion=True,font_size=15)))
 app=subprocess.Popen([str(EXE),str(path)],env=ENV,stdout=(OUT/'app.log').open('ab'),stderr=subprocess.STDOUT)
 for _ in range(80):
  time.sleep(.15);r=subprocess.run(['xdotool','search','--pid',str(app.pid),'--name','MDcard'],env=ENV,capture_output=True)
  if r.returncode==0:window=r.stdout.decode().splitlines()[0];break
 else:raise RuntimeError('No native window')
 run('xdotool','windowsize',window,'1240','820','windowactivate','--sync',window);time.sleep(2)
def close():
 global app
 key('ctrl+s','ctrl+q');app.wait(timeout=5);assert app.returncode==0;app=None
def stats(values):
 v=sorted(values);return dict(count=len(v),p95=v[min(len(v)-1,int(.95*len(v)))],maximum=max(v))
try:
 children.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.7)
 children.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.7)
 fixture=OUT/'fixture.md';original='# Formula and diagram\n\nBefore $x^2$ beside $\\alpha+\\beta$ after. Cost: $100 and $200.\n\n```mermaid\nflowchart LR\nA[Source] --> B[Preview]\n```\n\n```math\n\\ce{2H2 + O2 -> 2H2O}\n```\n';fixture.write_bytes(original.encode())
 launch(fixture);time.sleep(5);shot('01-preview');assert report()['counters']['completed']==4;assert report()['counters']['errors']==0;assert saved(fixture)==original.encode()
 control('node-2-Source');control('node-2-editor');key('ctrl+a');paste('flowchart LR\nA[Alpha] --> B[Beta]');edited=saved(fixture);assert b'A[Alpha] --> B[Beta]' in edited
 key('ctrl+z');assert saved(fixture)==original.encode();key('ctrl+shift+z');assert saved(fixture)==edited
 control('node-2-Split');time.sleep(2);shot('02-editable-split');control('node-2-Preview');r=rect('node-2-Preview');cx=(r[0]+r[2])/2+80;cy=r[3]+70;click(cx,cy)
 run('xdotool','click','--repeat','4','--delay','50','4');run('xdotool','mousedown','1');time.sleep(.15)
 for _ in range(9):run('xdotool','mousemove_relative','--','5','2');time.sleep(.04)
 run('xdotool','mouseup','1');time.sleep(.5)
 view=next(v for v in report()['viewStates'] if v['index']==2);assert view['zoom']>1.1 and any(view['pan']);assert saved(fixture)==edited;shot('03-svg-zoom-pan')
 control('node-0');control('floating-editor');key('ctrl+a');paste('z^3');key('Escape');formula=saved(fixture);assert b'$z^3$' in formula;key('ctrl+z');assert saved(fixture)==edited;key('ctrl+shift+z');assert saved(fixture)==formula;shot('04-formula-edited')
 stable=report()['counters']['submissions'];time.sleep(1);assert report()['counters']['submissions']==stable
 small_cpu=stats(report()['updateCpuMs']);close()
 locales=[]
 for language in ['zh-CN','zh-TW','en','ko','ja','ru']:
  launch(fixture,language,'Dark');time.sleep(4);shot('locale-'+language);assert saved(fixture)==formula;assert report()['counters']['errors']==0;locales.append(language);close()
 lazy=OUT/'lazy.md';lazy.write_text('# Lazy diagrams\n\n'+''.join(f'```mermaid\nflowchart LR\nA[Node {i}] --> B[Preview]\n```\n\n' for i in range(40)))
 launch(lazy);time.sleep(4);first=report();assert first['nodes']==40 and first['counters']['offscreen_nodes']>0 and first['counters']['submissions']<8
 stable=first['counters']['submissions'];time.sleep(1);assert report()['counters']['submissions']==stable;click(1210,700);run('xdotool','click','--repeat','35','--delay','20','5');time.sleep(4);after=report();assert after['counters']['submissions']>stable;shot('lazy-scrolled');close()
 korean=OUT/'korean.md';korean.write_text('```mermaid\nflowchart LR\nA[한글 수식] --> B[다이어그램]\n```\n');launch(korean,'ko','Dark');time.sleep(4);assert report()['counters']['completed']==1 and report()['counters']['errors']==0;shot('korean-svg');assert saved(korean)==korean.read_bytes();close()
 performance=[]
 for label,count,repeats in [('representative',2000,1),('one-mib',5000,20)]:
  large=OUT/(label+'.md');initial='# Long document\n\n'+''.join(f'Paragraph {i}: inline $x_{{{i}}}^2$ and ordinary `code {i}` text. '+('Content '*repeats)+'\n\n' for i in range(count));large.write_text(initial);assert label!='one-mib' or large.stat().st_size>=1048576
  launch(large);time.sleep(3);click(430,230);key('ctrl+Home');typed='abcdefghijklmnopqrstuvwxyz'*4
  expected=initial
  timings={}
  for mode in ['split','edit']:
   if mode=='edit':key('ctrl+e');key('ctrl+e');click(430,230);key('ctrl+Home')
   before=report();run('xdotool','type','--clearmodifiers','--delay','60',typed);time.sleep(.7)
   expected=typed+expected;key('ctrl+s')
   for _ in range(150):
    if large.read_text()==expected:break
    time.sleep(.1)
   assert large.read_text()==expected, 'Real typing lost or changed characters'
   current=report();samples=current['inputCpuMs'][len(before['inputCpuMs']):];assert len(samples)>=20
   interactions=current['interactionCpuMs'][len(before['interactionCpuMs']):]
   timings[mode+'InputCpuMs']=stats(samples);timings[mode+'InteractionCpuMs']=stats(interactions)
   click(430 if mode=='edit' else 1200,700);before=report();run('xdotool','click','--repeat','30','--delay','35','5');time.sleep(.7);after_scroll=report();samples=after_scroll['scrollCpuMs'][len(before['scrollCpuMs']):];assert samples
   timings[mode+'ScrollCpuMs']=stats(samples)
   shot(label+'-'+mode)
  performance.append(dict(label=label,bytes=large.stat().st_size,nodes=count,typedCharacters=len(typed)*2,exactSourceSaved=True,metrics=timings));close()
 result=dict(runId=str(uuid.uuid4()),date=time.strftime('%Y-%m-%dT%H:%M:%S%z'),passed=True,exeSha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),environment='Debian 12 / X11 Xvfb :113 / Openbox / llvmpipe OpenGL / Node 24 / Chromium 155 / Java 17',sourceEditing=True,sharedUndoRedo=True,vectorZoomPan=view,locales=locales,lazyInitial=first['counters'],lazyAfterScroll=after['counters'],smallUpdateCpuMs=small_cpu,performance=performance,measurement='eframe update CPU; excludes GPU presentation and input transport; source editor and preview layout included')
 (OUT/'result.json').write_text(json.dumps(result,indent=2));print(json.dumps(result,indent=2))
finally:
 if app:
  app.terminate()
  try:app.wait(timeout=3)
  except subprocess.TimeoutExpired:app.kill();app.wait()
 for child in reversed(children):
  child.terminate()
  try:child.wait(timeout=3)
  except subprocess.TimeoutExpired:child.kill();child.wait()
