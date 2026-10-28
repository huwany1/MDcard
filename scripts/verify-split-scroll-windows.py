# SPDX-License-Identifier: AGPL-3.0-only
"""Isolated Windows scroll inputs and actual two-pane desktop captures."""
import argparse
import ctypes as C
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import threading
import time
import queue
import base64
import sys
import shutil
from PIL import ImageGrab,Image
import imageio_ffmpeg
import psutil

C.windll.user32.SetProcessDpiAwarenessContext(C.c_void_p(-4))
spec = importlib.util.spec_from_file_location('native', Path(__file__).with_name('verify-preview-windows.py'))
n = importlib.util.module_from_spec(spec)
spec.loader.exec_module(n)
u = n.u

def compact(reading):
    names=['presented_offset','viewport_rect','visible_runs','geometry','revision','buffer_id','open_generation','layout_pending','pending']
    return dict(main_buffer_id=reading.get('main_buffer_id'),main_revision=reading.get('main_revision'),views={key:{name:view.get(name) for name in names} for key,view in reading.get('views',{}).items()})

class DesktopRegion:
    """Top-down DIB copied directly from the desktop DC, without full-screen PIL capture."""
    class Header(C.Structure):
        _fields_=[('size',C.c_uint32),('width',C.c_int32),('height',C.c_int32),('planes',C.c_uint16),('bits',C.c_uint16),('compression',C.c_uint32),('image_size',C.c_uint32),('xppm',C.c_int32),('yppm',C.c_int32),('used',C.c_uint32),('important',C.c_uint32)]
    def __init__(self,box):
        self.box=box;self.width=box[2]-box[0];self.height=box[3]-box[1]
        self.g=C.windll.gdi32
        u.GetDC.argtypes=[C.c_void_p];u.GetDC.restype=C.c_void_p;u.ReleaseDC.argtypes=[C.c_void_p,C.c_void_p]
        self.g.CreateCompatibleDC.argtypes=[C.c_void_p];self.g.CreateCompatibleDC.restype=C.c_void_p
        self.g.CreateDIBSection.argtypes=[C.c_void_p,C.c_void_p,C.c_uint,C.c_void_p,C.c_void_p,C.c_uint];self.g.CreateDIBSection.restype=C.c_void_p
        self.g.SelectObject.argtypes=[C.c_void_p,C.c_void_p];self.g.SelectObject.restype=C.c_void_p
        self.g.BitBlt.argtypes=[C.c_void_p,C.c_int,C.c_int,C.c_int,C.c_int,C.c_void_p,C.c_int,C.c_int,C.c_uint32]
        self.g.DeleteObject.argtypes=[C.c_void_p];self.g.DeleteDC.argtypes=[C.c_void_p]
        self.g.GdiFlush.argtypes=[];self.g.GdiFlush.restype=C.c_bool
        self.dc=u.GetDC(None);self.memory=self.g.CreateCompatibleDC(self.dc);bits=C.c_void_p()
        header=self.Header(C.sizeof(self.Header),self.width,-self.height,1,32,0,self.width*self.height*4,0,0,0,0)
        self.bitmap=self.g.CreateDIBSection(self.dc,C.byref(header),0,C.byref(bits),None,0)
        if not self.bitmap:raise RuntimeError('CreateDIBSection failed')
        self.previous=self.g.SelectObject(self.memory,self.bitmap)
        self.buffer=(C.c_ubyte*(self.width*self.height*4)).from_address(bits.value)
    def grab(self):
        if not self.g.BitBlt(self.memory,0,0,self.width,self.height,self.dc,self.box[0],self.box[1],0x40CC0020):raise RuntimeError('desktop BitBlt failed')
        if not self.g.GdiFlush():raise RuntimeError('desktop DIB synchronization failed')
        return self.buffer
    def close(self):
        self.g.SelectObject(self.memory,self.previous);self.g.DeleteObject(self.bitmap);self.g.DeleteDC(self.memory);u.ReleaseDC(None,self.dc)

