# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$Exe="$PSScriptRoot\..\artifacts\performance\windows\MDcard.exe",[switch]$Media,[string]$EvidenceName='')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class MdcardLargeInput {
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
 [DllImport("user32.dll")] public static extern IntPtr GetKeyboardLayout(uint thread);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr LoadKeyboardLayout(string name, uint flags);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr window, uint message, IntPtr wparam, IntPtr lparam);
 [StructLayout(LayoutKind.Sequential)] public struct KeyboardInput {public ushort vk, scan; public uint flags, time; public UIntPtr extra;}
 [StructLayout(LayoutKind.Explicit)] public struct InputUnion {
  [FieldOffset(0)] public KeyboardInput keyboard;
  [FieldOffset(0)] public MouseInput mouse;
 }
 [StructLayout(LayoutKind.Sequential)] public struct MouseInput {public int x,y; public uint data,flags,time; public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)] public struct Input {public uint type; public InputUnion data;}
 [DllImport("user32.dll",SetLastError=true)] public static extern uint SendInput(uint count, Input[] inputs, int size);
 public static void TypeUnicode(char value) {
  Input down=new Input {type=1, data=new InputUnion {keyboard=new KeyboardInput {scan=value,flags=4}}};
  Input up=down; up.data.keyboard.flags=6;
  if(SendInput(2,new Input[]{down,up},Marshal.SizeOf(typeof(Input)))!=2)
   throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
 }
}
'@
$output=Join-Path $root $(if($Media){'evidence\performance\master-review\windows-media'}else{'evidence\performance\master-review\windows'})
if($EvidenceName){$output=Join-Path $root "evidence\ui-polish-2026-10-22\reports\$EvidenceName"}
New-Item -ItemType Directory -Force -Path $output|Out-Null
$settings=Join-Path $output 'settings.json'
$report=Join-Path $output 'state.json'
$fixture=Join-Path $output 'long.md'
$utf8=[Text.UTF8Encoding]::new($false)
$text=[Text.StringBuilder]::new()
for($i=0;$i-lt3000;$i++){
 [void]$text.Append("# Heading $($i.ToString('00000'))`n`n中文原生渲染性能与精确位置检查 source-$($i.ToString('00000'))`n`n")
 if($Media-and$i%12-eq0-and$i-lt2640){
  $nodeIndex=[int]($i/12)
  switch($nodeIndex%4){
   0 {[void]$text.Append('$$'+"`nx_{$nodeIndex}^2 + \frac{1}{2}`n"+'$$'+"`n`n")}
   1 {[void]$text.Append('```mermaid'+"`nflowchart LR`nA$nodeIndex[Source $nodeIndex] --> B$nodeIndex[Preview]`n"+'```'+"`n`n")}
   2 {[void]$text.Append('```plantuml'+"`n@startuml`nAlice -> Bob: Message $nodeIndex`n@enduml`n"+'```'+"`n`n")}
   3 {[void]$text.Append('```flow'+"`nst=>start: Start $nodeIndex`ne=>end: End`nst->e`n"+'```'+"`n`n")}
  }
 }
}
$original=$text.ToString()
[IO.File]::WriteAllText($fixture,$original,$utf8)
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$envNames=@('MDCARD_PERFORMANCE_REPORT','MDCARD_PERFORMANCE_GPU_FINISH','MDCARD_PERFORMANCE_CONTINUOUS','MDCARD_RENDERER_DIR')
$oldEnv=@{}
foreach($name in $envNames){$oldEnv[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
$priorClipboard=$null
try{$priorClipboard=[Windows.Forms.Clipboard]::GetDataObject()}catch{}
$verification=[Guid]::NewGuid().ToString()
$snapshot=Join-Path $root "artifacts\performance\windows\master-$verification.exe"
New-Item -ItemType Directory -Force -Path (Split-Path $snapshot)|Out-Null
Copy-Item -LiteralPath (Resolve-Path -LiteralPath $Exe).Path -Destination $snapshot
$Exe=$snapshot
$binaryHash=(Get-FileHash -LiteralPath $snapshot -Algorithm SHA256).Hash
$env:MDCARD_PERFORMANCE_REPORT=$report
$env:MDCARD_PERFORMANCE_GPU_FINISH='1'
$env:MDCARD_PERFORMANCE_CONTINUOUS='1'
$env:MDCARD_RENDERER_DIR=Join-Path $root 'renderer'
function State {try{Get-Content -Raw -LiteralPath $report -Encoding utf8|ConvertFrom-Json}catch{$null}}
function WaitState([scriptblock]$Condition,[string]$Label,[int]$Seconds=12){
 $until=[DateTime]::UtcNow.AddSeconds($Seconds)
 while([DateTime]::UtcNow-lt$until){$s=State;if($s-and(&$Condition $s)){return $s};if($script:app.HasExited){throw "App exited: $Label"};Start-Sleep -Milliseconds 10}
 throw "Timeout: $Label"
}
function Foreground {
 if([MdcardUi]::GetForegroundWindow()-ne$script:app.MainWindowHandle){throw 'Native test window lost foreground ownership'}
}
function Literal([char]$Character){
 switch([string]$Character){
  '{' {'{{}'} '}' {'{}}'} '+' {'{+}'} '^' {'{^}'} '%' {'{%}'} '~' {'{~}'} '(' {'{(}'} ')' {'{)}'}
  default {[string]$Character}
 }
}
function ExactSave([string]$Expected){
 Foreground;Keys '^s'
 $until=[DateTime]::UtcNow.AddSeconds(5)
 while([DateTime]::UtcNow-lt$until){if([IO.File]::ReadAllText($fixture,$utf8)-ceq$Expected){return};Start-Sleep -Milliseconds 20}
 throw 'Native source did not save the exact expected Unicode text'
}
function Stats($Values){
 $v=@($Values|ForEach-Object {[double]$_}|Sort-Object)
 if(!$v.Count){return @{samples=0}}
 return @{samples=$v.Count;p50_ms=$v[[Math]::Min($v.Count-1,[int][Math]::Floor($v.Count*.5))];p95_ms=$v[[Math]::Min($v.Count-1,[int][Math]::Floor($v.Count*.95))];p99_ms=$v[[Math]::Min($v.Count-1,[int][Math]::Floor($v.Count*.99))];max_ms=$v[-1];over_16_667=@($v|Where-Object {$_-gt16.667}).Count}
}
$result=@{verification_id=$verification;binary_sha256=$binaryHash;platform='Windows native';characters=$original.Length;lines=($original.Split("`n").Length-1);media_nodes=$(if($Media){220}else{0});passed=$false}
$originalInputLayout=[IntPtr]::Zero
try{
 Remove-Item -LiteralPath $report -ErrorAction SilentlyContinue
 Launch 'renderer' '' $fixture
 $processIdValue=[uint32]0
 $inputThread=[MdcardLargeInput]::GetWindowThreadProcessId($script:app.MainWindowHandle,[ref]$processIdValue)
 $originalInputLayout=[MdcardLargeInput]::GetKeyboardLayout($inputThread)
 $usLayout=[MdcardLargeInput]::LoadKeyboardLayout('00000409',0)
 if($usLayout-eq[IntPtr]::Zero){throw 'US input layout is unavailable for deterministic native typing'}
 [void][MdcardLargeInput]::PostMessage($script:app.MainWindowHandle,0x0050,[IntPtr]::Zero,$usLayout)
 Start-Sleep -Milliseconds 200
 if(([MdcardLargeInput]::GetKeyboardLayout($inputThread).ToInt64()-band0xffff)-ne0x0409){throw 'Test process did not adopt US keyboard layout'}
 $ready=WaitState {param($s)$s.active-and$s.source_rect-and$s.paint.frames-gt2} 'Native GPU completion'
 $r=$ready.source_rect;Click ($r[0]+8) ($r[1]+5);Foreground;Keys '^{HOME}'
 $typed='abcdefghijklmnopqrstuvwxyz0123456789.,;:!?()[]{}/*_-+#='
 $injections=[Collections.Generic.List[object]]::new()
 foreach($character in $typed.ToCharArray()){
  Foreground
  $beforeInput=State
  $targetRevision=[long]$beforeInput.revision+1
  $injectedAt=[Diagnostics.Stopwatch]::GetTimestamp()
  [MdcardLargeInput]::TypeUnicode($character)
  $completed=WaitState {param($s) $s.revision-ge$targetRevision-and@($s.paint.input_completions|Where-Object {$_.revision-eq$targetRevision}).Count-gt0} 'Injected key GPU completion'
  $completion=@($completed.paint.input_completions|Where-Object {$_.revision-eq$targetRevision})[0]
  if([long]$completed.paint.qpc_frequency-ne[Diagnostics.Stopwatch]::Frequency){throw 'QPC frequency mismatch'}
  $latency=1000.0*([long]$completion.qpc-$injectedAt)/[Diagnostics.Stopwatch]::Frequency
  if($latency-lt0){throw 'GPU completion predates injected key'}
  $injections.Add(@{character=[string]$character;revision=$targetRevision;injected_qpc=$injectedAt;complete_qpc=[long]$completion.qpc;milliseconds=$latency})
 }
 ExactSave ($typed+$original)
 $typing=WaitState {param($s)$s.parse_revision-eq$s.revision} 'Typing parser catches up'
 $typing|ConvertTo-Json -Depth 10|Set-Content -LiteralPath (Join-Path $output 'typing-state.json') -Encoding utf8
 $result.typing_parse=Stats $typing.incremental_parse_ms
 $result.typing_update_start_to_finish=Stats $typing.input_event_to_update_finish_ms
 $result.typing_update_start_to_gpu_complete=Stats $typing.paint.input_to_complete_ms
 $result.typing_injection_to_gpu_complete=Stats @($injections|ForEach-Object {$_.milliseconds})
 $result.injection_samples=$injections.ToArray()
 $result.disk_open_to_gpu_complete_ms=$typing.paint.disk_open_to_first_complete_ms
 $result.exact_typing_saved=$true
 Foreground;Keys '^{END}'
 $paste=("`n# Large paste`n"+('中文大粘贴切片测试 '*3500)+"`n")
 [Windows.Forms.Clipboard]::SetText($paste)
 Keys '^v';ExactSave ($typed+$original+$paste)
 $result.exact_large_paste_saved=$true
 $pasteState=WaitState {param($s)$s.parse_revision-eq$s.revision} 'Large paste background parser catches up'
 $result.input_frames_including_paste_update_start_to_gpu_complete=Stats $pasteState.paint.input_to_complete_ms
 Foreground;Keys '^{HOME}'
 $start=WaitState {param($s)$s.source_scrollbar-and$s.source_offset-lt1} 'Real source scrollbar geometry'
 $track=$start.source_scrollbar
 $bounds=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0
 $px=$bounds.Left+[int](($track[0]+$track[2])*.5*$scale)
 $top=$bounds.Top+[int](($track[1]+4)*$scale)
 $bottom=$bounds.Top+[int](($track[3]-4)*$scale)
 [void][MdcardUi]::SetCursorPos($px,$top)
 [MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
 $beforeFrames=[long]$start.paint.frames;$beforeIntervals=@($start.paint.intervals_ms).Count
 $offsets=[Collections.Generic.List[double]]::new()
 $scrollWatch=[Diagnostics.Stopwatch]::StartNew()
 for($i=0;$i-lt480;$i++){
  Foreground
  $fraction=.5-.49*[Math]::Cos(2*[Math]::PI*$i/160)
  $py=[int]($top+($bottom-$top)*$fraction)
  [void][MdcardUi]::SetCursorPos($px,$py)
  if($i%16-eq0){$s=State;if($s){$offsets.Add([double]$s.source_offset)}}
  Start-Sleep -Milliseconds 6
 }
 [MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
 $scrollWatch.Stop()
 $end=State
 $intervals=@($end.paint.intervals_ms)
 $newCount=[long]$end.paint.frames-$beforeFrames
 $take=[Math]::Min($intervals.Count,[int]$newCount)
 $recent=if($take-gt0){@($intervals[($intervals.Count-$take)..($intervals.Count-1)])}else{@()}
 $result.scroll=Stats $recent
 $result.scroll.frames=$newCount
 $result.scroll.elapsed_ms=$scrollWatch.Elapsed.TotalMilliseconds
 $result.scroll.mean_completion_fps=if($recent.Count){1000.0/($recent|Measure-Object -Average).Average}else{0.0}
 $result.scroll.wall_count_fps=1000.0*$newCount/$scrollWatch.Elapsed.TotalMilliseconds
 $result.scroll.min_offset=($offsets|Measure-Object -Minimum).Minimum
 $result.scroll.max_offset=($offsets|Measure-Object -Maximum).Maximum
 $result.scroll.source_height=$end.source_height
 $result.scroll.real_scrollbar_drag=$true
 if(!$offsets.Count-or$result.scroll.max_offset-lt$end.source_height*.8){throw 'Mouse drag did not traverse most of the actual source document'}
 $end|ConvertTo-Json -Depth 12|Set-Content -LiteralPath (Join-Path $output 'scroll-state.json') -Encoding utf8
 Shot 'native-scrollbar-drag'
 $result.renderer=$end.renderer
 $result.cache=$end.cache
 $result.gpu_finish=$end.paint.gpu_finish
 $result.note='Cross-process QPC measures real SendInput injection through native event dispatch to GPU completion per document revision. Update-start metrics are separate. GPU-complete render cadence uses the real native scrollbar; display scanout and ETW present are not measured. No slow frame samples are removed.'
 $result.functional_passed=$true
 $result.thresholds=@{tti_under_200_ms=($result.disk_open_to_gpu_complete_ms-lt200);ordinary_key_parse_under_5_ms=($result.typing_parse.samples-ge$typed.Length-and$result.typing_parse.max_ms-lt5);injection_to_gpu_complete_under_16_ms=($result.typing_injection_to_gpu_complete.samples-eq$typed.Length-and$result.typing_injection_to_gpu_complete.max_ms-lt16);scroll_all_frames_under_16_667_ms=($result.scroll.max_ms-le16.667);scroll_completion_fps_at_least_60=($result.scroll.mean_completion_fps-ge60)}
 $result.thresholds.input_frames_including_paste_within_16_ms=$result.input_frames_including_paste_update_start_to_gpu_complete.max_ms-lt16
 if($Media){$result.thresholds.real_local_media_success=($end.cache.completed-ge5-and$end.cache.errors-eq0)}
 $result.passed=@($result.thresholds.Values|Where-Object {!$_}).Count-eq0
 [void][MdcardLargeInput]::PostMessage($script:app.MainWindowHandle,0x0050,[IntPtr]::Zero,$originalInputLayout)
 Start-Sleep -Milliseconds 100
 Foreground;Keys '^q';if(!$script:app.WaitForExit(5000)){throw 'Native editor failed to close'}
}catch{
 $result.error=$_.Exception.Message
 throw
}finally{
 [MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
 if($script:app-and!$script:app.HasExited){
  if($originalInputLayout-ne[IntPtr]::Zero){[void][MdcardLargeInput]::PostMessage($script:app.MainWindowHandle,0x0050,[IntPtr]::Zero,$originalInputLayout);Start-Sleep -Milliseconds 100}
  $script:app.Kill($true);$script:app.WaitForExit()
 }
 $result|ConvertTo-Json -Depth 12|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
 foreach($name in $envNames){[Environment]::SetEnvironmentVariable($name,$oldEnv[$name],'Process')}
 if($priorClipboard){[Windows.Forms.Clipboard]::SetDataObject($priorClipboard,$true)}
 if(Test-Path -LiteralPath $snapshot){Remove-Item -LiteralPath $snapshot}
}
$result|ConvertTo-Json -Depth 8
if(!$result.passed){throw 'One or more requested performance gates failed; see result.json'}
