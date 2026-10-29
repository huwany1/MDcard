# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$Exe = "$PSScriptRoot\..\target\release\mdcard.exe",[switch]$CaptureOnly,[switch]$FunctionsOnly)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public class MdcardUi {
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int Left,Top,Right,Bottom;}
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,IntPtr p);
 [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
 [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint from,uint to,bool attach);
 [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int cx,int cy,uint flags);
 [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int h2,bool b);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint x,uint y,uint d,UIntPtr e);
}
'@
[void][MdcardUi]::SetThreadDpiAwarenessContext([IntPtr]::new(-4))
$root=Split-Path $PSScriptRoot
$output=Join-Path $root 'evidence\ui-gpu\windows'
New-Item -ItemType Directory -Force $output | Out-Null
$settings=Join-Path $output 'settings.json'
function Bounds { $r=[MdcardUi+Rect]::new();[void][MdcardUi]::GetWindowRect($script:app.MainWindowHandle,[ref]$r);return $r }
function Shot([string]$Name) { Start-Sleep -Milliseconds 600;$r=Bounds;$b=[Drawing.Bitmap]::new($r.Right-$r.Left,$r.Bottom-$r.Top);$g=[Drawing.Graphics]::FromImage($b);$g.CopyFromScreen($r.Left,$r.Top,0,0,$b.Size);$b.Save((Join-Path $output "$Name.png"));$g.Dispose();$b.Dispose() }
function Keys([string]$Keys) {[Windows.Forms.SendKeys]::SendWait($Keys);Start-Sleep -Milliseconds 300}
function Click([float]$X,[float]$Y) {$r=Bounds;$s=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0;[void][MdcardUi]::SetCursorPos($r.Left+[int]($X*$s),$r.Top+[int]($Y*$s));[MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero);[MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 350}
function Launch([string]$Report,[string]$Gpu='',[string]$Document='') {
 $launch=[Diagnostics.ProcessStartInfo]::new((Resolve-Path -LiteralPath $Exe).Path);$launch.UseShellExecute=$false
 if($env:MDCARD_QA_CLEAN_ENV-eq'1') {
  foreach($name in @('MDCARD_RENDERER_DIR','MDCARD_CHROMIUM','MDCARD_JAVA','MDCARD_PANDOC','JAVA_HOME','NODE_PATH','NODE_OPTIONS','CARGO_HOME','RUSTUP_HOME')) {[void]$launch.Environment.Remove($name)}
  $launch.Environment['PATH']=Join-Path $env:SystemRoot 'System32'
  $launch.WorkingDirectory=$output
 }
 $launch.Environment['MDCARD_SETTINGS_PATH']=$settings;$launch.Environment['MDCARD_RENDERER_REPORT']=(Join-Path $output "$Report.json")
 if($Gpu) {$launch.ArgumentList.Add('--gpu');$launch.ArgumentList.Add($Gpu)}
 if($Document) {$launch.ArgumentList.Add($Document)}
 # A present empty path still enables Rust var_os diagnostics. Remove empty
 # report keys from the actual child block while preserving explicit reports.
 foreach($name in @($launch.Environment.Keys)) {
  if($name.StartsWith('MDCARD_')-and$name.EndsWith('_REPORT')-and[string]::IsNullOrWhiteSpace($launch.Environment[$name])) {[void]$launch.Environment.Remove($name)}
 }
 $startupClock=[Diagnostics.Stopwatch]::StartNew()
 $script:app=[Diagnostics.Process]::Start($launch)
 if($env:MDCARD_QA_STARTUP_TIMING_PATH) {
  do { Start-Sleep -Milliseconds 10; $script:app.Refresh() } while(!$script:app.HasExited-and($script:app.MainWindowHandle-eq0-or!(Test-Path -LiteralPath $env:MDCARD_UI_REPORT))-and$startupClock.Elapsed.TotalSeconds-lt30)
  @{process_to_native_window_and_first_report_seconds=$startupClock.Elapsed.TotalSeconds;measurement='native window + first diagnostic report; not scanout/presentation'}|ConvertTo-Json|Set-Content -LiteralPath $env:MDCARD_QA_STARTUP_TIMING_PATH -Encoding utf8
 } else { Start-Sleep -Seconds 3 }
 $script:app.Refresh()
 if($script:app.HasExited -or $script:app.MainWindowHandle -eq 0) {throw 'No app window'}
 [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,50,50,1550,1025,$true)
 # Windows may reject SetForegroundWindow while another process owns the foreground.
 # Temporarily raise the actual test window before clicking it, then restore normal z-order.
 [void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-1),0,0,0,0,0x43)
 [void][MdcardUi]::SetForegroundWindow($script:app.MainWindowHandle);Start-Sleep -Milliseconds 350
 Click 500 27
 [void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-2),0,0,0,0,0x43)
 for($focusAttempt=0;$focusAttempt-lt5-and[MdcardUi]::GetForegroundWindow()-ne$script:app.MainWindowHandle;$focusAttempt++) {
  Start-Sleep -Milliseconds 250;$script:app.Refresh()
  $currentThread=[MdcardUi]::GetCurrentThreadId();$foregroundThread=[MdcardUi]::GetWindowThreadProcessId([MdcardUi]::GetForegroundWindow(),[IntPtr]::Zero)
  [void][MdcardUi]::AttachThreadInput($currentThread,$foregroundThread,$true)
  [void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-1),0,0,0,0,0x43)
  [void][MdcardUi]::BringWindowToTop($script:app.MainWindowHandle)
  [void][MdcardUi]::SetForegroundWindow($script:app.MainWindowHandle);Click 500 27
  [void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-2),0,0,0,0,0x43)
  [void][MdcardUi]::AttachThreadInput($currentThread,$foregroundThread,$false)
 }
 if ([MdcardUi]::GetForegroundWindow() -ne $script:app.MainWindowHandle) {
  @{pid=$script:app.Id;main_handle=$script:app.MainWindowHandle.ToInt64();foreground=[MdcardUi]::GetForegroundWindow().ToInt64();main_title=$script:app.MainWindowTitle;utc=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json|Set-Content (Join-Path $output 'focus-failure.json')
  throw 'Actual test window did not receive keyboard focus'
 }
}
if($FunctionsOnly) {return}
if($CaptureOnly) {$script:app=Get-Process mdcard;Shot 'inspect';return}
function OpenPrefs {Keys '^+p'}
function AssertSettings([string]$Appearance,[int]$Accent) {
 $p=Get-Content -Raw -LiteralPath $settings | ConvertFrom-Json
 if($p.appearance -ne $Appearance -or $p.accent -ne $Accent) {throw "Settings mismatch: $($p | ConvertTo-Json -Compress)"}
}
function CloseApp {Keys '^q';if(!$script:app.WaitForExit(5000)){throw 'App did not close cleanly'}}
Launch 'gpu-renderer' 'on'
$matrix=@()
foreach($appearance in @('Light','Dark')) {
 foreach($accent in 0..5) {
  OpenPrefs
  Click $(if($appearance -eq 'Dark'){532}else{472}) 258
  Start-Sleep -Milliseconds 700
  Click (461+40*$accent) 321
  $check=Get-Content -Raw -LiteralPath $settings|ConvertFrom-Json
  if($check.accent -ne $accent) {Click (461+40*$accent) 321}
  AssertSettings $appearance $accent
  Shot "preferences-$($appearance.ToLower())-$accent"
  Keys '{ESC}'
  Shot "workspace-$($appearance.ToLower())-$accent"
  $matrix+=@{appearance=$appearance;accent=$accent;real_click=$true;persisted=$true}
 }
}
OpenPrefs;Click 461 360;Shot 'reduce-motion-on';Keys '{ESC}'
if(!(Get-Content -Raw $settings|ConvertFrom-Json).reduce_motion){throw 'Reduce motion did not persist'}
Keys '^b';Shot 'sidebar-hidden';Keys '^b'
Click 1190 82;Shot 'read-dark';Click 1063 82;Shot 'write-dark';Click 1126 82
OpenPrefs;Click 461 360;Click 461 503;Shot 'gpu-off-pending';Keys '{ESC}'
if((Get-Content -Raw $settings|ConvertFrom-Json).gpu_acceleration){throw 'GPU-off did not persist'}
CloseApp
Launch 'cpu-renderer'
if((Get-Content -Raw (Join-Path $output 'cpu-renderer.json')|ConvertFrom-Json).renderer -notmatch 'Microsoft Basic Render Driver.*Cpu / Dx12'){throw 'GPU off is not WARP CPU'}
Shot 'cpu-dark'
OpenPrefs;Shot 'cpu-preferences';Keys '{ESC}'
# Narrow logical minimum at Windows 125% DPI: toolbar, Read, popup, and scrollable Preferences.
[void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,80,80,775,525,$true);Start-Sleep -Milliseconds 500
Shot 'compact-dark';Click 570 82;Shot 'compact-read-dark'
Click 230 82;Shot 'compact-menu-dark';Keys '{ESC}';OpenPrefs;Shot 'compact-preferences-dark';Keys '{ESC}'
[void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,50,50,1550,1025,$true);Start-Sleep -Milliseconds 500
OpenPrefs
# CPU renderer line is shorter, so the centered modal is 6 logical pixels shorter.
Click 461 509
Shot 'gpu-on-pending';Keys '{ESC}'
if(!(Get-Content -Raw $settings|ConvertFrom-Json).gpu_acceleration){throw 'GPU-on did not persist'}
CloseApp
Launch 'gpu-restored-renderer'
Shot 'gpu-restored-dark'
if((Get-Content -Raw (Join-Path $output 'gpu-restored-renderer.json')|ConvertFrom-Json).renderer -notmatch 'OpenGL.*NVIDIA'){throw 'Hardware renderer was not restored'}
OpenPrefs;Start-Sleep -Milliseconds 600;Click 585 431;Click 585 431;Keys '^a';Keys '23';Keys '{ENTER}';Keys '{ESC}';Shot 'font-size-23-dark'
if((Get-Content -Raw $settings|ConvertFrom-Json).font_size -lt 22.9){throw '23px font size not applied'}
[void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,80,80,775,525,$true);Start-Sleep -Milliseconds 500
OpenPrefs
$r=Bounds;[void][MdcardUi]::SetCursorPos($r.Left+500,$r.Top+350)
for($i=0;$i -lt 8;$i++) {[MdcardUi]::mouse_event(0x800,0,0,[uint32]::MaxValue-119,[UIntPtr]::Zero);Start-Sleep -Milliseconds 60}
Shot 'compact-preferences-gpu-scrolled';Keys '{ESC}';Shot 'compact-font-size-23-dark'
[void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,50,50,1550,1025,$true);Start-Sleep -Milliseconds 500
Keys '^n'
$oldClipboard=[Windows.Forms.Clipboard]::GetDataObject()
try {Set-Clipboard -Value '# Dirty modal verification';Keys '^v'} finally {if($null-ne$oldClipboard){[Windows.Forms.Clipboard]::SetDataObject($oldClipboard,$true)}}
Keys '^n';Shot 'dirty-dialog-dark';Keys '{ESC}'
Keys '^+p';Shot 'dirty-preferences-dark';Keys '{ESC}'
# Clear the new document back to its saved empty baseline, then start with invalid UTF-8.
Click 500 200;Keys '^a';Keys '{DELETE}';CloseApp
$invalid=Join-Path $output 'invalid-utf8.md';[IO.File]::WriteAllBytes($invalid,[byte[]]@(255,254,255))
Launch 'error-renderer' '' $invalid
Shot 'error-dialog-dark';Keys '{ESC}'
CloseApp
@{verification_id=[Guid]::NewGuid().ToString();date=[DateTimeOffset]::UtcNow.ToString('o');timezone='UTC';matrix=$matrix;gpu_off_real_click=$true;cpu_only_after_restart=$true;gpu_on_real_click=$true;hardware_restored_after_restart=$true;reduce_motion_persisted=$true;compact_logical='620x420';preferences_shortcut='Ctrl+Shift+P';exe_sha256=(Get-FileHash -LiteralPath $Exe).Hash} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding UTF8
Write-Output $output
