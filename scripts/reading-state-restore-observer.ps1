# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
# Observe short restore frames outside the PowerShell resize call. This reads
# diagnostics and sends one real Win32 wheel event to the owned test window.
Add-Type @'
using System;
using System.IO;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using System.Runtime.InteropServices;
using System.Diagnostics;
using System.Globalization;
using System.Collections.Generic;
public static class ReadingRestoreObserver {
 [StructLayout(LayoutKind.Sequential)] struct Rect { public int Left,Top,Right,Bottom; }
 [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")] static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] static extern void mouse_event(uint flags,uint dx,uint dy,uint data,UIntPtr extra);
 public static Task<string> Start(IntPtr window,string path,string expectedId,double[] fallback) {
  var ready=new ManualResetEventSlim(false);
  var observer=Task.Run(() => {
   var deadline=Stopwatch.StartNew();
   ready.Set();
   while(deadline.ElapsedMilliseconds<5000) {
    try {
     string raw;
     using(var f=new FileStream(path,FileMode.Open,FileAccess.Read,FileShare.ReadWrite|FileShare.Delete))
     using(var r=new StreamReader(f)) raw=r.ReadToEnd();
     using(var json=JsonDocument.Parse(raw)) {
      if(json.RootElement.GetProperty("main_id").GetRawText()!=expectedId) { Thread.Sleep(1); continue; }
      var view=json.RootElement.GetProperty("views").GetProperty("main/preview");
      if(view.GetProperty("pending").ValueKind==JsonValueKind.Object) {
       var delivery=Stopwatch.StartNew();
       if(GetForegroundWindow()!=window) return "{\"observed\":false,\"error\":\"Owned window lost foreground\"}";
       var p=view.GetProperty("viewport_rect"); Rect bounds;
       if(!GetWindowRect(window,out bounds)) throw new Exception("No owned window bounds");
       var scale=GetDpiForWindow(window)/96.0;
       double x0=fallback[0],y0=fallback[1],x1=fallback[2],y1=fallback[3];
       if(p.ValueKind==JsonValueKind.Array) { x0=p[0].GetDouble();y0=p[1].GetDouble();x1=p[2].GetDouble();y1=p[3].GetDouble(); }
       SetCursorPos(bounds.Left+(int)((x0+x1)/2*scale),bounds.Top+(int)((y0+.97*(y1-y0))*scale));
       mouse_event(0x800,0,0,unchecked((uint)-120),UIntPtr.Zero);
       return "{\"observed\":true,\"pending_to_native_wheel_ms\":"+
         delivery.Elapsed.TotalMilliseconds.ToString("R",CultureInfo.InvariantCulture)+",\"frame\":"+raw+"}";
      }
     }
    } catch(IOException) {} catch(JsonException) {} catch(InvalidOperationException) {} catch(KeyNotFoundException) {}
    Thread.Sleep(1);
   }
   return "{\"observed\":false,\"error\":\"No pending preview frame observed within 5 seconds\"}";
  });
  if(!ready.Wait(2000)) throw new TimeoutException("Observer thread did not start");
  return observer;
 }
}
'@
