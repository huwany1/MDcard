# SPDX-License-Identifier: AGPL-3.0-only
"""Native X11 installed-application acceptance and three comparable A/B trials.

External QA only: Python/Xvfb/Openbox/xdotool/xclip are test equipment, not
product dependencies. Run via dbus-run-session as a normal desktop user.
"""
import ctypes,hashlib,json,os,struct,subprocess,time,traceback,zlib
from pathlib import Path
out=Path(os.environ.get('MDCARD_QA_OUTPUT','/qa-output/installed-linux-gui'));out.mkdir(parents=True,exist_ok=True)
env={k:v for k,v in os.environ.items() if not k.startswith('MDCARD_')}
env.update(DISPLAY=':98',LANG='C.UTF-8',LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',XDG_RUNTIME_DIR='/tmp/mdcard-015-x11',XDG_CURRENT_DESKTOP='Openbox')
Path(env['XDG_RUNTIME_DIR']).mkdir(mode=0o700,exist_ok=True)
services=[];app=None;records=[]
def run(*args,**kwargs):return subprocess.run(args,env=env,check=True,**kwargs)
def read(path):
 for _ in range(100):
  try:return json.loads(path.read_text())
  except (OSError,ValueError):time.sleep(.02)
 raise RuntimeError('Missing diagnostic '+str(path))
def wait(predicate,message,seconds=60):
 until=time.monotonic()+seconds
 while time.monotonic()<until:
  if predicate():return
  if app and app.poll() is not None:raise RuntimeError('Application exited early')
  time.sleep(.05)
 raise RuntimeError(message)
def key(*keys):run('xdotool','key','--clearmodifiers',*keys);time.sleep(.2)
def paste(text):run('xclip','-selection','clipboard',input=text.encode());time.sleep(.08);key('ctrl+v')
def capture(target):
 class XImage(ctypes.Structure):
  _fields_=[('width',ctypes.c_int),('height',ctypes.c_int),('xoffset',ctypes.c_int),('format',ctypes.c_int),('data',ctypes.c_void_p),('byte_order',ctypes.c_int),('bitmap_unit',ctypes.c_int),('bitmap_bit_order',ctypes.c_int),('bitmap_pad',ctypes.c_int),('depth',ctypes.c_int),('bytes_per_line',ctypes.c_int),('bits_per_pixel',ctypes.c_int),('red_mask',ctypes.c_ulong),('green_mask',ctypes.c_ulong),('blue_mask',ctypes.c_ulong)]
 lib=ctypes.CDLL('libX11.so.6');lib.XOpenDisplay.argtypes=[ctypes.c_char_p];lib.XOpenDisplay.restype=ctypes.c_void_p;display=lib.XOpenDisplay(b':98')
 lib.XDefaultRootWindow.argtypes=[ctypes.c_void_p];lib.XDefaultRootWindow.restype=ctypes.c_ulong
 lib.XGetImage.argtypes=[ctypes.c_void_p,ctypes.c_ulong,ctypes.c_int,ctypes.c_int,ctypes.c_uint,ctypes.c_uint,ctypes.c_ulong,ctypes.c_int];lib.XGetImage.restype=ctypes.POINTER(XImage)
 img=lib.XGetImage(display,lib.XDefaultRootWindow(display),0,0,1600,1200,ctypes.c_ulong(-1).value,2);i=img.contents
 assert i.bits_per_pixel==32 and i.red_mask==0xff0000
 raw=ctypes.string_at(i.data,i.bytes_per_line*i.height);rows=[]
 for y in range(i.height):
  line=raw[y*i.bytes_per_line:y*i.bytes_per_line+i.width*4];rgb=bytearray(i.width*3);rgb[0::3]=line[2::4];rgb[1::3]=line[1::4];rgb[2::3]=line[0::4];rows.append(b'\0'+rgb)
 def chunk(tag,data):return struct.pack('>I',len(data))+tag+data+struct.pack('>I',zlib.crc32(tag+data)&0xffffffff)
 target.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',i.width,i.height,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(b''.join(rows)))+chunk(b'IEND',b''))
 lib.XDestroyImage.argtypes=[ctypes.POINTER(XImage)];lib.XDestroyImage(img);lib.XCloseDisplay.argtypes=[ctypes.c_void_p];lib.XCloseDisplay(display)
try:
 assert os.getuid()==1000
 services.append(subprocess.Popen(['Xvfb',':98','-screen','0','1600x1200x24'],env=env,stdout=(out/'xvfb.log').open('w'),stderr=subprocess.STDOUT));time.sleep(.5)
 services.append(subprocess.Popen(['openbox'],env=env,stdout=(out/'openbox.log').open('w'),stderr=subprocess.STDOUT));time.sleep(.5)
 run('dbus-update-activation-environment','DISPLAY','XDG_RUNTIME_DIR','XDG_CURRENT_DESKTOP')
 cases=[('installed',Path('/opt/mdcard'))] if os.environ.get('MDCARD_QA_LIFECYCLE_ONLY') else [('baseline',Path('/home/reader/baseline')),('installed',Path('/opt/mdcard'))]
 for kind,stage in cases:
  for trial in ([1] if os.environ.get('MDCARD_QA_LIFECYCLE_ONLY') else range(1,4)):
   work=out/f'{kind}-{trial}';work.mkdir();profile=Path.home()/'qa'/out.name/f'{kind}-{trial}';profile.mkdir(parents=True)
   settings=profile/'config/MDcard/settings.json';settings.parent.mkdir(parents=True);settings.write_text(json.dumps(dict(language='en',reduce_motion=True,font_size=15)))
   ui=work/'ui.json';rich=work/'rich.json';search=work/'search.json';export=work/'export.json'
   appenv={**env,'PATH':'','XDG_CONFIG_HOME':str(profile/'config'),'XDG_STATE_HOME':str(profile/'state'),'XDG_CACHE_HOME':str(profile/'cache'),'MDCARD_UI_REPORT':str(ui),'MDCARD_RICH_REPORT':str(rich),'MDCARD_SEARCH_REPORT':str(search),'MDCARD_EXPORT_REPORT':str(export)}
   document=work/'中文 场景A.md';document.write_text('# First use\n\nInitial.\n')
   open_dialog=bool(os.environ.get('MDCARD_QA_OPEN_DIALOG'))
   begin=time.monotonic();app=subprocess.Popen([str(stage/'mdcard')]+([] if open_dialog else [str(document)]),cwd=work,env=appenv,stdout=(work/'app.log').open('w'),stderr=subprocess.STDOUT)
   wait(lambda:ui.exists(),'No first UI report');startup=time.monotonic()-begin
   window=run('xdotool','search','--all','--pid',str(app.pid),'--name','MDcard',capture_output=True).stdout.decode().splitlines()[0]
   run('xdotool','windowsize',window,'1400','1100','windowactivate','--sync',window);time.sleep(.5)
   def click(x,y):run('xdotool','mousemove','--window',window,str(round(x)),str(round(y)),'click','1');time.sleep(.2)
   def control(name):
    for attempt in range(6):
     c=next(c for c in read(ui)['controls'] if c['name']==name);r=c['rect'];clip=c['clip'];l=max(r[0],clip[0]);t=max(r[1],clip[1]);right=min(r[2],clip[2]);bottom=min(r[3],clip[3]);assert l<right and t<bottom,(name,c)
     x=l+40 if name=='main-source-editor' else (l+right)/2;y=t+14 if name=='main-source-editor' else (t+bottom)/2
     run('xdotool','mousemove','--window',window,str(round(x)),str(round(y)));time.sleep(.2)
     current=next(c for c in read(ui)['controls'] if c['name']==name)
     if current['rect']!=r:continue
     run('xdotool','mousedown','1');time.sleep(.08);run('xdotool','mouseup','1');time.sleep(.2);return
    raise RuntimeError('Control geometry did not stabilize: '+name)
   if open_dialog:
    key('ctrl+o');time.sleep(.7);key('ctrl+l');paste(str(document));key('Return');wait(lambda:read(ui)['document_path']==str(document),'Native Open did not choose the scenario document')
   control('Write');control('main-source-editor');key('ctrl+End');paste('\nEdited 中文 and English.');key('ctrl+s')
   wait(lambda:'Edited 中文 and English.' in document.read_text() and not read(ui)['dirty'],'Scene A did not save');a_seconds=time.monotonic()-begin;capture(work/'01-edited-saved.png')
   key('ctrl+f');paste('Edited');wait(lambda:read(search).get('results',0)>0,'Search did not find text');key('Escape')
   control('Write');control('main-source-editor');key('ctrl+a')
   full='# Full offline use\n\n$x^2+1$\n\n```mermaid\ngraph LR\n A --> B\n```\n\n```plantuml\nAlice -> Bob: 中文\n```\n\n```flow\nst=>start: Start\ne=>end: End\nst->e\n```\n'
   full_begin=time.monotonic();paste(full);key('ctrl+s');control('Split');wait(lambda:read(rich).get('cachedResults',0)>=4,'Four local engine preview results not ready');preview=time.monotonic()-full_begin;capture(work/'02-local-preview.png')
   key('ctrl+alt+e');wait(lambda:export.exists() and read(export)['open'],'No native export dialog')
   r=read(export)['controls']['export'];click(r['x']+r['w']/2,r['y']+r['h']/2);time.sleep(.8)
   pdf=work/'中文 完整导出.pdf';key('ctrl+a');paste(str(pdf));key('Return')
   wait(lambda:(read(export).get('last') or {}).get('type')=='complete','Native PDF export did not finish');assert pdf.read_bytes().startswith(b'%PDF-');b_seconds=time.monotonic()-full_begin;capture(work/'03-native-pdf.png')
   records.append(dict(kind=kind,trial=trial,binary=str(stage/'mdcard'),sha256=hashlib.file_digest((stage/'mdcard').open('rb'),'sha256').hexdigest(),startupSeconds=startup,scenarioAProcessToSavedSeconds=a_seconds,scenarioBAdditionalSeconds=b_seconds,previewReadySeconds=preview,guiPdf=str(pdf),searchPassed=True,nativeOpenDialog=open_dialog,documentPassedAsArgument=not open_dialog,chineseInput='Unicode clipboard paste; no Linux IME composition claim',network='Docker --network none',environment='Debian12 x64/Openbox/Xvfb/software Mesa; no system developer runtimes; native app PATH empty'))
   key('Escape');key('ctrl+q');app.wait(timeout=10);assert app.returncode==0;app=None
   (out/'measurements.json').write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n')
 # Additional lifecycle features use only the installed product and this QA
 # user's XDG profile. No fault-hook binary is substituted.
 document=work/'lifecycle.md';document.write_text('# Lifecycle 中文\n')
 def launch_lifecycle():
  global app,window
  app=subprocess.Popen(['/opt/mdcard/mdcard',str(document)],cwd=work,env=appenv,stdout=(work/'lifecycle-app.log').open('a'),stderr=subprocess.STDOUT)
  time.sleep(1)
  window=run('xdotool','search','--all','--pid',str(app.pid),'--name','MDcard',capture_output=True).stdout.decode().splitlines()[0]
  run('xdotool','windowsize',window,'1400','1100','windowactivate','--sync',window);time.sleep(.3)
 def save_as(target):
  key('ctrl+shift+s');time.sleep(.8);key('ctrl+a');paste(str(target));key('Return');wait(lambda:target.exists(),'Native Save As did not create file');wait(lambda:not read(ui)['dirty'],'Save As stayed dirty')
 launch_lifecycle();control('Write');control('main-source-editor');key('ctrl+End');paste('Saved through native Save As.')
 saved_as=work/'中文 另存为.md';save_as(saved_as);assert 'Saved through native Save As.' in saved_as.read_text();capture(work/'04-save-as.png')
 key('ctrl+comma');wait(lambda:read(ui)['settings_open'],'Preferences did not open');control('Auto save');wait(lambda:read(ui)['autosave']['enabled'],'Auto save switch was not actually enabled');key('Escape');control('Write');control('main-source-editor');key('ctrl+End');paste('\nAutomatic save 中文.');wait(lambda:'Automatic save 中文.' in saved_as.read_text() and not read(ui)['dirty'],'Installed Linux autosave did not commit');capture(work/'05-autosave.png')
 key('ctrl+comma');wait(lambda:read(ui)['settings_open'],'Preferences did not open');control('Auto save');wait(lambda:not read(ui)['autosave']['enabled'],'Auto save switch was not actually disabled');key('Escape');control('Write');control('main-source-editor');key('ctrl+End');paste('\nUnsaved 中文 crash recovery.')
 wait(lambda:read(ui)['recovery']['main']['current_version'] and read(ui)['recovery']['main']['committed_at']>0,'Recovery checkpoint not current')
 expected=saved_as.read_text()+'\nUnsaved 中文 crash recovery.';app.kill();app.wait(timeout=10);app=None;assert 'Unsaved 中文 crash recovery.' not in saved_as.read_text()
 launch_lifecycle();wait(lambda:read(ui)['recovery']['pending']>0,'No recovery offered after forced termination')
 if not read(ui)['recovery']['open']:control('recovery-open')
 control('recovery-restore-0');wait(lambda:read(ui)['dirty'] and read(ui)['document_path'] is None,'Recovery was not an unsaved copy');control('Write');control('main-source-editor');key('ctrl+a');key('ctrl+c')
 actual=run('xclip','-o','-selection','clipboard',capture_output=True).stdout.decode();assert actual==expected,(repr(actual),repr(expected));capture(work/'06-crash-recovered.png')
 recovered=work/'中文 恢复副本.md';save_as(recovered);assert recovered.read_text()==expected;assert 'Unsaved 中文 crash recovery.' not in saved_as.read_text()
 key('ctrl+q');app.wait(timeout=10);assert app.returncode==0;app=None
 (out/'lifecycle.json').write_text(json.dumps(dict(installedRoot='/opt/mdcard',nativeSaveAs=True,autosaveOptIn=True,crashRecoveryUnsavedCopy=True,forcedTermination='kill only the launched QA app',sourcePreserved=True,chineseUtf8Exact=True,network='Docker --network none',recoveredSHA256=hashlib.file_digest(recovered.open('rb'),'sha256').hexdigest()),indent=2)+'\n')
except Exception:
 (out/'failure.txt').write_text(traceback.format_exc());print(traceback.format_exc())
 try:capture(out/'failure.png')
 except Exception:pass
 raise
finally:
 if app and app.poll() is None:app.terminate();app.wait(timeout=10)
 for service in reversed(services):
  if service.poll() is None:service.terminate();service.wait(timeout=10)
