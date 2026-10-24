# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
 [Parameter(Mandatory=$true)][string]$Exe,
 [Parameter(Mandatory=$true)][string]$Stage,
 [ValidateSet('ordinary','one-mib','ten-mib')][string]$Size='ordinary',
 [ValidateSet('Off','On')][string]$Diagnostics='Off'
)
$ErrorActionPreference='Stop'
. "$PSScriptRoot/verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
. "$PSScriptRoot/workbench-native-input.ps1"
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class PrecisionPerformanceInput {
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern IntPtr GetKeyboardLayout(uint t);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr LoadKeyboardLayout(string n,uint f);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
}
'@
if($Stage-notmatch'^[A-Za-z0-9_-]+$'){throw 'Use a simple unique stage'}
$output=Join-Path $root "evidence/reading-precision/master/$Stage"
if(Test-Path -LiteralPath $output){throw 'Retain previous evidence; use a fresh stage'}
New-Item -ItemType Directory $output|Out-Null
$work=Join-Path $root 'artifacts/reading-precision/performance-inputs'
$store=Join-Path $output 'reading-state.json'
$apps=[Collections.Generic.List[object]]::new()
$inputs=[Collections.Generic.List[object]]::new()
$rss=[Collections.Generic.List[object]]::new()
$record=@{stage=$Stage;size=$Size;diagnostics=$Diagnostics;started=[DateTimeOffset]::UtcNow.ToString('o');
 platform='Windows 11 build 26100 x64; actual HWND/native wheel; function wall, not thread CPU or display latency';
 binary=(Resolve-Path -LiteralPath $Exe).Path;binary_sha256=(Get-FileHash -LiteralPath $Exe).Hash.ToLowerInvariant();
 harness_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant();
 protocol='Two real MD/TXT processes, requested Win32 size1240x820; actual screenshot dimensions and DPI recorded and required equal in the pair. Font15, native CtrlE from initial Split to Read; A100down/B55down/A25up at65ms; 1s tail each. UI/reading/rich diagnostics OFF primary, ON separately paired. Same frozen P95 increase <=max(20%,2ms) budget.'}
