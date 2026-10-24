# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$Exe = "$PSScriptRoot\..\dist\windows\MDcard.exe")
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class MdcardControls {
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr w, out Rect r);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr w);
 [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr w);
 [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr w);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr w);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr w,int c);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint x,uint y,uint d,UIntPtr e);
}
'@
[void][MdcardControls]::SetThreadDpiAwarenessContext([IntPtr]::new(-4))
$output = Join-Path (Split-Path $PSScriptRoot) 'evidence\windows\controls'
New-Item -ItemType Directory -Force $output | Out-Null
$app = Start-Process -FilePath (Resolve-Path -LiteralPath $Exe).Path -PassThru
Start-Sleep -Seconds 3
$app.Refresh()
$handle = $app.MainWindowHandle
$scale = [MdcardControls]::GetDpiForWindow($handle)/96.0
function Rect { $r = [MdcardControls+Rect]::new(); [void][MdcardControls]::GetWindowRect($handle,[ref]$r); return $r }
function Point([int]$X,[int]$Y) { $r = Rect; [void][MdcardControls]::SetCursorPos($r.Left+$X,$r.Top+$Y) }
function Down { [MdcardControls]::mouse_event(2,0,0,0,[UIntPtr]::Zero) }
function Up { [MdcardControls]::mouse_event(4,0,0,0,[UIntPtr]::Zero) }
function Lamp([int]$Index) { Point ([int]((25+23*$Index)*$scale)) ([int](27*$scale)); Down; Up; Start-Sleep -Milliseconds 600 }
[void][MdcardControls]::SetForegroundWindow($handle)
Lamp 2
if (![MdcardControls]::IsZoomed($handle)) { throw 'Green control did not maximize' }
Lamp 2
if ([MdcardControls]::IsZoomed($handle)) { throw 'Green control did not restore' }
$before = Rect
Point ([int](($before.Right-$before.Left)/2)) ([int](27*$scale))
Down
Start-Sleep -Milliseconds 100
[void][MdcardControls]::SetCursorPos([int](($before.Left+$before.Right)/2+10),[int]($before.Top+27*$scale+5))
Start-Sleep -Milliseconds 250
[void][MdcardControls]::SetCursorPos([int](($before.Left+$before.Right)/2+70),[int]($before.Top+27*$scale+35))
Start-Sleep -Milliseconds 300
Up
Start-Sleep -Milliseconds 500
$moved = Rect
if ($moved.Left -eq $before.Left -and $moved.Top -eq $before.Top) { throw 'Titlebar drag did not move window' }
Point ($moved.Right-$moved.Left-20) ($moved.Bottom-$moved.Top-15)
Down
Start-Sleep -Milliseconds 100
[void][MdcardControls]::SetCursorPos($moved.Right-10,$moved.Bottom-5)
Start-Sleep -Milliseconds 250
[void][MdcardControls]::SetCursorPos($moved.Right+50,$moved.Bottom+30)
Start-Sleep -Milliseconds 300
Up
Start-Sleep -Milliseconds 500
$resized = Rect
if (($resized.Right-$resized.Left) -eq ($moved.Right-$moved.Left) -and ($resized.Bottom-$resized.Top) -eq ($moved.Bottom-$moved.Top)) { throw 'Resize grip did not resize' }
Lamp 1
if (![MdcardControls]::IsIconic($handle)) { throw 'Yellow control did not minimize' }
[void][MdcardControls]::ShowWindow($handle,9)
[void][MdcardControls]::SetForegroundWindow($handle)
Start-Sleep -Milliseconds 500
[System.Windows.Forms.SendKeys]::SendWait('^n')
Start-Sleep -Milliseconds 300
[System.Windows.Forms.SendKeys]::SendWait('dirty close')
Start-Sleep -Milliseconds 300
Lamp 0
$app.Refresh()
if ($app.HasExited) { throw 'Red control lost dirty document' }
$r = Rect
$bmp = [System.Drawing.Bitmap]::new($r.Right-$r.Left,$r.Bottom-$r.Top)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.Left,$r.Top,0,0,$bmp.Size)
$bmp.Save((Join-Path $output 'dirty-close.png'))
$g.Dispose(); $bmp.Dispose()
Point ([int](($r.Right-$r.Left)/2-88*$scale)) ([int](($r.Bottom-$r.Top)/2+55*$scale))
Down; Up
if (!$app.WaitForExit(5000)) { throw 'Discard on Close did not close' }
@{maximize_restore=$true; titlebar_drag=$true; resize_grip=$true; minimize_restore=$true; dirty_red_close_protected=$true; dpi_scale=$scale; before=$before; moved=$moved; resized=$resized} | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $output 'result.json') -Encoding UTF8
