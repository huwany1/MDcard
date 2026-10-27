# SPDX-License-Identifier: AGPL-3.0-only
"""Real Windows keyboard/mouse, screen recording and diagnostic sampling.

Run with an isolated candidate; this never edits a user's document/settings.
Screen capture timestamps are independent of update/GL callback diagnostics.
"""
import argparse
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import subprocess
import threading
import time
import io
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from PIL import Image, ImageGrab
import imageio_ffmpeg
import psutil

u = C.windll.user32
u.SetProcessDPIAware()
u.SetForegroundWindow.argtypes = [C.c_void_p]
u.MoveWindow.argtypes = [C.c_void_p, C.c_int, C.c_int, C.c_int, C.c_int, C.c_bool]
u.GetDpiForWindow.argtypes = [C.c_void_p]
u.GetWindowRect.argtypes = [C.c_void_p, C.c_void_p]
u.GetForegroundWindow.restype = C.c_void_p
u.GetWindowThreadProcessId.argtypes = [C.c_void_p, C.c_void_p]
u.SetWindowPos.argtypes = [C.c_void_p,C.c_void_p,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint]
u.PostMessageW.argtypes=[C.c_void_p,C.c_uint,C.c_size_t,C.c_void_p]
u.LoadKeyboardLayoutW.argtypes=[C.c_wchar_p,C.c_uint];u.LoadKeyboardLayoutW.restype=C.c_void_p

def key(v, ctrl=False, shift=False, alt=False):
    mods = ([0x11] if ctrl else []) + ([0x10] if shift else []) + ([0x12] if alt else [])
    for m in mods: u.keybd_event(m, 0, 0, 0)
    if mods:time.sleep(.015)
    u.keybd_event(v, 0, 0, 0)
    time.sleep(.015)
    u.keybd_event(v, 0, 2, 0)
    for m in reversed(mods): u.keybd_event(m, 0, 2, 0)
    time.sleep(.005)

def clipboard(s):
    # CF_UNICODETEXT owns its HGLOBAL after SetClipboardData succeeds.
    k = C.windll.kernel32
    k.GlobalAlloc.restype = C.c_void_p
    k.GlobalLock.argtypes = [C.c_void_p]; k.GlobalLock.restype = C.c_void_p
    k.GlobalUnlock.argtypes = [C.c_void_p]
    u.SetClipboardData.argtypes = [C.c_uint, C.c_void_p]
    data = (s + '\0').encode('utf-16-le')
    for _ in range(50):
        if u.OpenClipboard(None): break
        time.sleep(.01)
    else: raise RuntimeError('clipboard unavailable')
    try:
        u.EmptyClipboard()
        handle = k.GlobalAlloc(0x42, len(data))
        dest = k.GlobalLock(handle); C.memmove(dest, data, len(data)); k.GlobalUnlock(handle)
        if not u.SetClipboardData(13, handle): raise RuntimeError('clipboard copy failed')
    finally: u.CloseClipboard()

