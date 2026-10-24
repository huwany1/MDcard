# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
# Bounded retry for the OS-wide clipboard lock. Exhaustion still fails the test.
Add-Type -AssemblyName System.Windows.Forms
if(-not ('WorkbenchClipboard' -as [type])) {
 Add-Type -ReferencedAssemblies ([Windows.Forms.Clipboard].Assembly.Location) @'
using System;
using System.Windows.Forms;
public static class WorkbenchClipboard {
 [System.Runtime.InteropServices.DllImport("kernel32.dll")] static extern void Sleep(uint ms);
 public static void SetText(string value) { for(int i=0;;i++) { try { Clipboard.SetText(value); return; } catch(System.Runtime.InteropServices.ExternalException) { if(i==7) throw; Sleep((uint)(40+20*i)); } } }
 public static void SetDataObject(object value,bool persist) { for(int i=0;;i++) { try { Clipboard.SetDataObject(value,persist); return; } catch(System.Runtime.InteropServices.ExternalException) { if(i==7) throw; Sleep((uint)(40+20*i)); } } }
 public static string GetText() { for(int i=0;;i++) { try { return Clipboard.GetText(); } catch(System.Runtime.InteropServices.ExternalException) { if(i==7) throw; Sleep((uint)(40+20*i)); } } }
 public static IDataObject GetDataObject() { for(int i=0;;i++) { try { return Clipboard.GetDataObject(); } catch(System.Runtime.InteropServices.ExternalException) { if(i==7) throw; Sleep((uint)(40+20*i)); } } }
}
'@
}
