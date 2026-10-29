# SPDX-License-Identifier: AGPL-3.0-only
"""Real IBus/libpinyin XIM preedit and candidate commit on installed Linux app."""
import ast,ctypes,hashlib,json,os,struct,subprocess,time,zlib
from pathlib import Path
out=Path(os.environ.get('MDCARD_QA_IME_OUTPUT','/qa-output/installed-linux-ime'));out.mkdir(exist_ok=True)
module=ast.parse(Path('/qa/verify-installed-linux.py').read_text());exec(compile(ast.Module(body=[n for n in module.body if isinstance(n,ast.FunctionDef) and n.name=='capture'],type_ignores=[]),'<qa-capture>','exec'))
env=dict(os.environ,DISPLAY=':98',LANG='C.UTF-8',XDG_CURRENT_DESKTOP='Openbox',XDG_RUNTIME_DIR='/tmp/mdcard-015-ime',XMODIFIERS='@im=ibus',GTK_IM_MODULE='ibus',QT_IM_MODULE='ibus',IBUS_ADDRESS='unix:path=/tmp/mdcard-015-ime/ibus.sock',IBUS_USE_PORTAL='0',LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2');Path(env['XDG_RUNTIME_DIR']).mkdir(mode=0o700,exist_ok=True)
profile=Path.home()/'qa'/out.name;settings=profile/'config/MDcard/settings.json';settings.parent.mkdir(parents=True,exist_ok=True);settings.write_text(json.dumps(dict(language='en',font_size=15,reduce_motion=True)))
ui=out/'ui.json';document=out/'IBus 中文.md';document.write_text('# Real IBus Pinyin\n\n');services=[];app=None
def run(*args,**kwargs):
 p=subprocess.run(args,env=env,capture_output=True,**kwargs)
 if p.returncode:
  with (out/'command-errors.log').open('a') as log:log.write(repr(args)+'\n'+p.stdout.decode(errors='replace')+'\n'+p.stderr.decode(errors='replace')+'\n')
  p.check_returncode()
 return p
def read():return json.loads(ui.read_text())
try:
 services.append(subprocess.Popen(['Xvfb',':98','-screen','0','1600x1200x24'],env=env,stdout=(out/'xvfb.log').open('w'),stderr=subprocess.STDOUT));time.sleep(.5);services.append(subprocess.Popen(['openbox'],env=env,stdout=(out/'openbox.log').open('w'),stderr=subprocess.STDOUT));time.sleep(.5)
 run('dbus-update-activation-environment','DISPLAY','XDG_RUNTIME_DIR','XDG_CURRENT_DESKTOP','XMODIFIERS','GTK_IM_MODULE','QT_IM_MODULE')
 Path('/tmp/mdcard-015-ime/ibus.sock').unlink(missing_ok=True)
 run('ibus-daemon','--daemonize','--xim','--replace','--panel=disable','--address='+env['IBUS_ADDRESS']);time.sleep(2)
 (out/'engines.txt').write_bytes(run('ibus','list-engine').stdout)
 app=subprocess.Popen(['/opt/mdcard/mdcard',str(document)],env={**env,'PATH':'','XDG_CONFIG_HOME':str(profile/'config'),'XDG_STATE_HOME':str(profile/'state'),'MDCARD_UI_REPORT':str(ui)},cwd=out,stdout=(out/'app.log').open('w'),stderr=subprocess.STDOUT);time.sleep(2)
 window=run('xdotool','search','--all','--pid',str(app.pid),'--name','MDcard').stdout.decode().splitlines()[0];run('xdotool','windowsize',window,'1400','1100','windowactivate','--sync',window);time.sleep(.3)
 def control(name):
  for _ in range(6):
   c=next(c for c in read()['controls'] if c['name']==name);r=c['rect'];clip=c['clip'];x=max(r[0],clip[0])+40 if name=='main-source-editor' else (r[0]+r[2])/2;y=max(r[1],clip[1])+14 if name=='main-source-editor' else (r[1]+r[3])/2
   run('xdotool','mousemove','--window',window,str(round(x)),str(round(y)));time.sleep(.2)
   if next(c for c in read()['controls'] if c['name']==name)['rect']!=r:continue
   run('xdotool','mousedown','1');time.sleep(.06);run('xdotool','mouseup','1');time.sleep(.2);return
  raise RuntimeError('Unstable geometry')
 control('Write');control('main-source-editor');run('xdotool','key','--clearmodifiers','ctrl+End');time.sleep(.2)
 for attempt in range(8):
  selected=subprocess.run(['ibus','engine','libpinyin'],env=env,capture_output=True)
  with (out/'engine-selection.log').open('a') as log:log.write(str(attempt)+' '+str(selected.returncode)+' '+selected.stdout.decode(errors='replace')+' '+selected.stderr.decode(errors='replace')+'\n')
  if selected.returncode==0:break
  time.sleep(.5)
 if selected.returncode:
  import gi
  os.environ.update(IBUS_ADDRESS=env['IBUS_ADDRESS'],IBUS_USE_PORTAL='0',DISPLAY=':98')
  gi.require_version('IBus','1.0')
  from gi.repository import IBus
  IBus.init();bus=IBus.Bus();accepted=bus.set_global_engine('libpinyin');engine=bus.get_global_engine()
  (out/'ibus-direct-probe.json').write_text(json.dumps(dict(connected=bus.is_connected(),setAccepted=accepted,engine=engine.get_name() if engine else None),indent=2))
  assert accepted and engine and engine.get_name()=='libpinyin','IBus did not activate the installed engine'
 run('xdotool','type','--clearmodifiers','--delay','120','nihao');time.sleep(.5);preedit=read();capture(out/'01-real-ibus-preedit.png')
 run('xdotool','key','--clearmodifiers','space');time.sleep(.4);run('xdotool','key','--clearmodifiers','ctrl+s');time.sleep(.5);text=document.read_text();capture(out/'02-real-ibus-committed.png')
 result=dict(passed=preedit['autosave']['ime'] and '你好' in text,preeditActive=preedit['autosave']['ime'],actualText=text,input='xdotool ASCII key events n i h a o and Space; no Unicode paste or synthesized egui IME event',provider=run('ibus','engine').stdout.decode().strip(),binarySHA256=hashlib.file_digest(Path('/opt/mdcard/mdcard').open('rb'),'sha256').hexdigest(),network='Docker --network none',system='Debian12 x64/X11/Openbox/IBus libpinyin',productPathEmpty=True)
 (out/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(json.dumps(result,ensure_ascii=False));assert result['passed']
 run('xdotool','key','--clearmodifiers','ctrl+q');app.wait(timeout=10)
finally:
 if app and app.poll() is None:app.terminate();app.wait(timeout=10)
 for service in reversed(services):
  if service.poll() is None:service.terminate();service.wait(timeout=10)
