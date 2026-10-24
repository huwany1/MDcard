#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Real X11 mouse/keyboard search regression, plus six-language visual matrix."""
import hashlib,json,os,subprocess,sys,time,uuid
from pathlib import Path

ROOT=Path('/work');OUT=ROOT/'evidence/search/linux-final';OUT.mkdir(parents=True,exist_ok=True)
FIX=ROOT/'artifacts/global-search/linux-workspace';FIX.mkdir(parents=True,exist_ok=True);(FIX/'nested').mkdir(exist_ok=True)
EXE=Path(sys.argv[1] if len(sys.argv)>1 else '/work/target/release/mdcard')
ENV=dict(os.environ,DISPLAY=':119',LANG='C.UTF-8',LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',XDG_RUNTIME_DIR='/tmp/mdcard-search-runtime',MDCARD_SETTINGS_PATH=str(OUT/'settings.json'),MDCARD_SEARCH_REPORT=str(OUT/'state.json'))
Path(ENV['XDG_RUNTIME_DIR']).mkdir(exist_ok=True);Path(ENV['XDG_RUNTIME_DIR']).chmod(0o700)
children=[];app=None;window=None
def run(*args,**kwargs):return subprocess.run(args,env=ENV,check=True,**kwargs)
def key(*keys):run('xdotool','key','--clearmodifiers',*keys);time.sleep(.14)
def state():
    try:return json.loads((OUT/'state.json').read_text())
    except (OSError,ValueError) as error:return {'read_error':str(error)}
def wait(predicate,message,seconds=10):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        s=state()
        if predicate(s):return s
        if app.poll() is not None:raise RuntimeError('Application exited: '+message)
        time.sleep(.04)
    raise AssertionError(message+': '+json.dumps(state(),ensure_ascii=False)[:1800])
def control(name):
    s=wait(lambda s:any(c['name']==name for c in s.get('controls',[])),'Missing control '+name)
    r=next(c['rect'] for c in s['controls'] if c['name']==name)
    run('xdotool','mousemove','--window',window,str(round((r[0]+r[2])/2)),str(round((r[1]+r[3])/2)),'click','1');time.sleep(.12)
def paste(text):
    run('xclip','-selection','clipboard','-loops','1',input=text.encode(),stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);key('ctrl+v')
def fill(name,text):control(name);key('ctrl+a');paste(text);time.sleep(.25)
def shot(name):time.sleep(.15);run('import','-window',window,str(OUT/f'{name}.png'))
def launch(language='en',size=(1240,860)):
    global app,window
    (OUT/'settings.json').write_text(json.dumps(dict(language=language,appearance='Light',gpu_acceleration=True,reduce_motion=True,font_size=15)))
    (OUT/'state.json').unlink(missing_ok=True)
    app=subprocess.Popen([str(EXE),str(FIX/'a.md')],env=ENV,stdout=(OUT/f'{language}.log').open('wb'),stderr=subprocess.STDOUT)
    for _ in range(80):
        p=subprocess.run(['xdotool','search','--onlyvisible','--pid',str(app.pid),'--name','MDcard'],env=ENV,capture_output=True)
        if p.returncode==0:window=p.stdout.decode().splitlines()[0];break
        time.sleep(.05)
    else:raise RuntimeError('No native window')
    run('xdotool','windowsize',window,str(size[0]),str(size[1]),'windowfocus',window);time.sleep(.6)
def close():
    global app
    key('Escape','Escape','ctrl+q');app.wait(timeout=8);assert app.returncode==0;app=None

verification=str(uuid.uuid4());(OUT/'result.json').unlink(missing_ok=True)
original='# Main 🦀\n\ncat 12\ncat 34\n';other='---\naliases: [kitten, 小猫]\n---\n# Second\ncat 56\n'
(FIX/'a.md').write_text(original);(FIX/'b.md').write_text(other);(FIX/'nested/c.md').write_text('# Nested\ncat 78\n')
(FIX/'jump.md').write_text('🦀 line\n'*349+'🦀 jump-unique-marker end\n')
try:
    children.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    children.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.5)
    launch();key('ctrl+h');fill('find-query',r'(?m)^(?<animal>cat) (\d+)$');control('regex');fill('replace-query','$<animal>=$2suffix')
    wait(lambda s:s.get('results')==2 and not s.get('error'),'Regex results');control('preview');wait(lambda s:s.get('preview_files')==1,'Single diff');shot('01-capture-diff')
    control('apply');after=original.replace('cat 12','cat=12suffix').replace('cat 34','cat=34suffix')
    wait(lambda s:s.get('text')==after and not s.get('open'),'Single atomic apply');key('ctrl+s');assert(FIX/'a.md').read_text()==after
    key('ctrl+z','ctrl+s');assert(FIX/'a.md').read_text()==original
    key('ctrl+shift+z','ctrl+s');assert(FIX/'a.md').read_text()==after
    key('ctrl+z','ctrl+s');key('ctrl+h');fill('find-query','(');wait(lambda s:bool(s.get('error')),'Inline invalid regex');shot('02-inline-error');key('Escape')
    key('ctrl+shift+f');fill('find-query','cat');fill('replace-query','dog');fill('include','**/*.md');fill('exclude','nested/**')
    wait(lambda s:s.get('results')==3 and not s.get('error'),'Workspace filtered matches');control('preview');wait(lambda s:s.get('preview_files')==2,'Workspace diff');shot('03-workspace-diff');control('apply')
    wait(lambda s:not s.get('busy') and s.get('text')==original.replace('cat','dog'),'Disk/memory commit');assert(FIX/'b.md').read_text()==other.replace('cat','dog');assert'cat 78'in(FIX/'nested/c.md').read_text();key('Escape');close()
    launch();key('ctrl+shift+f');control('history');control('rollback-0');wait(lambda s:not s.get('busy') and s.get('text')==original,'Restart rollback');assert(FIX/'b.md').read_text()==other;shot('04-restart-rollback');close()
    launch();key('ctrl+shift+f');fill('find-query','jump-unique-marker');wait(lambda s:s.get('results')==1 and s['groups'][0]['hits'][0]['line']==350,'Line350 result');shot('05-context-highlight');control('result-0-0');wait(lambda s:not s.get('busy') and str(s.get('path','')).endswith('jump.md'),'Open jump target');key('ctrl+c')
    clip=run('xclip','-selection','clipboard','-o',capture_output=True).stdout.decode();assert clip=='jump-unique-marker',repr(clip);shot('06-exact-jump');close()
    launch();key('ctrl+p');fill('quick-query','kitten');wait(lambda s:len(s.get('quick',[]))==1,'Alias fuzzy match');shot('07-quick-alias');key('shift+Return');wait(lambda s:bool(s.get('secondary')),'Real second document');control('secondary-editor');key('ctrl+End');paste('second-unsaved');key('ctrl+s');wait(lambda s:s.get('secondary') and not s['secondary']['dirty'] and not s.get('busy'),'Secondary save routing');assert(FIX/'b.md').read_text()==other+'second-unsaved';assert(FIX/'a.md').read_text()==original;shot('08-dual-document');close()
    launch();key('ctrl+p');fill('quick-query','created-'+verification+'.md');wait(lambda s:any(c['name']=='quick-create' for c in s.get('controls',[])),'Missing filename create');key('Return');wait(lambda s:str(s.get('path','')).endswith('created-'+verification+'.md') and not s.get('busy'),'Create and open');close()
    for language in ['zh-CN','zh-TW','en','ko','ja','ru']:
        launch(language);key('ctrl+shift+f');fill('find-query','(');control('regex');wait(lambda s:bool(s.get('error')),'Localized inline syntax error');shot(language+'-search-error');key('Escape','ctrl+p');fill('quick-query','kitten');wait(lambda s:len(s.get('quick',[]))==1,'Localized Quick Switcher');shot(language+'-quick');close()
    result=dict(verification_id=verification,date='2026-10-20',timezone='Asia/Shanghai',exe=str(EXE),exe_sha256=hashlib.file_digest(EXE.open('rb'),'sha256').hexdigest(),real_mouse_keyboard=True,atomic_undo_redo=True,regex_captures_and_adjacent_text=True,inline_syntax_error=True,include_exclude_diff=True,disk_memory_sync=True,restart_rollback=True,exact_line350_unicode_selection=True,aliases_fuzzy_and_real_dual_pane=True,secondary_ctrl_s=True,missing_enter_create=True,languages=['zh-CN','zh-TW','en','ko','ja','ru'])
    (OUT/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
finally:
    if app is not None and app.poll() is None:app.kill();app.wait()
    for child in reversed(children):child.terminate();child.wait(timeout=5)