def main():
    p = argparse.ArgumentParser()
    p.add_argument('--exe', required=True)
    p.add_argument('--output', required=True)
    p.add_argument('--bytes', type=int, default=8192)
    p.add_argument('--samples', type=int, default=100)
    p.add_argument('--reduce-motion', action='store_true')
    p.add_argument('--scenario',choices=['latency','functional'],default='latency')
    p.add_argument('--fixture')
    p.add_argument('--geometry',action='store_true',help='Additional painted text diagnostics; use separately from latency sampling')
    p.add_argument('--baseline',action='store_true',help='Old binary without scroll-controller diagnostics; accepted only as a consumption/resource baseline')
    p.add_argument('--video-fps',type=int,default=0,help='Full-window video rate; 0 keeps latency capture to the two desktop ROIs only')
    p.add_argument('--profiles',action='store_true',help='Save independent one-pixel vertical desktop ink profiles for tolerant position/convergence analysis')
    a = p.parse_args()
    out = Path(a.output).resolve(); out.mkdir(parents=True, exist_ok=False)
    (out/'harness').mkdir();shutil.copy2(__file__,out/'harness'/Path(__file__).name);shutil.copy2(n.__file__,out/'harness'/'verify-preview-windows.py')
    (out/'command.json').write_text(json.dumps(dict(cwd=str(Path.cwd()),argv=sys.argv),ensure_ascii=False,indent=2),encoding='utf-8')
    row = 'Paragraph {i:06d} **anchor** 中文内容 scrolling synchronizes the same sentence.\n\n'
    text = ''.join(row.format(i=i) for i in range(a.bytes // len(row.format(i=0).encode())))
    text += 'x' * (a.bytes - len(text.encode()))
    if a.fixture:text=Path(a.fixture).read_text(encoding='utf-8')
    fixture = out / 'fixture.md'; fixture.write_bytes(text.encode())
    settings = out/'settings.json'
    settings.write_text(json.dumps(dict(language='en',appearance='Light',gpu_acceleration=True,reduce_motion=a.reduce_motion,font_size=15,autosave=False,recovery=False)))
    env = {k:v for k,v in os.environ.items() if not k.startswith('MDCARD_')}
    for k,v in dict(SETTINGS_PATH=settings,UI_REPORT=out/'ui.json',RICH_REPORT=out/'rich.json',PERFORMANCE_REPORT=out/'performance.json',SCROLL_REPORT=out/'scroll.json',READING_REPORT=out/'reading.json',READING_STATE_PATH=out/'reading-state.json',RECOVERY_DIR=out/'recovery',RENDERER_DIR=Path('App/renderer').resolve()).items(): env['MDCARD_'+k]=str(v)
    if (Path(a.exe).resolve().parent/'renderer').exists():env['MDCARD_RENDERER_DIR']=str(Path(a.exe).resolve().parent/'renderer')
    if a.geometry:env['MDCARD_READING_GEOMETRY_REPORT']='1'
    proc = subprocess.Popen([str(Path(a.exe).resolve()),str(fixture)],env=env)
    hwnd = None
    ENUM = C.WINFUNCTYPE(C.c_bool,C.c_void_p,C.c_void_p)
    def visit(h,_):
        nonlocal hwnd
        pid=C.c_ulong(); u.GetWindowThreadProcessId(h,C.byref(pid))
        title=C.create_unicode_buffer(512);u.GetWindowTextW(C.c_void_p(h),title,512)
        if pid.value==proc.pid and title.value and u.IsWindowVisible(C.c_void_p(h)): hwnd=h; return False
        return True
    deadline=time.perf_counter()+30
    while not hwnd and time.perf_counter()<deadline: u.EnumWindows(ENUM(visit),0);time.sleep(.1)
    if not hwnd: raise RuntimeError('no native window')
    u.MoveWindow(hwnd,40,40,1550,1024,True);u.SetWindowPos(hwnd,C.c_void_p(-1),0,0,0,0,0x43);u.SetForegroundWindow(hwnd)
    time.sleep(3)
    dpi=u.GetDpiForWindow(hwnd)/96
    deadline=time.perf_counter()+60
    while time.perf_counter()<deadline:
        r=n.read(out/'reading.json'); rich=n.read(out/'rich.json')
        if r.get('views',{}).get('main/preview',{}).get('geometry') and rich.get('mappedRevision'): break
        time.sleep(.1)
    u.MoveWindow(hwnd,40,40,1550,1024,True);time.sleep(.4)
    bounds=(C.c_long*4)();u.GetWindowRect(hwnd,C.byref(bounds));bbox=list(bounds)
    width=(bbox[2]-bbox[0])//2*2;height=(bbox[3]-bbox[1])//2*2
    bbox[2]=bbox[0]+width;bbox[3]=bbox[1]+height
    r=n.read(out/'reading.json')
    if a.scenario=='latency':
        # Stable latency is measured away from both scroll boundaries. Boundary
        # attempts are retained separately in the earlier/top-edge series.
        l,t,rr,b=r['views']['main/source']['viewport_rect'];u.SetCursorPos(int(bbox[0]+(l+rr)*dpi/2),int(bbox[1]+(t+b)*dpi/2))
        for _ in range(20):u.mouse_event(0x800,0,0,C.c_uint(-120).value,0);time.sleep(.025)
        time.sleep(.8);r=n.read(out/'reading.json')
    initial_ui=n.read(out/'ui.json')
    frames=[];actions=[];stop=threading.Event()
    view_rects=[r['views'][key]['viewport_rect'] for key in ['main/source','main/preview']]
    rois=[];centers=[]
    for rect in view_rects:
        l,t,rr,b=rect
        rois.append((int((l+15)*dpi),int((t+20)*dpi),int((rr-25)*dpi),int((min(b,t+450)-15)*dpi)))
        centers.append((int(bbox[0]+(l+rr)*dpi/2),int(bbox[1]+(t+b)*dpi/2)))
    ff=subprocess.Popen([imageio_ffmpeg.get_ffmpeg_exe(),'-y','-loglevel','error','-f','rawvideo','-vcodec','rawvideo','-s',f'{width}x{height}','-pix_fmt','rgb24','-r',str(a.video_fps),'-i','-','-c:v','libx264','-preset','ultrafast','-crf','24','-pix_fmt','yuv420p',str(out/'screen.mp4')],stdin=subprocess.PIPE) if a.video_fps else None
    video_queue=queue.Queue(maxsize=1);video_frames=[];capture_errors=[];capture_stats=dict(requested_roi_period_ms=8,video_attempts=0,video_queue_dropped=0)
    def encode():
        while True:
            item=video_queue.get()
            if item is None:return
            data,stamp=item;ff.stdin.write(data);video_frames.append(dict(frame=len(video_frames),captured_at=stamp))
    encoder=threading.Thread(target=encode) if ff else None
    if encoder:encoder.start()
    def capture():
        regions=[];full=None
        try:
            regions=[DesktopRegion((bbox[0]+l,bbox[1]+t,bbox[0]+r,bbox[1]+b)) for l,t,r,b in rois];full=DesktopRegion(bbox)
            next_video=0.;next_diag=0.;tail=[];memory={}
            while not stop.is_set():
                started=time.perf_counter();hashes=[];profiles=[]
                for region in regions:
                    raw=region.grab();hashes.append(hashlib.sha256(raw).hexdigest())
                    if a.profiles:
                        im=Image.frombytes('RGB',(region.width,region.height),bytes(raw),'raw','BGRX').convert('L')
                        profiles.append(base64.b64encode(im.resize((1,region.height),Image.Resampling.BOX).tobytes()).decode('ascii'))
                captured=time.perf_counter()
                frames.append(dict(frame=len(frames),capture_started=started,capture_finished=captured,hashes=hashes,profiles=profiles,ui_tail=tail,**memory))
                if ff and captured>=next_video:
                    capture_stats['video_attempts']+=1
                    raw=bytes(full.grab());stamp=time.perf_counter();im=Image.frombytes('RGB',(width,height),raw,'raw','BGRX')
                    try:video_queue.put_nowait((im.tobytes(),stamp))
                    except queue.Full:capture_stats['video_queue_dropped']+=1
                    next_video=captured+1/a.video_fps
                if captured>=next_diag:
                    trace=n.read(out/'scroll.json') if (out/'scroll.json').exists() else [];tail=trace[-2:] if isinstance(trace,list) else []
                    pm=psutil.Process(proc.pid).memory_info();memory=dict(private_bytes=getattr(pm,'private',pm.vms),working_set=pm.rss);next_diag=captured+.1
                time.sleep(max(0,.008-(time.perf_counter()-started)))
        except Exception as error:capture_errors.append(repr(error))
        finally:
            for region in regions:region.close()
            if full:full.close()
    th=threading.Thread(target=capture);th.start()
    try:
        ImageGrab.grab(bbox=bbox).save(out/'before.png')
        for pane,side in enumerate(['source','preview']) if a.scenario=='latency' else []:
            accepted=0;i=0
            while accepted<a.samples and i<a.samples*2+10:
                # Alternating bounded excursions avoid top/bottom censoring.
                delta=-120 if i%12<6 else 120
                u.SetCursorPos(*centers[pane]);time.sleep(.060)
                cursor=(C.c_long*2)();u.GetCursorPos(C.byref(cursor))
                if tuple(cursor)!=centers[pane]:raise RuntimeError(f'pointer not on requested {side} pane: {tuple(cursor)} != {centers[pane]}')
                rect=n.read(out/'reading.json')['views'][['main/source','main/preview'][pane]]['viewport_rect']
                local=((cursor[0]-bbox[0])/dpi,(cursor[1]-bbox[1])/dpi)
                if not (rect[0]<local[0]<rect[2] and rect[1]<local[1]<rect[3]):raise RuntimeError(f'requested {side} input is outside the actual pane: {local}, {rect}')
                before=n.read(out/'reading.json'); t=time.perf_counter()
                u.mouse_event(0x800,0,0,C.c_uint(delta).value,0)
                actions.append(dict(side=side,at=t,delta=delta,before=compact(before)))
                time.sleep(.32)
                actions[-1]['after']=compact(n.read(out/'reading.json'))
                trace=n.read(out/'scroll.json')
                actions[-1]['scroll_trace']=trace[-128:] if isinstance(trace,list) else []
                active=[r for r in actions[-1]['scroll_trace'] if t<=r.get('clock_ms',0)/1000<t+.32]
                consumed=abs(actions[-1]['after']['views'][['main/source','main/preview'][pane]]['presented_offset'][1]-before['views'][['main/source','main/preview'][pane]]['presented_offset'][1])>.5
                valid=consumed and (a.baseline or (active and all(r.get('current') for r in active) and any(r['pane']==['Source','Preview'][pane] and r.get('user') for r in active)))
                actions[-1]['accepted_stable_input']=bool(valid)
                if valid:accepted+=1
                i+=1
            if accepted<a.samples:raise RuntimeError(f'{side}: only {accepted}/{a.samples} valid inputs after {i} attempts')
        if a.scenario=='functional':
            def mark(kind,**extra):actions.append(dict(kind=kind,at=time.perf_counter(),**extra))
            def wheel(pane,delta):
                u.SetCursorPos(*centers[pane]);u.mouse_event(0x800,0,0,C.c_uint(delta).value,0)
            mark('continuous-60s-start');begin=time.perf_counter();i=0
            while time.perf_counter()-begin<60:
                wheel(0,-60 if i%40<20 else 60);i+=1;time.sleep(.05)
            mark('continuous-60s-stop',inputs=i)
            time.sleep(1)
            mark('takeover-start')
            for i in range(80):wheel(i%2,-120 if i%8<4 else 120);time.sleep(.04)
            mark('takeover-stop');time.sleep(.7)
            for pane in [0,1]:
                trace=n.read(out/'scroll.json')
                last=next(x for x in reversed(trace) if x['pane']==['Source','Preview'][pane]);bar=last['scrollbar']
                if not bar:mark('scrollbar-unavailable',pane=pane);continue
                l,t,rr,b=bar;x=bbox[0]+(l+rr)*dpi/2
                for fraction in [.7,.2,.9,.4]:
                    y=bbox[1]+(t+(b-t)*fraction)*dpi
                    mark('scrollbar-track-and-drag',pane=pane,fraction=fraction)
                    u.SetCursorPos(int(x),int(y));u.mouse_event(2,0,0,0,0);time.sleep(.08)
                    for d in [15,30,-20,0]:u.SetCursorPos(int(x),int(y+d));time.sleep(.04)
                    u.mouse_event(4,0,0,0,0);time.sleep(.4)
            # Actual window reflow, followed by restoring a valid Split width.
            mark('resize-start');u.MoveWindow(hwnd,40,40,1400,950,True);time.sleep(.8);u.MoveWindow(hwnd,40,40,1550,1024,True);time.sleep(1);mark('resize-stop')
            # Explicit page commands in both active panes, without editing text.
            for pane in [0,1]:
                u.SetCursorPos(*centers[pane]);u.mouse_event(2,0,0,0,0);u.mouse_event(4,0,0,0,0);time.sleep(.1)
                mark('page-down',pane=pane);n.key(0x22);time.sleep(.4)
            # Navigation to the source start, then native paste/undo/redo while
            # the installed preview is a stale snapshot.
            u.SetCursorPos(*centers[0]);u.mouse_event(2,0,0,0,0);u.mouse_event(4,0,0,0,0);n.key(0x24,ctrl=True)
            n.clipboard('INSERTED ABOVE VIEWPORT 中文\n\n');mark('paste-above');n.key(0x56,ctrl=True)
            wheel(1,-120);time.sleep(.03);wheel(0,-120);time.sleep(.7)
            mark('undo');n.key(0x5A,ctrl=True);time.sleep(.6);mark('redo');n.key(0x59,ctrl=True);time.sleep(.7)
            # Write/Read/Split and file-open identity transitions are retained
            # as separate native operations in the video and UI snapshots.
            for label in ['Write','Read','Split']:
                report=n.read(out/'ui.json')
                control=next(c for c in report['controls'] if c['name']==label)
                l,t,rr,b=control['rect'];u.SetCursorPos(int(bbox[0]+(l+rr)*dpi/2),int(bbox[1]+(t+b)*dpi/2));mark('mode',label=label);u.mouse_event(2,0,0,0,0);u.mouse_event(4,0,0,0,0);time.sleep(.5)
                actual=n.read(out/'ui.json');assert actual['mode']==label,(label,actual['mode'])
                actions[-1]['observed']=actual
            n.key(0x53,ctrl=True);time.sleep(.4)
            assert not n.read(out/'ui.json')['dirty'],'manual save did not complete'
            other=out/'other.md';other.write_text('# Other document\n\nIndependent identity.\n\n'*200,encoding='utf-8')
            for index,path in enumerate([other,fixture]*12):
                mark('open-file',index=index,path=str(path));n.key(0x4F,ctrl=True);time.sleep(.35);n.clipboard(str(path));n.key(0x56,ctrl=True);n.key(0x0D)
                deadline=time.perf_counter()+10
                while time.perf_counter()<deadline:
                    report=n.read(out/'ui.json')
                    if report.get('document_path') and Path(report['document_path']).resolve()==path:break
                    time.sleep(.05)
                else:raise RuntimeError('native file switch did not complete')
                time.sleep(.2);actions[-1]['observed']=n.read(out/'ui.json')
        ImageGrab.grab(bbox=bbox).save(out/'after.png')
        final_ui=n.read(out/'ui.json')
        invariance=dict(initial=initial_ui,final=final_ui,checks={key:initial_ui.get(key)==final_ui.get(key) for key in ['document_digest','revision','dirty','selection','focused']})
        (out/'document-invariance.json').write_text(json.dumps(invariance,ensure_ascii=False),encoding='utf-8')
        if a.scenario=='latency':assert all(invariance['checks'].values()),invariance['checks']
        quiet_before=n.read(out/'scroll.json')
        # Reading proof enrichment, persistence and scrollbar fade are allowed
        # to finish before the separate two-second settled-state observation.
        time.sleep(2);quiet_before=n.read(out/'scroll.json');idle_perf_before=n.read(out/'performance.json')
        time.sleep(2)
        quiet_after=n.read(out/'scroll.json')
        (out/'idle.json').write_text(json.dumps(dict(before=quiet_before,after=quiet_after,performance_before=idle_perf_before,performance_after=n.read(out/'performance.json'))),encoding='utf-8')
    finally:
        stop.set();th.join()
        if ff:video_queue.put(None);encoder.join();ff.stdin.close();ff.wait()
        for name,data in [('frames',frames),('actions',actions)]: (out/(name+'.json')).write_text(json.dumps(data,ensure_ascii=False),encoding='utf-8')
        (out/'video-frames.json').write_text(json.dumps(video_frames),encoding='utf-8');(out/'capture-errors.json').write_text(json.dumps(capture_errors),encoding='utf-8')
        capture_stats.update(roi_frames=len(frames),video_encoded=len(video_frames),roi_gaps_over_16ms=sum(b['capture_finished']-a['capture_finished']>.016 for a,b in zip(frames,frames[1:])))
        (out/'capture-stats.json').write_text(json.dumps(capture_stats),encoding='utf-8')
        pm=psutil.Process(proc.pid).memory_info()
        (out/'environment.json').write_text(json.dumps(dict(exe=str(Path(a.exe).resolve()),sha256=hashlib.sha256(Path(a.exe).read_bytes()).hexdigest(),bytes=len(text.encode()),baseline=a.baseline,geometry_diagnostics=a.geometry,profiles=a.profiles,dpi=dpi,window=bbox,rois=rois,reduce_motion=a.reduce_motion,font_size=15,source_wrap='Automatic (wrap <=256KiB, nowrap above)',renderer='OpenGL',capture='Win32 desktop BitBlt + GdiFlush + SHA256; ROI-only for latency; separate full-window functional video',video_fps=a.video_fps,video_frames=len(video_frames),recorder_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),hardware='../hardware.json',private_bytes=getattr(pm,'private',pm.vms),clock='Python perf_counter / Windows QPC; capture completion upper bound, not physical presentation')),encoding='utf-8')
        proc.terminate();proc.wait()

if __name__=='__main__':main()
