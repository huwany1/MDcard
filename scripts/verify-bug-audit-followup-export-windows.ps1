# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe,[Parameter(Mandatory=$true)][string]$Renderer,[Parameter(Mandatory=$true)][string]$OutputDir)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$output=[IO.Path]::GetFullPath($OutputDir)
New-Item -ItemType Directory -Force $output|Out-Null
Remove-Item -LiteralPath (Join-Path $output 'result.json') -ErrorAction SilentlyContinue
$runDir=Join-Path $output ([guid]::NewGuid().ToString())
New-Item -ItemType Directory $runDir|Out-Null
$settings=Join-Path $runDir 'settings.json';$report=Join-Path $runDir 'state.json'
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
[IO.File]::WriteAllText((Join-Path $runDir 'export-settings.json'),'{"format":0}',[Text.UTF8Encoding]::new($false))
$oldRenderer=$env:MDCARD_RENDERER_DIR;$oldReport=$env:MDCARD_EXPORT_REPORT
$env:MDCARD_RENDERER_DIR=[IO.Path]::GetFullPath($Renderer);$env:MDCARD_EXPORT_REPORT=$report
$utf8=[Text.UTF8Encoding]::new($false)
$clipboard=$null;try{$clipboard=[Windows.Forms.Clipboard]::GetDataObject()}catch{}
Add-Type @'
using System; using System.Runtime.InteropServices;
public class AuditNativeKeys {
 [DllImport("user32.dll")] public static extern void keybd_event(byte key,byte scan,uint flags,UIntPtr extra);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint pid);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h,System.Text.StringBuilder text,int count);
 private delegate bool EnumCallback(IntPtr h,IntPtr extra);
 [DllImport("user32.dll")] private static extern bool EnumWindows(EnumCallback callback,IntPtr extra);
 public static IntPtr SaveDialog(uint pid){
  IntPtr found=IntPtr.Zero;
  EnumWindows((h,extra)=>{uint owner;GetWindowThreadProcessId(h,out owner);if(owner==pid){var text=new System.Text.StringBuilder(512);GetWindowText(h,text,512);if(text.ToString()=="另存为"||text.ToString()=="Save As"){found=h;return false;}}return true;},IntPtr.Zero);
  return found;
 }
 public static IntPtr ConfirmDialog(uint pid){
  IntPtr found=IntPtr.Zero;
  EnumWindows((h,extra)=>{uint owner;GetWindowThreadProcessId(h,out owner);if(owner==pid){var text=new System.Text.StringBuilder(512);GetWindowText(h,text,512);if(text.ToString().Contains("确认另存为")||text.ToString().Contains("Confirm Save As")){found=h;return false;}}return true;},IntPtr.Zero);
  return found;
 }
}
'@
function NativeChord([byte[]]$Codes){
 foreach($code in $Codes){[AuditNativeKeys]::keybd_event($code,0,0,[UIntPtr]::Zero)}
 for($i=$Codes.Count-1;$i-ge0;$i--){[AuditNativeKeys]::keybd_event($Codes[$i],0,2,[UIntPtr]::Zero)}
 Start-Sleep -Milliseconds 200
}
function FocusTest([IntPtr]$Handle=[IntPtr]::Zero) {
 if($Handle-eq[IntPtr]::Zero){$Handle=$script:app.MainWindowHandle}
 if($handle-eq0){throw 'No native test window'}
 [void][MdcardUi]::SetWindowPos($handle,[IntPtr]::new(-1),0,0,0,0,0x43)
 [void][MdcardUi]::SetForegroundWindow($handle);Start-Sleep -Milliseconds 200
 if([MdcardUi]::GetForegroundWindow()-ne$handle){throw 'Test window does not own foreground; input refused'}
 # Keep the bounded test window visible while native dialog input is running.
}
function DialogShot([IntPtr]$Handle,[string]$Name){
 $b=[MdcardUi+Rect]::new();[void][MdcardUi]::GetWindowRect($Handle,[ref]$b)
 $bitmap=[Drawing.Bitmap]::new($b.Right-$b.Left,$b.Bottom-$b.Top);$graphics=[Drawing.Graphics]::FromImage($bitmap)
 $graphics.CopyFromScreen($b.Left,$b.Top,0,0,$bitmap.Size);$bitmap.Save((Join-Path $output "$Name.png"));$graphics.Dispose();$bitmap.Dispose()
}
function NativeButton([IntPtr]$Handle,[string]$AutomationId){
 FocusTest $Handle
 $window=[System.Windows.Automation.AutomationElement]::FromHandle($Handle)
 $button=$window.FindFirst([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::AutomationIdProperty,$AutomationId))
 if(!$button-and$AutomationId-eq'6'){$button=@($window.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition)|Where-Object {$_.Current.ControlType-eq[System.Windows.Automation.ControlType]::Button-and$_.Current.Name-match'^是|^Yes|^替换|^Replace'})|Select-Object -First 1}
 if(!$button){throw "Native button $AutomationId missing"}
 $bounds=$button.Current.BoundingRectangle
 [void][MdcardUi]::SetCursorPos([int]($bounds.X+$bounds.Width/2),[int]($bounds.Y+$bounds.Height/2))
 [MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero);[MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
 Start-Sleep -Milliseconds 300
}
function Python([string[]]$Arguments){
 $psi=[Diagnostics.ProcessStartInfo]::new('python');$psi.UseShellExecute=$false;$psi.CreateNoWindow=$true
 foreach($argument in $Arguments){$psi.ArgumentList.Add($argument)}
 return [Diagnostics.Process]::Start($psi)
}
function State {try{Get-Content -LiteralPath $report -Raw -Encoding utf8|ConvertFrom-Json}catch{$null}}
function Await([scriptblock]$Condition,[string]$Message,[int]$Seconds=25){
 $until=[DateTime]::UtcNow.AddSeconds($Seconds)
 while([DateTime]::UtcNow-lt$until){$s=State;if($s-and(&$Condition $s)){return $s};if($script:app.HasExited){throw "App exited: $Message"};Start-Sleep -Milliseconds 30}
 Shot 'failure';throw "$Message : $(State|ConvertTo-Json -Depth 6 -Compress)"
}
function Control([string]$Name){FocusTest;Await {param($s)$null-ne$s.controls.$Name} "Control $Name"|Out-Null;Start-Sleep -Milliseconds 200;$s=Await {param($s)$null-ne$s.controls.$Name} "Settled control $Name";$r=$s.controls.$Name;[IO.File]::AppendAllText((Join-Path $runDir 'input.log'),"click $Name $($r|ConvertTo-Json -Compress) bounds=$((Bounds)|ConvertTo-Json -Compress) dpi=$([MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle))`n");if($Name-eq'export'){Shot 'before-export'};Click ($r.x+$r.w/2) ($r.y+$r.h/2)}
function Owned {
 if($script:ownedHandles){foreach($handle in $script:ownedHandles.Values){$handle.Dispose()}}
 $script:ownedHandles=@{}
 $all=@(Get-CimInstance Win32_Process);$ids=[Collections.Generic.HashSet[int]]::new();[void]$ids.Add($script:app.Id)
 $byId=@{};foreach($p in $all){$byId[[int]$p.ProcessId]=$p}
 # Windows retains a dead parent's PID. Reject edges to a newer reused PID.
 do{$before=$ids.Count;foreach($p in $all){$parent=$byId[[int]$p.ParentProcessId];if($ids.Contains([int]$p.ParentProcessId)-and$parent-and$p.CreationDate-ge$parent.CreationDate){[void]$ids.Add([int]$p.ProcessId)}}}while($ids.Count-ne$before)
 return @($all|Where-Object {$ids.Contains([int]$_.ProcessId)-and$_.ProcessId-ne$script:app.Id}|ForEach-Object {
  $captured=$_
  try{
   $native=[Diagnostics.Process]::GetProcessById([int]$captured.ProcessId);[void]$native.Handle
   if($native.StartTime.ToUniversalTime().ToString('yyyyMMddHHmmssffffff')-eq$captured.CreationDate.ToUniversalTime().ToString('yyyyMMddHHmmssffffff')){$script:ownedHandles[[int]$captured.ProcessId]=$native}else{$native.Dispose();throw 'PID identity changed during observation'}
  }catch{if(Get-Process -Id $captured.ProcessId -ErrorAction SilentlyContinue){throw}}
  [pscustomobject]@{pid=$captured.ProcessId;parent=$captured.ParentProcessId;created=$captured.CreationDate.ToUniversalTime().ToString('o');command=$captured.CommandLine}
 })
}
function AssertGone($Owned){foreach($p in $Owned){
 $native=$script:ownedHandles[[int]$p.pid]
 $exited=(!$native)-or$native.HasExited
 $now=Get-CimInstance Win32_Process -Filter "ProcessId=$($p.pid)"
 $observation=@{pid=$p.pid;created=$p.created;native_handle_captured=($null-ne$native);kernel_has_exited=$exited;cim_object_still_present=($null-ne$now);observed_at=[DateTimeOffset]::UtcNow.ToString('o')}
 [IO.File]::AppendAllText((Join-Path $output 'process-observations.jsonl'),($observation|ConvertTo-Json -Compress)+"`n")
 if(!$exited){throw "Owned converter has not exited at completion: $($p|ConvertTo-Json -Compress)"}
}}
function ChooseTarget {
 [IO.File]::AppendAllText((Join-Path $runDir 'input.log'),"choose export`n")
 Control 'export'
 $until=[DateTime]::UtcNow.AddSeconds(10)
 while([AuditNativeKeys]::SaveDialog($script:app.Id)-eq[IntPtr]::Zero){
  if([DateTime]::UtcNow-gt$until){throw 'Native Save dialog did not open'};Start-Sleep -Milliseconds 50
 }
 [IO.File]::AppendAllText((Join-Path $runDir 'input.log'),"dialog opened`n")
 FocusTest ([AuditNativeKeys]::SaveDialog($script:app.Id));NativeChord @(18,78);NativeChord @(17,65);Set-Clipboard -Value $destination;NativeChord @(17,86)
 [IO.File]::AppendAllText((Join-Path $runDir 'input.log'),"filename pasted`n")
 DialogShot ([AuditNativeKeys]::SaveDialog($script:app.Id)) 'chooser-pasted'
 NativeButton ([AuditNativeKeys]::SaveDialog($script:app.Id)) '1'
 $confirm=[AuditNativeKeys]::ConfirmDialog($script:app.Id)
 if($confirm-ne[IntPtr]::Zero){DialogShot $confirm 'chooser-confirm';FocusTest $confirm;NativeChord @(37);NativeChord @(13)}
 if([AuditNativeKeys]::SaveDialog($script:app.Id)-ne[IntPtr]::Zero){DialogShot ([AuditNativeKeys]::SaveDialog($script:app.Id)) 'chooser-after-save'}
 [IO.File]::AppendAllText((Join-Path $runDir 'input.log'),"save sent`n")
}
function StartTask {
 Remove-Item -LiteralPath (Join-Path $runDir 'requests.log') -ErrorAction SilentlyContinue
 ChooseTarget
 Await {param($s)$s.running-and(Test-Path -LiteralPath (Join-Path $runDir 'requests.log'))} 'Live resource converter' 60|Out-Null
 if((State).output-ne$destination){throw "Wrong chosen target: $((State).output)"}
 $owned=Owned
 if(!@($owned|Where-Object command -match 'export.mjs').Count-or!@($owned|Where-Object command -match 'chrome').Count){throw 'Converter tree missing'}
 return $owned
}
function AssertSafe {
 if((Get-FileHash -LiteralPath $source).Hash-ne$sourceHash){throw 'Source changed'}
 if([IO.File]::ReadAllText($destination)-cne'OLD OUTPUT'){throw 'Old target changed'}
 if(@(Get-ChildItem -LiteralPath $runDir -Directory -Filter '.mdcard-task-*').Count){throw 'Task workspace leaked'}
 if($unrelated.HasExited){throw 'Unrelated process terminated'}
}
$server=$null;$unrelated=$null
try{
 $server=Python @((Join-Path $PSScriptRoot 'serve-bug-audit-resource.py'),$runDir)
 $until=[DateTime]::UtcNow.AddSeconds(10);while(!(Test-Path -LiteralPath (Join-Path $runDir 'ready.json'))){if([DateTime]::UtcNow-gt$until){throw 'Resource server failed'};Start-Sleep -Milliseconds 50}
 $port=(Get-Content -Raw (Join-Path $runDir 'ready.json')|ConvertFrom-Json).port
 $source=Join-Path $runDir '稿件 中文.md';$destination=Join-Path $runDir '旧目标.html'
 [IO.File]::WriteAllText($source,"# Cancellation / 取消`n`n![controlled](http://127.0.0.1:$port/slow.png)`n",$utf8)
 [IO.File]::WriteAllText($destination,'OLD OUTPUT',$utf8);$sourceHash=(Get-FileHash -LiteralPath $source).Hash
 $unrelated=Python @('-c','import time; time.sleep(180)')
 Launch 'native' '' $source;Keys '^%e';Await {param($s)$s.open} 'Export dialog'|Out-Null
 $samples=@()
 for($n=0;$n-lt3;$n++){
  $owned=StartTask;Shot "cancel-$n-running";$clock=[Diagnostics.Stopwatch]::StartNew();Control 'cancel'
  $final=Await {param($s)!$s.running-and$s.last.type-eq'cancelled'} 'Cancellation finished'
  AssertGone $owned;AssertSafe
  $samples+=@{action='cancel';elapsed_ms=$clock.Elapsed.TotalMilliseconds;before=$owned;state=$final}
 }
 [IO.File]::WriteAllText((Join-Path $runDir 'release'),'go',$utf8);ChooseTarget
 $completed=Await {param($s)!$s.running-and$s.last.type-eq'complete'} 'Retry complete' 60
 if((Get-FileHash -LiteralPath $source).Hash-ne$sourceHash-or[IO.File]::ReadAllText($destination)-notmatch'data:image/png;base64,'){throw 'Retry source or output invalid'}
 Shot 'retry-complete';Keys '^q';if(!$script:app.WaitForExit(10000)-or$script:app.ExitCode-ne0){throw 'Retry close failed'}
 Remove-Item -LiteralPath (Join-Path $runDir 'release');[IO.File]::WriteAllText($destination,'OLD OUTPUT',$utf8)
 Launch 'native-close' '' $source;Keys '^%e';Await {param($s)$s.open} 'Reopened export dialog'|Out-Null
 $owned=StartTask;$clock=[Diagnostics.Stopwatch]::StartNew();[void]$script:app.CloseMainWindow()
 if(!$script:app.WaitForExit(15000)-or$script:app.ExitCode-ne0){throw 'Native close failed'}
 AssertGone $owned;AssertSafe;$samples+=@{action='native-close';elapsed_ms=$clock.Elapsed.TotalMilliseconds;before=$owned}
 @{verification_id=[Guid]::NewGuid().ToString();date=[DateTimeOffset]::UtcNow.ToString('o');exe_sha256=(Get-FileHash -LiteralPath $Exe).Hash;renderer=$Renderer;platform='Windows11 native';samples=$samples;retry=$completed;source_preserved=$true;old_target_preserved=$true;unrelated_process_preserved=$true;no_live_owned_descendants_at_completion=$true;task_directories_removed=$true}|ConvertTo-Json -Depth 12|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
 Write-Output "Passed: $output"
}finally{
 if($script:app-and!$script:app.HasExited){[void]$script:app.CloseMainWindow();if(!$script:app.WaitForExit(15000)){$script:app.Kill();$script:app.WaitForExit()}}
 foreach($p in @($server,$unrelated)){if($p-and!$p.HasExited){$p.Kill();$p.WaitForExit()}}
 if($script:ownedHandles){foreach($handle in $script:ownedHandles.Values){$handle.Dispose()}}
 if($null-ne$clipboard){[Windows.Forms.Clipboard]::SetDataObject($clipboard,$true)}
 $env:MDCARD_RENDERER_DIR=$oldRenderer;$env:MDCARD_EXPORT_REPORT=$oldReport
}
