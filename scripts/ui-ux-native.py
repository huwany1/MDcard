# SPDX-License-Identifier: AGPL-3.0-only
"""Native egui screenshots and UI actions on this task's dedicated Xvfb display.

Run inside mdcard-ui-native only. DISPLAY=:141 belongs to this task. No Windows input.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from datetime import datetime, timezone

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'evidence/ui-ux/2026-10-28'
STATE=OUT/'native-state.json'
OUT.mkdir(parents=True,exist_ok=True)
state=json.loads(STATE.read_text()) if STATE.exists() else {}
ENV=dict(os.environ,DISPLAY=':141',LANG='C.UTF-8',LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',XDG_RUNTIME_DIR='/tmp/mdcard-ui-ux-runtime')
Path(ENV['XDG_RUNTIME_DIR']).mkdir(exist_ok=True);Path(ENV['XDG_RUNTIME_DIR']).chmod(0o700)

def run(*args,**kwargs):
    return subprocess.run(args,env=ENV,check=True,capture_output=kwargs.pop('capture_output',False),**kwargs)

def read(path):
    for _ in range(20):
        try:return json.loads(Path(path).read_text(encoding='utf8'))
        except (ValueError,OSError):time.sleep(.05)
    return {}

def key(value):
    # Only the dedicated :141 X server receives these keys. Keep native dialog
    # focus when a file picker owns it instead of forcing focus to its parent.
    run('xdotool','key','--clearmodifiers',value);time.sleep(.25)

def click(x,y):
    scale=read(state['ui']).get('pixels_per_point',1.0)
    run('xdotool','mousemove','--window',state['window'],str(round(x*scale)),str(round(y*scale)),'click','1');time.sleep(.35)

def control(name,kind='ui'):
    rows=read(state[kind]).get('controls',[])
    if isinstance(rows,dict):
        r=rows.get(name)
        if isinstance(r,dict):r=[r['x'],r['y'],r['x']+r['w'],r['y']+r['h']]
    else:
        matches=[v for v in rows if v['name']==name and v.get('enabled',True) and ('clip' not in v or (v['rect'][1]<v['clip'][3] and v['rect'][3]>v['clip'][1]))]
        item=matches[-1] if matches else None
        r=item['rect'] if item else None
        if item and 'clip' in item:r=[max(r[0],item['clip'][0]),max(r[1],item['clip'][1]),min(r[2],item['clip'][2]),min(r[3],item['clip'][3])]
    if not r:raise RuntimeError(f'Cannot find control {kind}:{name}')
    click((r[0]+r[2])/2,(r[1]+r[3])/2)

def paste(value):
    run('xclip','-selection','clipboard','-loops','1',input=value.encode(),stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    key('ctrl+v')

def shot(name,notes='',root=False):
    time.sleep(.6);folder=OUT/state['phase'];folder.mkdir(parents=True,exist_ok=True)
    image=folder/(name+'.png')
    run('xdotool','mousemove','1595','1095')
    run('import','-window','root' if root else state['window'],str(image))
    actual_ui=read(state['ui']);scale=actual_ui.get('pixels_per_point',1.0)
    physical=[int(x) for x in run('identify','-format','%w %h',str(image),capture_output=True).stdout.decode().split()]
    content=actual_ui.get('content_rect',[0,0,physical[0]/scale,physical[1]/scale])
    info={'kind':'actual-running-application','platform':'Debian 12 / X11 dedicated Xvfb :141 / Mesa software OpenGL','captured_utc':datetime.now(timezone.utc).isoformat(),'image':str(image),'xvfb_server_dpi':96,'scale':scale,'native_pixels_per_point':actual_ui.get('native_pixels_per_point'),'physical_size':physical,'logical_app_size':[content[2]-content[0],content[3]-content[1]],'scaling_source':'WINIT_X11_SCALE_FACTOR controlled per-process platform scaling' if scale!=1 else 'Xvfb 96 DPI native scaling','exe_sha256':state['sha256'],'notes':notes,'settings':read(state['settings'])}
    for kind in ['ui','search','export','structure']:info[kind]=read(state[kind])
    image.with_suffix('.json').write_text(json.dumps(info,ensure_ascii=False,indent=2),encoding='utf8')
    print(image,flush=True)

for command in json.loads(sys.argv[1]):
    action,*args=command
    if action=='display':
        subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24','-nolisten','tcp'],env=ENV,stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT)
        time.sleep(.6)
        subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT);time.sleep(.6)
    elif action=='launch':
        exe=ROOT/args[0];phase=args[1];case=args[2] if len(args)>2 else 'light';session=OUT/'sessions'/f'native-{phase}-{case}';session.mkdir(parents=True,exist_ok=True)
        state={'phase':phase,'sha256':hashlib.sha256(exe.read_bytes()).hexdigest()}
        for kind in ['settings','ui','search','export','structure']:state[kind]=str(session/(kind+'.json'))
        settings=dict(language='en' if 'english' in case else 'zh-CN',appearance='Dark' if 'dark' in case else 'Light',accent=0,font_size=23.0 if '23px' in case else 15.0,autosave=False,recovery=True,reduce_motion=True,gpu_acceleration=True)
        Path(state['settings']).write_text(json.dumps(settings))
        renderer='/tmp/mdcard-ui-runtime/renderer' if Path('/tmp/mdcard-ui-runtime/renderer/runtime/node').exists() else '/opt/mdcard-renderer'
        env=dict(ENV,MDCARD_SETTINGS_PATH=state['settings'],MDCARD_UI_REPORT=state['ui'],MDCARD_SEARCH_REPORT=state['search'],MDCARD_EXPORT_REPORT=state['export'],MDCARD_STRUCTURE_REPORT=state['structure'],MDCARD_READING_STATE_PATH=str(session/'reading.json'),MDCARD_RECOVERY_DIR=str(session/'recovery'),MDCARD_RENDERER_DIR=renderer)
        if 'scale125' in case:env['WINIT_X11_SCALE_FACTOR']='1.25'
        if 'scale150' in case:env['WINIT_X11_SCALE_FACTOR']='1.5'
        argv=[str(exe)]+([str(ROOT/args[3])] if len(args)>3 and args[3] else [])
        app=subprocess.Popen(argv,env=env,cwd=ROOT,stdout=(session/'app.log').open('wb'),stderr=subprocess.STDOUT)
        state['pid']=app.pid;time.sleep(2)
        result=run('xdotool','search','--pid',str(app.pid),'--name','MDcard',capture_output=True)
        state['window']=result.stdout.decode().splitlines()[0]
        run('xdotool','windowsize',state['window'],'1240','820','windowmove',state['window'],'20','20','windowfocus',state['window']);time.sleep(.5)
    elif action=='phase':state['phase']=args[0]
    elif action=='shot':shot(*args)
    elif action=='shot-root':shot(args[0],'Dedicated Xvfb :141 including native picker',True)
    elif action=='key':key(args[0])
    elif action=='paste':paste(args[0])
    elif action=='click':click(*args)
    elif action=='control':control(*args)
    elif action=='wait':time.sleep(args[0])
    elif action=='resize':run('xdotool','windowsize',state['window'],str(args[0]),str(args[1]));time.sleep(.6)
    elif action=='scroll':
        run('xdotool','mousemove','--window',state['window'],str(args[0]),str(args[1]),'click','--repeat',str(abs(args[2])),'--delay','60','5' if args[2]<0 else '4');time.sleep(.5)
    elif action=='open':key('ctrl+o');time.sleep(.6);key('ctrl+l');paste(str(ROOT/args[0]));key('Return');time.sleep(.7)
    elif action=='close':
        key('ctrl+q');time.sleep(.5)
        if Path(f'/proc/{state["pid"]}').exists():
            # A zombie has exited but still appears until the parent is reaped by init.
            status=Path(f'/proc/{state["pid"]}/stat').read_text().split()[2]
            if status!='Z':raise RuntimeError('App did not close cleanly')
    elif action=='kill':os.kill(state['pid'],15);time.sleep(.4)
    elif action=='report':
        data=read(state[args[0] if args else 'ui']);print(json.dumps(data,ensure_ascii=False),flush=True)
    elif action=='focus-control':
        for _ in range(40):
            if any(x['name']==args[0] and x.get('focus') for x in read(state['ui']).get('controls',[])):break
            key('Tab')
        else:raise AssertionError('Keyboard traversal did not reach '+args[0])
    elif action=='assert':
        kind,path,expected=args
        for _ in range(30):
            value=read(state[kind])
            for part in path.split('.'):value=value[part]
            if value==expected:break
            time.sleep(.05)
        else:raise AssertionError(f'{kind}:{path}: actual {value!r} != expected {expected!r}')
    else:raise ValueError(action)
    STATE.write_text(json.dumps(state))
