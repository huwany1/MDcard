# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$Exe="$PSScriptRoot\..\dist\windows\MDcard.exe")
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;using System.Runtime.InteropServices;
public class CornerProof {
 [StructLayout(LayoutKind.Sequential)]public struct Rect{public int Left,Top,Right,Bottom;}
 [DllImport("user32.dll")]public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")]public static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")]public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int h2,bool b);
 [DllImport("user32.dll")]public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int h2,uint flags);
 [DllImport("user32.dll")]public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")]public static extern void mouse_event(uint f,uint x,uint y,uint d,UIntPtr e);
 [DllImport("user32.dll")]public static extern bool IsZoomed(IntPtr h);
 [DllImport("user32.dll")]public static extern bool IsIconic(IntPtr h);
 [DllImport("user32.dll")]public static extern bool ShowWindow(IntPtr h,int c);
 [DllImport("dwmapi.dll")]public static extern int DwmGetWindowAttribute(IntPtr h,uint a,out uint v,uint s);
}
'@
[void][CornerProof]::SetThreadDpiAwarenessContext([IntPtr]::new(-4))
$root=Split-Path $PSScriptRoot
$out=Join-Path $root 'evidence\window-corners\windows'
New-Item -ItemType Directory -Force $out|Out-Null
$backdrop=[Windows.Forms.Form]::new();$backdrop.FormBorderStyle='None';$backdrop.StartPosition='Manual';$backdrop.Location=[Drawing.Point]::new(40,40);$backdrop.Size=[Drawing.Size]::new(1600,1050);$backdrop.BackColor=[Drawing.Color]::FromArgb(219,92,172);$backdrop.Show();[Windows.Forms.Application]::DoEvents()
function Bounds {$r=[CornerProof+Rect]::new();[void][CornerProof]::GetWindowRect($script:app.MainWindowHandle,[ref]$r);return $r}
function Keys([string]$value){[Windows.Forms.SendKeys]::SendWait($value);Start-Sleep -Milliseconds 400}
function Click([double]$x,[double]$y){$r=Bounds;$s=[CornerProof]::GetDpiForWindow($script:app.MainWindowHandle)/96.0;[void][CornerProof]::SetCursorPos($r.Left+[int]($x*$s),$r.Top+[int]($y*$s));[CornerProof]::mouse_event(2,0,0,0,[UIntPtr]::Zero);[CornerProof]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 600}
function Attribute {[uint32]$v=0;$hr=[CornerProof]::DwmGetWindowAttribute($script:app.MainWindowHandle,33,[ref]$v,4);if($hr-ne0){throw "DWM attribute failed $hr"};return $v}
function Shot([string]$name,[switch]$Surround){[void][CornerProof]::SetCursorPos(1500,1000);Start-Sleep -Milliseconds 600;$r=Bounds;$margin=if($Surround){20}else{0};$b=[Drawing.Bitmap]::new($r.Right-$r.Left+2*$margin,$r.Bottom-$r.Top+2*$margin);$g=[Drawing.Graphics]::FromImage($b);$g.CopyFromScreen($r.Left-$margin,$r.Top-$margin,0,0,$b.Size);$b.Save((Join-Path $out "$name.png"));$g.Dispose();$b.Dispose()}
$cases=@()
try {
 foreach($case in @(@{name='light-opengl';appearance='Light';gpu=$true},@{name='dark-cpu';appearance='Dark';gpu=$false})) {
  $settings=Join-Path $out ($case.name+'-settings.json');@{appearance=$case.appearance;accent=1;gpu_acceleration=$case.gpu;reduce_motion=$false;font_size=15;language='en'}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding UTF8
  $start=[Diagnostics.ProcessStartInfo]::new((Resolve-Path -LiteralPath $Exe).Path);$start.UseShellExecute=$false;$start.Environment['MDCARD_SETTINGS_PATH']=$settings;$start.Environment['MDCARD_RENDERER_REPORT']=(Join-Path $out ($case.name+'-renderer.json'))
  $script:app=[Diagnostics.Process]::Start($start);Start-Sleep -Seconds 3;$script:app.Refresh();if($script:app.HasExited-or$script:app.MainWindowHandle-eq0){throw 'No window'}
  [void][CornerProof]::MoveWindow($script:app.MainWindowHandle,160,140,1100,740,$true);[void][CornerProof]::SetWindowPos($backdrop.Handle,[IntPtr]::Zero,0,0,0,0,0x53);[void][CornerProof]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::Zero,0,0,0,0,0x43);[void][CornerProof]::SetForegroundWindow($script:app.MainWindowHandle);Start-Sleep -Milliseconds 500
  if((Attribute)-ne2){throw 'Restored window did not opt in to native rounding'}
  Shot ($case.name+'-restored') -Surround
  Click 71 27
  if(![CornerProof]::IsZoomed($script:app.MainWindowHandle)){throw 'Maximize failed'}
  if((Attribute)-ne1){throw 'Maximized window did not request square corners'}
  Shot ($case.name+'-maximized')
  Click 71 27
  if([CornerProof]::IsZoomed($script:app.MainWindowHandle)-or(Attribute)-ne2){throw 'Restore did not restore rounding'}
  Shot ($case.name+'-restored-again') -Surround
  Click 48 27
  if(![CornerProof]::IsIconic($script:app.MainWindowHandle)){throw 'Minimize failed'}
  [void][CornerProof]::ShowWindow($script:app.MainWindowHandle,9);[void][CornerProof]::SetForegroundWindow($script:app.MainWindowHandle);Start-Sleep -Milliseconds 500
  if((Attribute)-ne2){throw 'Minimize/restore lost corners'}
  Keys '^q';if(!$script:app.WaitForExit(5000)){throw 'Clean exit failed'}
  $cases+=@{name=$case.name;native_dwm_round_preference=2;maximized_preference=1;restore_preference=2;maximize_restore=$true;minimize_restore=$true;exit_code=$script:app.ExitCode}
 }
 @{verification_id=[guid]::NewGuid().ToString();date='2026-10-19';timezone='Asia/Shanghai';exe_sha256=(Get-FileHash -LiteralPath $Exe).Hash;cases=$cases;visual_corner_check='Review screen captures with pink background; native surface screenshots alone cannot show DWM clipping.'}|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $out 'result.json') -Encoding UTF8
} finally {if($null-ne$script:app-and!$script:app.HasExited){$script:app.CloseMainWindow()|Out-Null};$backdrop.Close();$backdrop.Dispose()}