def read(path):
    for _ in range(12):
        try:return json.loads(path.read_text(encoding='utf-8-sig'))
        except (OSError, ValueError):time.sleep(.003)
    return {}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--exe', required=True)
    ap.add_argument('--output', required=True)
    ap.add_argument('--bytes', type=int, default=8192)
    ap.add_argument('--seconds', type=float, default=32)
    ap.add_argument('--scenario', choices=['continuous','latency','media','matrix','ime','resources','threshold','rich','anchor-middle'], default='continuous')
    ap.add_argument('--samples', type=int, default=40)
    ap.add_argument('--interval', type=float, default=1.0)
    ap.add_argument('--position', choices=['heading','paragraph'], default='paragraph')
    ap.add_argument('--fixture')
    a = ap.parse_args()
    out = Path(a.output).resolve(); out.mkdir(parents=True, exist_ok=False)
    fixture = out / 'manuscript.md'
    head = '# Continuous preview\n\nVISIBLE TOP ANCHOR — 中文输入验证\n\n'
    para = 'A stable paragraph remains readable during editing. **Bold** and `code`.\n\n'
    fixture_bytes=a.bytes-8 if a.scenario=='latency' else a.bytes
    text = head + para * max(1, (fixture_bytes-len(head.encode()))//len(para))
    text += 'x' * max(0,fixture_bytes-len(text.encode()))
    if a.scenario in ('matrix','anchor-middle'):
        pattern='Paragraph {i:06d} remains readable. **Bold** and `code`.\n\n'
        row=pattern.format(i=0)
        count=max(1,(a.bytes-len(head.encode()))//len(row))
        text=head+''.join(pattern.format(i=i) for i in range(count))
        text+='x'*max(0,a.bytes-len(text.encode()))
    if a.fixture: text=Path(a.fixture).read_text(encoding='utf-8')
    server=None
    if a.scenario in ('media','resources'):
        payload=io.BytesIO();Image.new('RGB',(512,512) if a.scenario=='resources' else (320,160),'#8ab69b').save(payload,format='PNG'); png=payload.getvalue()
        requests=[]
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                attempt=sum(r['path']==self.path for r in requests)+1
                entry=dict(path=self.path,at=time.perf_counter(),attempt=attempt)
                requests.append(entry)
                if self.path=='/slow.png': time.sleep(10)
                if self.path=='/hang.png' and attempt==1: time.sleep(25)
                body=b'not an image' if self.path=='/invalid.png' and attempt==1 else png
                self.send_response(200);self.send_header('Content-Type','image/png');self.end_headers()
                try:self.wfile.write(body)
                except OSError:pass
                entry['finished_at']=time.perf_counter()
            def log_message(self,*args):pass
        server=ThreadingHTTPServer(('127.0.0.1',0),Handler)
        threading.Thread(target=server.serve_forever,daemon=True).start()
        base=f'http://127.0.0.1:{server.server_port}'
        text=f'''# Media isolation\n\nBODY REMAINS READABLE while one block waits.\n\n![slow image]({base}/slow.png)\n\nBODY AFTER SLOW IMAGE remains scrollable.\n\n![invalid image]({base}/invalid.png)\n\nBODY AFTER FAILED IMAGE.\n\n![timeout image]({base}/hang.png)\n\nInvalid formula:\n\n$$\\thisCommandDoesNotExist{{x}}$$\n\n```mermaid\nflowchart THIS IS INVALID !!!\n```\n\nFINAL BODY AFTER ALL MEDIA.\n'''
        if a.scenario=='resources':text=f'# Image cache stress\n\n![image]({base}/image-0.png)\n\nReadable body.\n'
    fixture.write_bytes(text.encode('utf-8'))
    initial_file_bytes=fixture.stat().st_size
    settings = out/'settings.json'
    settings.write_text(json.dumps(dict(language='en', appearance='Light', gpu_acceleration=True,
        reduce_motion=True, font_size=15, autosave=False, recovery=False)),encoding='utf-8')
    env = {k:v for k,v in os.environ.items() if not k.startswith('MDCARD_')}
    env.update(MDCARD_SETTINGS_PATH=str(settings), MDCARD_UI_REPORT=str(out/'ui.json'),
        MDCARD_RICH_REPORT=str(out/'rich.json'), MDCARD_PERFORMANCE_REPORT=str(out/'performance.json'),
        MDCARD_READING_REPORT=str(out/'reading-report.json'), MDCARD_READING_STATE_PATH=str(out/'reading-state.json'),
        MDCARD_RECOVERY_DIR=str(out/'recovery'), MDCARD_RENDERER_DIR=str(Path('renderer').resolve()))
    env['PATH']=str(Path('C:/Program Files/nodejs'))+os.pathsep+env.get('PATH','')
    first_record=subprocess.Popen([imageio_ffmpeg.get_ffmpeg_exe(),'-y','-loglevel','error',
        '-f','gdigrab','-framerate','20','-offset_x','40','-offset_y','40','-video_size','1550x1024',
        '-i','desktop','-c:v','libx264','-preset','ultrafast','-crf','24','-pix_fmt','yuv420p',str(out/'launch-screen.mp4')],
        stdin=subprocess.PIPE,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    time.sleep(.3);launch_at=time.perf_counter()
    launch_samples=[];launch_stop=threading.Event()
    def observe_launch():
        while not launch_stop.is_set():
            rui=read(out/'ui.json');rrich=read(out/'rich.json')
            launch_samples.append(dict(t=time.perf_counter(),revision=rui.get('revision'),
                path=rui.get('document_path'),source_stamp=rrich.get('sourceStamp'),
                displayed_revision=rrich.get('displayedRevision'),parses=rrich.get('counters',{}).get('parses')))
            time.sleep(.01)
    launch_thread=threading.Thread(target=observe_launch,daemon=True);launch_thread.start()
    app = subprocess.Popen([str(Path(a.exe).resolve()), str(fixture)], env=env)
    hwnd = None
    ENUM = C.WINFUNCTYPE(C.c_bool, C.c_void_p, C.c_void_p)
    def visit(h, _):
        nonlocal hwnd
        pid=C.c_ulong(); u.GetWindowThreadProcessId(h, C.byref(pid))
        title=C.create_unicode_buffer(512);u.GetWindowTextW(C.c_void_p(h),title,512)
        if pid.value == app.pid and u.IsWindowVisible(C.c_void_p(h)) and title.value:
            hwnd=h; return False
        return True
    for _ in range(150):
        u.EnumWindows(ENUM(visit), 0)
        if hwnd: break
        time.sleep(.1)
    if not hwnd: raise RuntimeError('no window')
    u.MoveWindow(hwnd, 40, 40, 1550, 1024, True)
    u.SetWindowPos(hwnd,C.c_void_p(-1),0,0,0,0,0x43)
    u.SetForegroundWindow(hwnd)
    time.sleep(2)
    # Establish focus by an actual title-bar click, then use recorded editor control.
    def click(x,y):
        u.SetCursorPos(int(x),int(y)); time.sleep(.08)
        u.mouse_event(2,0,0,0,0); time.sleep(.08); u.mouse_event(4,0,0,0,0)
    click(500,65); time.sleep(.3)
    if u.GetForegroundWindow() != hwnd:
        rect=(C.c_long*4)();u.GetWindowRect(hwnd,C.byref(rect))
        title=C.create_unicode_buffer(512);u.GetWindowTextW(C.c_void_p(hwnd),title,512)
        fg=u.GetForegroundWindow();pid=C.c_ulong();u.GetWindowThreadProcessId(fg,C.byref(pid))
        app.terminate(); raise RuntimeError(f'actual app did not receive foreground focus: hwnd={hwnd},fg={fg},pid={pid.value},title={title.value},rect={list(rect)}')
    dpi=u.GetDpiForWindow(hwnd)/96
    if a.scenario!='ime':
        english=u.LoadKeyboardLayoutW('00000409',0)
        u.PostMessageW(hwnd,0x50,0,english);time.sleep(.2)
    report=read(out/'ui.json')
    control=next(c for c in report['controls'] if c['name']=='main-source-editor')
    x0,y0,x1,y1=control['rect']; clip=control.get('clip',control['rect'])
    for _ in range(4):
        click(40+(max(x0,clip[0])+50)*dpi,40+(max(y0,clip[1])+15)*dpi)
        time.sleep(.4)
        if next(c for c in read(out/'ui.json')['controls'] if c['name']=='main-source-editor')['focus']: break
    else:
        ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'focus-failure.png')
        app.terminate(); raise RuntimeError('source did not receive editor focus')
    key(0x24,ctrl=True)
    if a.position=='paragraph':
        for _ in range(4): key(0x28)
    key(0x23)
    time.sleep(.2)
    if a.scenario=='latency':
        # Warm one fixed-width slot inside the ordinary paragraph. Timed edits
        # replace these eight bytes, so 64 KiB / 1 MiB samples stay on their
        # stated side of the threshold throughout all forty measurements.
        original_stamp=read(out/'rich.json').get('sourceStamp')
        clipboard('edit-000');key(0x56,ctrl=True)
        deadline=time.perf_counter()+20
        while time.perf_counter()<deadline:
            rui=read(out/'ui.json');rrich=read(out/'rich.json')
            if rui.get('document_text_bytes')==a.bytes and rrich.get('sourceStamp') not in (None,original_stamp):break
            time.sleep(.05)
        else:
            app.terminate();first_record.communicate(b'q',timeout=10)
            raise RuntimeError('fixed-size benchmark slot did not reach the requested byte count')
    launch_record_ended=time.perf_counter()
    first_record.communicate(b'q',timeout=10)
    launch_stop.set();launch_thread.join()
    (out/'launch-samples.json').write_text(json.dumps(launch_samples),encoding='utf-8')
    stop=threading.Event(); frames=[]; reports=[]
    ff=subprocess.Popen([imageio_ffmpeg.get_ffmpeg_exe(),'-y','-loglevel','error',
        '-f','rawvideo','-pixel_format','rgb24','-video_size','1550x1024','-framerate','20',
        '-i','-','-c:v','libx264','-preset','ultrafast','-crf','22','-pix_fmt','yuv420p',str(out/'screen.mp4')],stdin=subprocess.PIPE)
    def capture():
        start=time.perf_counter(); n=0
        while not stop.is_set():
            stamp=time.perf_counter()
            im=ImageGrab.grab(bbox=(40,40,1590,1064)).convert('RGB')
            ff.stdin.write(im.tobytes())
            roi=im.crop((925,310,1510,920)).convert('L')
            hist=roi.histogram()
            frames.append(dict(frame=n, capture_started=stamp, capture_finished=time.perf_counter(),
                preview_pixel_sha256=hashlib.sha256(roi.tobytes()).hexdigest(),preview_ink_pixels=sum(hist[:150])))
            rui=read(out/'ui.json'); rrich=read(out/'rich.json')
            pm=psutil.Process(app.pid).memory_info()
            extra={}
            if a.scenario in ('media','resources'):
                tree_rss=pm.rss;tree_private=getattr(pm,'private',pm.vms);children=psutil.Process(app.pid).children(recursive=True)
                counted=0
                for child in children:
                    try:
                        cm=child.memory_info();tree_rss+=cm.rss;tree_private+=getattr(cm,'private',cm.vms);counted+=1
                    except (psutil.NoSuchProcess,psutil.AccessDenied):pass
                extra=dict(process_tree_working_set=tree_rss,process_tree_private_bytes=tree_private,child_processes=counted)
            reports.append(dict(t=time.perf_counter(),ui=rui,rich=rrich,
                working_set=pm.rss,private_bytes=getattr(pm,'private',pm.vms),**extra))
            n+=1
            time.sleep(max(0,start+n*.05-time.perf_counter()))
    thread=threading.Thread(target=capture); thread.start()
    actions=[]
    outcome={'status':'running'}
    def wait_until(predicate,message,seconds=15):
        deadline=time.perf_counter()+seconds
        while time.perf_counter()<deadline:
            if predicate():return
            time.sleep(.05)
        raise RuntimeError(message)
    def current_preview_ready():
        rui=read(out/'ui.json');rrich=read(out/'rich.json');rread=read(out/'reading-report.json')
        mapped=rrich.get('mappedRevision')
        return bool(mapped and mapped[1]==rui.get('revision') and mapped[0]==rread.get('main_buffer_id')
            and rread.get('main_revision')==rui.get('revision')
            and rread.get('views',{}).get('main/preview',{}).get('geometry'))
    try:
        time.sleep(1)
        begin=time.perf_counter(); i=0
        if a.scenario=='latency':
            for i in range(a.samples):
                before=read(out/'ui.json')
                wait_until(lambda:'revision' in read(out/'ui.json'),'complete pre-edit UI report unavailable',3)
                before=read(out/'ui.json')
                u.keybd_event(0x10,0,0,0);time.sleep(.015)
                for _ in range(8):key(0x25)
                u.keybd_event(0x10,0,2,0);time.sleep(.02)
                selected=read(out/'ui.json')
                marker=f'edit-{i+1:03d}'
                clipboard(marker); t=time.perf_counter(); key(0x56,ctrl=True)
                action=dict(kind='local-edit',text=marker,at=t,before_revision=before['revision'],
                    before_selection=before.get('selection'),selected_selection=selected.get('selection'),expected_revision=before['revision']+1)
                actions.append(action)
                deadline=t+a.interval
                observed=None
                while time.perf_counter()<deadline:
                    snapshot=read(out/'ui.json')
                    if snapshot.get('revision')==action['expected_revision'] and 'document_text_bytes' in snapshot:
                        observed=snapshot
                        action['first_new_revision_report']=dict(at=time.perf_counter(),revision=snapshot['revision'],bytes=snapshot['document_text_bytes'],selection=snapshot.get('selection'))
                        break
                    time.sleep(.005)
                if observed is None:
                    action['censored_source_report']=True
                    action['censor_window_ms']=(time.perf_counter()-t)*1000
                    raise RuntimeError('timed edit source report was missing/censored at its deadline')
                if observed['document_text_bytes']!=a.bytes:
                    raise RuntimeError('timed local edit changed the fixed benchmark byte count')
                time.sleep(max(0,deadline-time.perf_counter()))
        elif a.scenario=='media':
            time.sleep(3)
            # Scroll through individually failing blocks while other text stays editable.
            for _ in range(6):
                u.SetCursorPos(1350,800);u.mouse_event(0x800,0,0,C.c_uint(-360).value,0);time.sleep(1)
            ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'media-errors.png')
            wait_until(lambda:any(r['path']=='/hang.png' for r in requests),'timeout image never requested')
            hang_start=next(r['at'] for r in requests if r['path']=='/hang.png')
            clipboard(' LIVE-WHILE-IMAGE-WAITS ');key(0x56,ctrl=True)
            actions.append(dict(kind='edit-during-image-wait',at=time.perf_counter()))
            time.sleep(max(0,hang_start+16.2-time.perf_counter()))
            def image_control(prefix,suffix):
                for _ in range(15):
                    c=next((c for c in read(out/'rich.json').get('controls',[]) if c['name'].startswith(prefix) and c['name'].endswith(suffix)),None)
                    if c and 270<=c['rect'][1] and c['rect'][3]<=775:return c
                    delta=120 if c is None or c['rect'][1]<270 else -120
                    u.SetCursorPos(1350,750);u.mouse_event(0x800,0,0,C.c_uint(delta).value,0);time.sleep(.2)
                raise RuntimeError(f'image control not visibly reachable: {prefix}{suffix}')
            timeout=image_control('image-retry-','/hang.png')
            ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'media-timeout.png')
            if time.perf_counter()>=hang_start+24.5:raise RuntimeError('timeout screenshot missed pre-response window')
            for suffix in ['/hang.png','/invalid.png']:
                c=image_control('image-retry-',suffix)
                ph=next(x for x in read(out/'rich.json')['controls'] if x['name'].startswith('image-placeholder-') and x['name'].endswith(suffix))
                if ph['rect'][3]-ph['rect'][1]<159:raise RuntimeError('image placeholder minimum height was not preserved')
                count=sum(r['path']==suffix for r in requests)
                x0,y0,x1,y1=c['rect'];click(40+(x0+x1)*dpi/2,40+(y0+y1)*dpi/2)
                wait_until(lambda:sum(r['path']==suffix for r in requests)>count,'real image Retry did not issue a new request',5)
                actions.append(dict(kind='retry-image',path=suffix,at=time.perf_counter(),placeholder_height=ph['rect'][3]-ph['rect'][1]))
                time.sleep(.5)
            ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'media-image-retries.png')
            for _ in range(6):
                u.SetCursorPos(1350,750);u.mouse_event(0x800,0,0,C.c_uint(-360).value,0);time.sleep(.2)
            time.sleep(1)
            for c in read(out/'rich.json').get('controls',[]):
                if c['name'].endswith('-Retry') and 270<=c['rect'][1] and c['rect'][3]<=775:
                    x0,y0,x1,y1=c['rect'];click(40+(x0+x1)*dpi/2,40+(y0+y1)*dpi/2)
                    actions.append(dict(kind='retry-render',control=c['name'],at=time.perf_counter()));time.sleep(2);break
            ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'media-diagram-error.png')
            c=next(c for c in read(out/'ui.json')['controls'] if c['name']=='main-source-editor')
            left=max(c['rect'][0],c['clip'][0]);top=max(c['rect'][1],c['clip'][1]);click(40+(left+40)*dpi,40+(top+15)*dpi)
            valid=text.replace('\\thisCommandDoesNotExist{x}','x^2+1').replace('flowchart THIS IS INVALID !!!','flowchart LR\n A[Start] --> B[Ready]')
            before_fix=read(out/'ui.json');key(0x41,ctrl=True);time.sleep(.25)
            for attempt in range(3):
                clipboard(valid);time.sleep(.12);key(0x56,ctrl=True)
                deadline=time.perf_counter()+1
                while time.perf_counter()<deadline and read(out/'ui.json').get('revision')==before_fix['revision']:time.sleep(.05)
                if read(out/'ui.json').get('revision')!=before_fix['revision']:break
            else:raise RuntimeError('native paste of repaired media source was not delivered')
            actions.append(dict(kind='repair-media-source',attempts=attempt+1,at=time.perf_counter()))
            if read(out/'ui.json')['document_text_bytes']!=len(valid.encode()):raise RuntimeError('repaired media source has unexpected byte count')
            for _ in range(5):
                u.SetCursorPos(1350,750);u.mouse_event(0x800,0,0,C.c_uint(-360).value,0);time.sleep(.2)
            wait_until(lambda:len(read(out/'rich.json').get('mediaTimings',[]))>=2 and all(x['ok'] for x in read(out/'rich.json')['mediaTimings'][-2:]),'fixed formula/diagram did not render successfully',20)
            ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'media-valid.png')
        elif a.scenario=='ime':
            layout=u.LoadKeyboardLayoutW('00000804',0)
            u.PostMessageW(hwnd,0x50,0,layout);time.sleep(.7)
            imm=C.windll.imm32
            imm.ImmGetContext.argtypes=[C.c_void_p];imm.ImmGetContext.restype=C.c_void_p
            imm.ImmSetOpenStatus.argtypes=[C.c_void_p,C.c_bool]
            imm.ImmReleaseContext.argtypes=[C.c_void_p,C.c_void_p]
            context=imm.ImmGetContext(hwnd)
            if context:imm.ImmSetOpenStatus(context,True);imm.ImmReleaseContext(hwnd,context)
            actions.append(dict(kind='native-ime-request',layout=layout,context=bool(context),at=time.perf_counter()))
            for cycle in range(5):
                for letter in 'nihao':
                    key(ord(letter.upper()));time.sleep(.18)
                ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/f'ime-preedit-{cycle}.png')
                time.sleep(.5);key(0x20);time.sleep(.7)
                actions.append(dict(kind='ime-space-commit',at=time.perf_counter()))
        elif a.scenario=='resources':
            for i in range(100):
                key(0x41,ctrl=True)
                clipboard(f'# Image cache stress {i}\n\n![image]({base}/image-{i}.png)\n\nReadable body {i}.\n')
                t=time.perf_counter();key(0x56,ctrl=True)
                actions.append(dict(kind='image-cycle',index=i,at=t));time.sleep(.35)
            # Revisit the first URI after eviction: a second HTTP request proves
            # that the old loader entry was forgotten, not merely hidden.
            key(0x41,ctrl=True);clipboard(f'# Revisit evicted image\n\n![image]({base}/image-0.png)\n\nReadable body.\n');key(0x56,ctrl=True)
            wait_until(lambda:sum(r['path']=='/image-0.png' for r in requests)>=2,'old image loader was not evicted',15)
            actions.append(dict(kind='evicted-image-reloaded',at=time.perf_counter()))
            key(0x53,ctrl=True);wait_until(lambda:not read(out/'ui.json').get('dirty',True),'resource fixture save did not finish')
            other=out/'second.md';other.write_bytes(f'# Resource switch\n\n![image]({base}/image-101.png)\n\nSECOND FILE.\n'.encode())
            for index in range(24):
                target=other if index%2==0 else fixture
                key(0x4f,ctrl=True);wait_until(lambda:u.GetForegroundWindow()!=hwnd,'resource open dialog did not appear',5)
                key(0x4e,alt=True);clipboard(str(target));key(0x56,ctrl=True);key(0x0d)
                wait_until(lambda:(read(out/'ui.json').get('document_path') or '').endswith(target.name),'resource file switch did not finish',10)
                wait_until(current_preview_ready,'resource switched preview did not catch up',15)
                actions.append(dict(kind='resource-file-switch',index=index,target=target.name,at=time.perf_counter()));time.sleep(.25)
            ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'resource-switch-complete.png')
        elif a.scenario=='rich':
            wait_until(lambda:sum(m.get('ok') is True for m in read(out/'rich.json').get('mediaTimings',[]))>=2,'valid formula and diagram did not render',40)
            state=read(out/'rich.json')
            if any(abs(v.get('zoom',1)-1)>0.001 for v in state.get('viewStates',[])):
                raise RuntimeError('fresh media canvas did not retain default zoom 1')
            ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'default-zoom-formula-diagram.png')
            actions.append(dict(kind='default-zoom-media-ready',at=time.perf_counter()))
            for _ in range(7):
                u.SetCursorPos(1520,970);u.mouse_event(0x800,0,0,C.c_uint(-120).value,0);time.sleep(.2)
            ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'table-long-code.png')
            actions.append(dict(kind='native-scroll-table-code',at=time.perf_counter()))
        elif a.scenario=='threshold':
            before=read(out/'ui.json')['document_text_bytes']
            key(0x41,ctrl=True);clipboard('# Small after deletion\n\nSURVIVING BODY.');key(0x56,ctrl=True);time.sleep(1)
            small=read(out/'ui.json')['document_text_bytes']
            key(0x5a,ctrl=True);time.sleep(1)
            restored=read(out/'ui.json')['document_text_bytes']
            actions.append(dict(kind='threshold-delete-undo',before=before,small=small,restored=restored,at=time.perf_counter()))
            ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'threshold-undo.png')
        elif a.scenario=='anchor-middle':
            wait_until(current_preview_ready,'initial middle-anchor preview not ready',45)
            perf=read(out/'performance.json');bar=perf['preview_scrollbar'];x=40+(bar[0]+bar[2])*.5*dpi
            top=40+(bar[1]+4)*dpi;middle=40+(bar[1]+(bar[3]-bar[1])*.5)*dpi
            u.SetCursorPos(int(x),int(top));u.mouse_event(2,0,0,0,0);time.sleep(.1)
            for step in range(1,21):
                u.SetCursorPos(int(x),int(top+(middle-top)*step/20));time.sleep(.025)
            u.mouse_event(4,0,0,0,0);time.sleep(1)
            wait_until(lambda:read(out/'performance.json').get('preview_offset',0)>100000,'real scrollbar did not navigate into the document',15)
            wait_until(current_preview_ready,'middle before edit has no current geometry',30)
            reading_before=read(out/'reading-report.json');before_ui=read(out/'ui.json')
            before_geometry=reading_before['views']['main/preview']['geometry']
            if not .15*a.bytes<before_geometry['source_byte']<.85*a.bytes:
                raise RuntimeError('scrollbar fixture did not reach a genuine middle source range')
            ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'middle-before.png')
            clipboard(' LOCAL-MIDDLE-UPDATE');key(0x56,ctrl=True)
            wait_until(lambda:read(out/'ui.json').get('revision',-1)>before_ui['revision'],'middle source paste did not arrive')
            input_ui=read(out/'ui.json');wait_until(current_preview_ready,'middle updated preview not ready',45)
            ready_ui=read(out/'ui.json');reading_after=read(out/'reading-report.json')
            after_geometry=reading_after['views']['main/preview']['geometry']
            (out/'middle-anchor.json').write_text(json.dumps(dict(before=reading_before,after=reading_after,source_before=before_ui,source_after_input=input_ui,source_after_preview=ready_ui),ensure_ascii=False),encoding='utf8')
            ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'middle-after.png')
            if after_geometry['source_byte']!=before_geometry['source_byte']+20 or abs(after_geometry['viewport_y']-before_geometry['viewport_y'])>2:
                raise RuntimeError('genuine middle content anchor drifted after off-window paste')
            if input_ui.get('selection')!=ready_ui.get('selection') or input_ui.get('focused')!=ready_ui.get('focused'):
                raise RuntimeError('genuine middle preview update changed source selection/focus')
            actions.append(dict(kind='genuine-middle-anchor-passed',byte=before_geometry['source_byte'],at=time.perf_counter()))
        elif a.scenario=='matrix':
            for _ in range(20):key(0x41);time.sleep(.06)
            wait_until(lambda:read(out/'ui.json').get('document_text_bytes')==initial_file_bytes+20,'twenty native keys did not reach source',30)
            original_bytes=read(out/'ui.json')['document_text_bytes']
            actions.append(dict(kind='native-typing-20-keys',at=time.perf_counter()))
            huge='\n\nGIANT PASTE\n\n'+('A pasted paragraph.\n\n'*13000)
            clipboard(huge);t=time.perf_counter();key(0x56,ctrl=True)
            actions.append(dict(kind='giant-paste',bytes=len(huge.encode()),at=t))
            wait_until(lambda:read(out/'ui.json').get('document_text_bytes')==original_bytes+len(huge.encode()),'giant paste did not reach source',30)
            wait_until(current_preview_ready,'giant paste preview did not catch up',45)
            for vk,expected_bytes in [(0x5a,original_bytes),(0x59,original_bytes+len(huge.encode())),(0x5a,original_bytes)]:
                key(vk,ctrl=True)
                wait_until(lambda:read(out/'ui.json').get('document_text_bytes')==expected_bytes,'native giant history command did not reach source',20)
            wait_until(current_preview_ready,'history preview did not catch up',45)
            actions.append(dict(kind='giant-undo-redo-undo',at=time.perf_counter()))
            # Real preview wheel, then mode switches and native window resize.
            for _ in range(5):
                u.SetCursorPos(1370,750);u.mouse_event(0x800,0,0,C.c_uint(-120).value,0);time.sleep(.15)
            time.sleep(1)
            wait_until(current_preview_ready,'middle baseline has no current geometry',30)
            ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'matrix-scrolled.png')
            reading_before=read(out/'reading-report.json')
            before_ui=read(out/'ui.json');clipboard(' LOCAL-MIDDLE-UPDATE');key(0x56,ctrl=True)
            wait_until(lambda:read(out/'ui.json').get('revision',-1)>before_ui['revision'],'middle edit did not reach source')
            input_ui=read(out/'ui.json')
            wait_until(current_preview_ready,'middle edit has no current geometry',45)
            ready_ui=read(out/'ui.json');reading_after=read(out/'reading-report.json')
            ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'matrix-after-middle-edit.png')
            (out/'middle-anchor.json').write_text(json.dumps(dict(before=reading_before,after=reading_after,
                source_after_input=input_ui,source_after_preview=ready_ui),ensure_ascii=False),encoding='utf-8')
            before_geometry=reading_before['views']['main/preview'].get('geometry')
            after_geometry=reading_after['views']['main/preview'].get('geometry')
            if not before_geometry or not after_geometry or (
                after_geometry['source_byte']!=before_geometry['source_byte']+20
                or abs(after_geometry['viewport_y']-before_geometry['viewport_y'])>2):
                raise RuntimeError('middle preview content anchor drifted after prefix edit')
            if input_ui.get('selection')!=ready_ui.get('selection') or input_ui.get('focused')!=ready_ui.get('focused'):
                raise RuntimeError('preview update changed source selection/focus')
            def control(name):
                c=next(c for c in read(out/'ui.json')['controls'] if c['name']==name)
                x0,y0,x1,y1=c['rect'];cx0,cy0,cx1,cy1=c.get('clip',c['rect'])
                left,top,right,bottom=max(x0,cx0),max(y0,cy0),min(x1,cx1),min(y1,cy1)
                px,py=(left+right)/2,(top+bottom)/2
                if name=='main-source-editor':px,py=left+50,top+15
                click(40+px*dpi,40+py*dpi);time.sleep(.5)
            for name in ['Read','Split','Write','Split']:
                control(name);actions.append(dict(kind='mode',value=name,at=time.perf_counter()))
            for width in [1280,1550,1100,1550]:
                u.MoveWindow(hwnd,40,40,width,1024,True);time.sleep(.6)
                actions.append(dict(kind='resize',width=width,at=time.perf_counter()))
            # Delete down across both adapters, then undo into the large adapter.
            control('main-source-editor');key(0x41,ctrl=True);clipboard('# Small after deletion\n\nSURVIVING BODY.');key(0x56,ctrl=True);time.sleep(.8)
            actions.append(dict(kind='large-to-small',at=time.perf_counter()))
            ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'matrix-small.png')
            key(0x5a,ctrl=True);time.sleep(1)
            actions.append(dict(kind='small-to-large-undo',at=time.perf_counter()))
            if read(out/'ui.json')['document_text_bytes']<65536:
                raise RuntimeError('native undo did not restore large source')
            key(0x53,ctrl=True);time.sleep(.7)
            wait_until(lambda:not read(out/'ui.json').get('dirty',True),'manual save did not finish')
            other=out/'second.md';other.write_bytes(b'# SECOND DOCUMENT\n\nUNIQUE NEW DOCUMENT.\n')
            for target in [other,fixture,other,fixture]:
                key(0x4f,ctrl=True)
                wait_until(lambda:u.GetForegroundWindow()!=hwnd,'native open dialog did not appear',5)
                key(0x4e,alt=True);clipboard(str(target));key(0x56,ctrl=True);key(0x0d)
                wait_until(lambda:(read(out/'ui.json').get('document_path') or '').endswith(target.name),f'open did not install {target.name}',10)
                if read(out/'ui.json').get('autosave',{}).get('pending_navigation'):
                    raise RuntimeError('unexpected unsaved navigation guard')
                actions.append(dict(kind='open-file',target=target.name,at=time.perf_counter()))
                time.sleep(.2)
        else:
            while time.perf_counter()-begin < a.seconds:
                marker=f' {i:03d}'
                clipboard(marker); t=time.perf_counter(); key(0x56,ctrl=True)
                actions.append(dict(kind='paste',text=marker,at=t))
                i+=1; time.sleep(.1)
        time.sleep(1)
        for name,v,ctrl in [('delete',8,False),('undo',0x5a,True),('redo',0x59,True)]:
            t=time.perf_counter(); key(v,ctrl=ctrl); actions.append(dict(kind=name,at=t)); time.sleep(.7)
        clipboard(' 最终正文 FINAL-CONSISTENT')
        actions.append(dict(kind='final-paste',at=time.perf_counter())); key(0x56,ctrl=True)
        time.sleep(2)
        ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'final.png')
        key(0x53,ctrl=True); time.sleep(1)
        click(1450,330);time.sleep(2)
        idle_before=read(out/'performance.json').get('frame')
        time.sleep(4)
        idle_after=read(out/'performance.json').get('frame')
        (out/'idle.json').write_text(json.dumps(dict(before=idle_before,after=idle_after,seconds=4)),encoding='utf-8')
        outcome={'status':'passed'}
    except Exception as error:
        outcome={'status':'failed','reason':str(error)}
        ImageGrab.grab(bbox=(40,40,1590,1064)).save(out/'failure.png')
        raise
    finally:
        stop.set(); thread.join(); ff.stdin.close(); ff.wait()
        app.terminate(); app.wait()
        if server:
            server.shutdown(); (out/'media-requests.json').write_text(json.dumps(requests),encoding='utf-8')
        (out/'actions.json').write_text(json.dumps(actions,indent=2),encoding='utf-8')
        (out/'outcome.json').write_text(json.dumps(outcome),encoding='utf-8')
        (out/'frames.json').write_text(json.dumps(frames),encoding='utf-8')
        (out/'samples.jsonl').write_text('\n'.join(json.dumps(r) for r in reports),encoding='utf-8')
        (out/'environment.json').write_text(json.dumps(dict(exe=str(Path(a.exe).resolve()),
            exe_sha256=hashlib.sha256(Path(a.exe).read_bytes()).hexdigest(),bytes=len(text.encode()),
            scenario=a.scenario,position=a.position,
            initial_file_bytes=initial_file_bytes,
            latency_interval=a.interval,
            measurement_bytes=a.bytes if a.scenario=='latency' else None,
            requested_local_samples=a.samples if a.scenario=='latency' else 0,
            local_edit_method='replace the same eight-byte paragraph slot' if a.scenario=='latency' else None,
            launch_started=launch_at,launch_record_ended=launch_record_ended,
            dpi=dpi,frames=len(frames),native_input=True,screen_capture='PIL ImageGrab 20 fps target; actual timestamps in frames.json',
            script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            memory_scope='process-tree and main PID' if a.scenario in ('media','resources') else 'main PID only',
            note='Video capture is screen evidence; diagnostic update is not hardware presentation.'),indent=2),encoding='utf-8')

if __name__=='__main__': main()
