# SPDX-License-Identifier: AGPL-3.0-only
"""Passive Windows launch/placement/capture for this task's left-screen instance.

No mouse, keyboard, clipboard or foreground activation. Interactive acceptance
uses the dedicated Xvfb display with ui-ux-native.py.
"""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from datetime import datetime, timezone
from PIL import ImageGrab, ImageStat
import psutil

u=C.windll.user32
u.SetThreadDpiAwarenessContext.argtypes=[C.c_void_p]
u.SetThreadDpiAwarenessContext(C.c_void_p(-4))
u.GetWindowRect.argtypes=[C.c_void_p,C.c_void_p]
u.GetWindowThreadProcessId.argtypes=[C.c_void_p,C.c_void_p]
u.GetWindowTextW.argtypes=[C.c_void_p,C.c_void_p,C.c_int]
u.GetDpiForWindow.argtypes=[C.c_void_p]
u.MoveWindow.argtypes=[C.c_void_p,C.c_int,C.c_int,C.c_int,C.c_int,C.c_bool]
u.SetWindowPos.argtypes=[C.c_void_p,C.c_void_p,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint]
u.ShowWindow.argtypes=[C.c_void_p,C.c_int]
u.GetForegroundWindow.restype=C.c_void_p
u.PostMessageW.argtypes=[C.c_void_p,C.c_uint,C.c_size_t,C.c_ssize_t]
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'evidence/ui-ux/2026-10-28'
STATE=OUT/'capture-state.json'
state=json.loads(STATE.read_text()) if STATE.exists() else {}

def read(path):
    try:return json.loads(Path(path).read_text(encoding='utf-8-sig'))
    except (ValueError,OSError):return {}

def window():
    if not psutil.pid_exists(state['pid']):raise RuntimeError('Task instance no longer exists')
    result=[]
    @C.WINFUNCTYPE(C.c_bool,C.c_void_p,C.c_void_p)
    def enum(h,l):
        pid=C.c_ulong();u.GetWindowThreadProcessId(h,C.byref(pid))
        if pid.value==state['pid']:
            r=(C.c_long*4)();u.GetWindowRect(h,C.byref(r))
            title=C.create_unicode_buffer(512);u.GetWindowTextW(h,title,512)
            if 'MDcard' in title.value and r[2]-r[0]>300 and r[3]-r[1]>200:result.append(h)
        return True
    u.EnumWindows(enum,0)
    if not result:raise RuntimeError('Task main window is not ready')
    return result[0]

def bounds(h):
    r=(C.c_long*4)();u.GetWindowRect(h,C.byref(r));return list(r)

for command in json.loads(sys.argv[1]):
    action,*args=command
    if action=='launch':
        exe=(ROOT/args[0]).resolve();phase=args[1];case=args[2] if len(args)>2 else 'light';session=OUT/'sessions'/f'windows-{phase}-{case}';session.mkdir(parents=True,exist_ok=True)
        state=dict(phase=phase,sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),exe=str(exe))
        for kind in ['settings','ui']:state[kind]=str(session/(kind+'.json'))
        Path(state['settings']).write_text(json.dumps(dict(language='zh-CN',appearance='Dark' if 'dark' in case else 'Light',accent=0,font_size=15,autosave=False,recovery=True,reduce_motion=True,gpu_acceleration='cpu' not in case)))
        env=os.environ.copy();env.update(dict(MDCARD_SETTINGS_PATH=state['settings'],MDCARD_UI_REPORT=state['ui'],MDCARD_READING_STATE_PATH=str(session/'reading.json'),MDCARD_RECOVERY_DIR=str(session/'recovery'),MDCARD_RENDERER_DIR=r'D:\MDcard\App\renderer',MDCARD_WINDOW_POSITION='-1600x30'))
        startup=subprocess.STARTUPINFO();startup.dwFlags|=subprocess.STARTF_USESHOWWINDOW;startup.wShowWindow=0
        argv=[str(exe)]+([str((ROOT/args[3]).resolve())] if len(args)>3 and args[3] else [])
        before=u.GetForegroundWindow();process=subprocess.Popen(argv,cwd=ROOT,env=env,startupinfo=startup);state['pid']=process.pid
        STATE.write_text(json.dumps(state))
        for _ in range(100):
            try:h=window();break
            except RuntimeError:time.sleep(.05)
        else:raise RuntimeError('No task window after launch')
        state['initial_rect']=bounds(h);state['initial_foreground']=u.GetForegroundWindow()
        r=state['initial_rect']
        if not (-2560<=r[0]<r[2]<=0 and 0<=r[1]<r[3]<=1440) or state['initial_foreground']!=before:
            u.PostMessageW(h,0x10,0,0)
            STATE.write_text(json.dumps(state))
            raise RuntimeError('Initial window isolation failed; only this task window was closed: '+json.dumps(state))
        u.MoveWindow(h,-2520,40,1860,1230,True)
        u.ShowWindow(h,4)
        u.SetWindowPos(h,-1,0,0,0,0,0x53)
        for _ in range(100):
            if read(state['ui']).get('content_rect'):break
            time.sleep(.1)
        time.sleep(.5)
        state['foreground_before']=before;state['foreground_after']=u.GetForegroundWindow()
        print(json.dumps(dict(pid=state['pid'],hwnd=h,initial_rect=state['initial_rect'],bounds=bounds(h),foreground_unchanged=state['foreground_before']==state['foreground_after'])))
    elif action=='shot':
        h=window();r=bounds(h)
        if not (-2560<=r[0]<r[2]<=0 and 0<=r[1]<r[3]<=1440):raise RuntimeError(f'Task window must remain on the left physical monitor: {r}')
        time.sleep(.5);im=ImageGrab.grab(bbox=tuple(r),all_screens=True)
        if max(ImageStat.Stat(im.convert('RGB')).stddev)<1:raise RuntimeError('Black capture rejected')
        folder=OUT/state['phase'];folder.mkdir(exist_ok=True);path=folder/(args[0]+'.png');im.save(path)
        metadata=dict(kind='actual-running-application-passive-windows-capture',captured_utc=datetime.now(timezone.utc).isoformat(),pid=state['pid'],hwnd=h,initial_rect=state.get('initial_rect'),initial_foreground=state.get('initial_foreground'),rect_physical=r,size_physical=[r[2]-r[0],r[3]-r[1]],dpi=u.GetDpiForWindow(h),exe_sha256=state['sha256'],settings=read(state['settings']),ui=read(state['ui']),foreground_before=state.get('foreground_before'),foreground_after=state.get('foreground_after'))
        path.with_suffix('.json').write_text(json.dumps(metadata,ensure_ascii=False,indent=2),encoding='utf8');print(path)
    elif action=='close':
        if psutil.pid_exists(state.get('pid',-1)):
            u.PostMessageW(window(),0x10,0,0);time.sleep(.6)
    elif action=='report':print(json.dumps(read(state['ui']),ensure_ascii=False))
    else:raise ValueError('Only passive launch, shot, close and report are supported')
    STATE.write_text(json.dumps(state))
