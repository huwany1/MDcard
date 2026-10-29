# SPDX-License-Identifier: AGPL-3.0-only
"""Single-step installed preferences probe; external disposable QA only."""
import ast,ctypes,json,os,struct,subprocess,time,zlib
from pathlib import Path
out=Path('/qa-output/linux-autosave-probe');out.mkdir(exist_ok=True)
module=ast.parse(Path('/qa/verify-installed-linux.py').read_text());exec(compile(ast.Module(body=[n for n in module.body if isinstance(n,ast.FunctionDef) and n.name=='capture'],type_ignores=[]),'<qa-capture>','exec'))
env=dict(os.environ,DISPLAY=':98',LANG='C.UTF-8',XDG_CURRENT_DESKTOP='Openbox',XDG_RUNTIME_DIR='/tmp/mdcard-015-probe',LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2');Path(env['XDG_RUNTIME_DIR']).mkdir(mode=0o700,exist_ok=True)
profile=Path.home()/'qa/probe';settings=profile/'config/MDcard/settings.json';settings.parent.mkdir(parents=True,exist_ok=True);settings.write_text(json.dumps(dict(language='en',font_size=15,reduce_motion=True)))
ui=out/'ui.json';document=out/'document.md';document.write_text('# Probe\n')
services=[subprocess.Popen(['Xvfb',':98','-screen','0','1600x1200x24'],env=env,stdout=(out/'xvfb.log').open('w'),stderr=subprocess.STDOUT)];time.sleep(.4);services.append(subprocess.Popen(['openbox'],env=env,stdout=(out/'openbox.log').open('w'),stderr=subprocess.STDOUT));time.sleep(.4)
subprocess.run(['dbus-update-activation-environment','DISPLAY','XDG_RUNTIME_DIR','XDG_CURRENT_DESKTOP'],env=env,check=True)
app=subprocess.Popen(['/opt/mdcard/mdcard',str(document)],env={**env,'PATH':'','XDG_CONFIG_HOME':str(profile/'config'),'XDG_STATE_HOME':str(profile/'state'),'MDCARD_UI_REPORT':str(ui)},cwd=out,stdout=(out/'app.log').open('w'),stderr=subprocess.STDOUT)
def run(*args):return subprocess.run(args,env=env,check=True,capture_output=True)
try:
 time.sleep(2);window=run('xdotool','search','--all','--pid',str(app.pid),'--name','MDcard').stdout.decode().splitlines()[0];run('xdotool','windowsize',window,'1400','1100','windowactivate','--sync',window);time.sleep(.5)
 run('xdotool','key','--clearmodifiers','ctrl+comma');time.sleep(.6);report=json.loads(ui.read_text());(out/'before.json').write_text(json.dumps(report,indent=2));capture(out/'01-preferences.png')
 c=next(c for c in report['controls'] if c['name']=='Auto save');r=c['rect'];clip=c['clip'];x=(max(r[0],clip[0])+min(r[2],clip[2]))/2;y=(max(r[1],clip[1])+min(r[3],clip[3]))/2
 (out/'click.json').write_text(json.dumps(dict(control=c,x=x,y=y),indent=2));run('xdotool','mousemove','--window',window,str(round(x)),str(round(y)));time.sleep(.1);run('xdotool','mousedown','1');time.sleep(.08);run('xdotool','mouseup','1');time.sleep(.6)
 capture(out/'02-after-click.png');(out/'after.json').write_text(ui.read_text());(out/'settings-after.json').write_text(settings.read_text());print((out/'click.json').read_text());print(settings.read_text())
finally:
 app.terminate();app.wait(timeout=10)
 for service in reversed(services):service.terminate();service.wait(timeout=10)