function JsonRead([string]$Path){
 for($i=0;$i-lt100;$i++){
  try{
   $raw=[IO.File]::ReadAllText($Path,[Text.Encoding]::UTF8)
   if([string]::IsNullOrWhiteSpace($raw)){throw 'Observer between writes'}
   $parsed=$raw|ConvertFrom-Json
   if($null-eq$parsed){throw 'No complete object'}
   return $parsed
  }catch{Start-Sleep -Milliseconds 20}
 }
 throw "No complete observation: $Path"
}
function Canon([string]$Path){return (($Path-replace'^\\\\\?\\','').Replace('\','/')).ToLowerInvariant()}
function Position($Owned){
 $disk=JsonRead $store
 $entry=@($disk.records.PSObject.Properties.Value|Where-Object {(Canon $_.path)-ceq(Canon $Owned.path)})|Select-Object -First 1
 if(!$entry){return $null}
 return $entry.views.PSObject.Properties['main/preview'].Value
}
function Ready($Owned){
 $deadline=[DateTime]::UtcNow.AddSeconds(40)
 do{
  try{$p=Position $Owned;if($p-and$p.fingerprint-ne0){return $p}}catch{}
  if($Owned.process.HasExited){throw 'Owned performance app exited'}
  Start-Sleep -Milliseconds 80
 }while([DateTime]::UtcNow-lt$deadline)
 throw 'No actual persistable Read viewport after native CtrlE'
}
function Active($Owned){
 $script:app=$Owned.process;$script:app.Refresh()
 Click 320 20
 if([MdcardUi]::GetForegroundWindow()-ne$script:app.MainWindowHandle){throw 'Performance input lost owned foreground'}
}
function SampleRss{
 $total=[long]0
 foreach($a in $apps){$a.process.Refresh();$total+=$a.process.WorkingSet64}
 $rss.Add(@{utc=[DateTimeOffset]::UtcNow.ToString('o');combined_app_working_set_bytes=$total})
}
function StartOwned([string]$Name,[string]$Extension){
 $path=Join-Path $work "$Size.$Extension"
 $preferences=Join-Path $output "$Name-settings.json"
 @{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|
  ConvertTo-Json|Set-Content -LiteralPath $preferences -Encoding utf8
 $launch=[Diagnostics.ProcessStartInfo]::new($record.binary);$launch.UseShellExecute=$false;$launch.WorkingDirectory=$work
 foreach($key in @($launch.Environment.Keys|Where-Object {$_-like'MDCARD_*'})){[void]$launch.Environment.Remove($key)}
 $launch.Environment['MDCARD_SETTINGS_PATH']=$preferences
 $launch.Environment['MDCARD_READING_STATE_PATH']=$store
 $launch.Environment['MDCARD_FULL_UPDATE_REPORT']=(Join-Path $output "$Name-full-update.json")
 $launch.Environment['MDCARD_RENDERER_REPORT']=(Join-Path $output "$Name-renderer.json")
 if($Diagnostics-eq'On'){
  $launch.Environment['MDCARD_READING_REPORT']=(Join-Path $output "$Name-reading.json")
  $launch.Environment['MDCARD_UI_REPORT']=(Join-Path $output "$Name-ui.json")
  $launch.Environment['MDCARD_RICH_REPORT']=(Join-Path $output "$Name-rich.json")
 }
 $renderer=Join-Path (Split-Path $record.binary -Parent) 'renderer'
 if(!(Test-Path -LiteralPath $renderer -PathType Container)){$renderer=Join-Path $root 'dist/windows/renderer'}
 $launch.Environment['MDCARD_RENDERER_DIR']=$renderer
 $launch.ArgumentList.Add($path)
 $process=[Diagnostics.Process]::Start($launch)
 $owned=@{name=$Name;path=$path;process=$process;old_layout=[IntPtr]::Zero;
  manuscript_sha256=(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()}
 $apps.Add($owned)
 $deadline=[DateTime]::UtcNow.AddSeconds(20)
 do{Start-Sleep -Milliseconds 100;$process.Refresh()}while(!$process.HasExited-and$process.MainWindowHandle-eq0-and[DateTime]::UtcNow-lt$deadline)
 if($process.HasExited-or$process.MainWindowHandle-eq0){throw 'No native performance window'}
 Start-Sleep -Milliseconds 1800
 [void][MdcardUi]::MoveWindow($process.MainWindowHandle,50,50,1240,820,$true)
 Active $owned
 [uint32]$pidCheck=0;$thread=[PrecisionPerformanceInput]::GetWindowThreadProcessId($process.MainWindowHandle,[ref]$pidCheck)
 if($pidCheck-ne$process.Id){throw 'Wrong input-layout owner'}
 $owned.old_layout=[PrecisionPerformanceInput]::GetKeyboardLayout($thread)
 [void][PrecisionPerformanceInput]::PostMessage($process.MainWindowHandle,0x50,[IntPtr]::Zero,[PrecisionPerformanceInput]::LoadKeyboardLayout('00000409',0))
 Start-Sleep -Milliseconds 250;Keys '^e';[void](Ready $owned)
 $owned.dpi=[MdcardUi]::GetDpiForWindow($process.MainWindowHandle)
 Shot "$Name-before";SampleRss
 $capture=[Drawing.Image]::FromFile((Join-Path $output "$Name-before.png"))
 try{$owned.window_capture_pixels=@($capture.Width,$capture.Height)}finally{$capture.Dispose()}
 return $owned
}
function NativeWheel($Owned,[int]$Notches){
 Active $Owned
 $before=Position $Owned;$bounds=Bounds
 [void][MdcardUi]::SetCursorPos($bounds.Left+900,$bounds.Top+400)
 $begin=[Diagnostics.Stopwatch]::GetTimestamp()
 $nativeTimes=[Collections.Generic.List[long]]::new()
 $delta=if($Notches-gt0){[uint32]::MaxValue-119}else{[uint32]120}
 for($i=0;$i-lt[Math]::Abs($Notches);$i++){
  if([MdcardUi]::GetForegroundWindow()-ne$Owned.process.MainWindowHandle){throw 'Wheel target no longer owns foreground'}
  $nativeTimes.Add([Diagnostics.Stopwatch]::GetTimestamp())
  [MdcardUi]::mouse_event(0x800,0,0,$delta,[UIntPtr]::Zero)
  SampleRss;Start-Sleep -Milliseconds 65
 }
 $end=[Diagnostics.Stopwatch]::GetTimestamp();Start-Sleep -Milliseconds 1000
 $after=Position $Owned
 if(!$after-or$after.offset[1]-le100){throw 'Native input did not produce a persisted nontrivial viewport'}
 if(($Notches-gt0-and$after.offset[1]-le$before.offset[1])-or($Notches-lt0-and$after.offset[1]-ge$before.offset[1])){throw 'Persisted viewport did not move in the native wheel direction'}
 $inputs.Add(@{app=$Owned.name;notches=$Notches;delay_ms=65;before=$before;after=$after;
  native_begin_ticks=$begin;native_end_ticks=$end;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;
  actual_injection_ticks=$nativeTimes.ToArray();native_input_wall_seconds=($end-$begin)/[double][Diagnostics.Stopwatch]::Frequency})
 Shot "$($Owned.name)-after-$($inputs.Count)"
}
try{
 $a=StartOwned 'a' 'md';$b=StartOwned 'b' 'txt'
 NativeWheel $a 100;NativeWheel $b 55;NativeWheel $a -25
 foreach($owned in $apps){
  if((Get-FileHash -LiteralPath $owned.path).Hash.ToLowerInvariant()-cne$owned.manuscript_sha256){throw 'Performance test changed original manuscript bytes'}
  $probe=JsonRead (Join-Path $output "$($owned.name)-full-update.json")
  if($probe.queue_dropped_samples-ne0){throw 'Observer dropped samples'}
  if(@($probe.samples|Where-Object wheel_input -eq $true).Count-lt30){throw 'Insufficient actual native wheel update samples'}
 }
 $record.inputs=$inputs.ToArray();$record.apps=@($apps|ForEach-Object {@{name=$_.name;path=$_.path;dpi=$_.dpi;window_capture_pixels=$_.window_capture_pixels;manuscript_sha256=$_.manuscript_sha256}})
 $record.rss_samples=$rss.ToArray();$record.peak_observed_combined_app_working_set_bytes=($rss|Measure-Object -Property combined_app_working_set_bytes -Maximum).Maximum
 $record.rss_boundary='Sampled application WorkingSet64 around real input, excludes renderer descendants, not private allocator peak. Actual input intervals retained; sampling overhead is outside product App::update.'
 foreach($owned in $apps){Active $owned;Keys '^q';if(!$owned.process.WaitForExit(8000)-or$owned.process.ExitCode-ne0){throw 'Performance app did not close normally'}}
 $record.final_observer_integrity=@{}
 foreach($owned in $apps){
  $probePath=Join-Path $output "$($owned.name)-full-update.json"
  $probe=JsonRead $probePath
  if($probe.flush_complete-ne$true-or$probe.submitted_samples-ne$probe.processed_samples-or$probe.processed_samples-ne$probe.total_samples-or$probe.total_samples-ne@($probe.samples).Count-or$probe.queue_dropped_samples-ne0-or$probe.retention_dropped_samples-ne0){throw 'Final observer publication is incomplete'}
  $record.final_observer_integrity[$owned.name]=@{flush_complete=$true;submitted_samples=$probe.submitted_samples;processed_samples=$probe.processed_samples;total_samples=$probe.total_samples;raw_sha256=(Get-FileHash -LiteralPath $probePath).Hash.ToLowerInvariant()}
 }
 $record.passed=$true
}catch{
 $record.passed=$false;$record.error=$_.Exception.Message;$record.error_stack=$_.ScriptStackTrace
 try{if($script:app-and!$script:app.HasExited){Shot 'failure'}}catch{}
}finally{
 foreach($owned in $apps){
  if(!$owned.process.HasExited){
   if($owned.old_layout-ne[IntPtr]::Zero){[void][PrecisionPerformanceInput]::PostMessage($owned.process.MainWindowHandle,0x50,[IntPtr]::Zero,$owned.old_layout)}
   $owned.process.Kill();$owned.process.WaitForExit()
  }
 }
 $record.finished=[DateTimeOffset]::UtcNow.ToString('o')
 $record|ConvertTo-Json -Depth 32|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
}
$record|Select-Object stage,size,diagnostics,passed,error,peak_observed_combined_app_working_set_bytes|ConvertTo-Json
if(!$record.passed){throw 'Native Windows full-update cohort failed'}
