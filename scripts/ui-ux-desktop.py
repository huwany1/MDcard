"""Read-only monitor and window inventory for task-scoped automation."""
import ctypes as C
import json
import psutil
u=C.windll.user32
u.SetProcessDPIAware()
u.SetThreadDpiAwarenessContext.argtypes=[C.c_void_p]
u.SetThreadDpiAwarenessContext(C.c_void_p(-4))
for fn in ['GetForegroundWindow']:
    getattr(u,fn).restype=C.c_void_p
u.GetWindowRect.argtypes=[C.c_void_p,C.c_void_p]
u.GetWindowTextW.argtypes=[C.c_void_p,C.c_void_p,C.c_int]
u.IsWindowVisible.argtypes=[C.c_void_p]
u.GetWindowThreadProcessId.argtypes=[C.c_void_p,C.c_void_p]
items=[];monitors=[];fg=u.GetForegroundWindow()
@C.WINFUNCTYPE(C.c_bool,C.c_void_p,C.c_void_p)
def win(h,l):
    if u.IsWindowVisible(h):
        p=C.c_ulong();r=(C.c_long*4)();b=C.create_unicode_buffer(512)
        u.GetWindowThreadProcessId(h,C.byref(p));u.GetWindowRect(h,C.byref(r));u.GetWindowTextW(h,b,512)
        try:name=psutil.Process(p.value).name()
        except psutil.Error:name=''
        items.append(dict(hwnd=h,pid=p.value,exe=name,title=b.value,rect=list(r),foreground=h==fg))
    return True
@C.WINFUNCTYPE(C.c_bool,C.c_void_p,C.c_void_p,C.c_void_p,C.c_void_p)
def mon(m,d,r,l):
    monitors.append(dict(handle=m,bounds=list(C.cast(r,C.POINTER(C.c_long*4)).contents)))
    return True
u.EnumWindows(win,0);u.EnumDisplayMonitors(0,0,mon,0)
print(json.dumps(dict(monitors=monitors,windows=items),ensure_ascii=False))
