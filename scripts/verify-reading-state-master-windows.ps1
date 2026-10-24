# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
 [Parameter(Mandatory=$true)][string]$Exe,
 [string]$Stage='master-r1',
 [switch]$OnlyReadingHelpers,
 [switch]$WithoutSearchReport
)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\workbench-clipboard.ps1"
. "$PSScriptRoot\verify-search-master-windows.ps1" -Exe $Exe -LoadFunctionsOnly
. "$PSScriptRoot\workbench-native-input.ps1"
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class ReadingInputNative {
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern IntPtr GetKeyboardLayout(uint t);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr LoadKeyboardLayout(string n,uint f);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
}
'@
if($Stage-notmatch'^[A-Za-z0-9_-]+$'){throw 'Use a simple stage name'}
$output=Join-Path $root "evidence\reading-state\master\$Stage"
if(Test-Path -LiteralPath (Join-Path $output 'result.json')){throw 'Use a fresh Stage to retain previous evidence'}
$scratch=Join-Path $root "artifacts\reading-state\$Stage"
$fixtures=Join-Path $scratch 'workspace'
New-Item -ItemType Directory -Force $output,$scratch|Out-Null
python "$PSScriptRoot\generate-reading-state-master-fixtures.py" --output $fixtures
if($LASTEXITCODE-ne0){throw 'Fixture generation failed'}
$settings=Join-Path $scratch 'settings.json'
$report=Join-Path $output 'search.json'
$readingReport=Join-Path $output 'reading.json'
$uiReport=Join-Path $output 'ui.json'
$readingStore=Join-Path $scratch 'reading-state.json'
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|
 ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$record=@{verification_id=[Guid]::NewGuid().ToString();stage=$Stage;
 binary_sha256=(Get-FileHash -LiteralPath $Exe).Hash;
 binary=(Resolve-Path -LiteralPath $Exe).Path;
 platform='Windows 11 26100 x64 native Win32 input and desktop screenshots';
 real_mouse_keyboard=$true;cases=@();snapshots=@();started=[DateTimeOffset]::UtcNow.ToString('o')}
$script:readingProcessRuns=@()
$script:readingObservationBases=$null
$script:readingObservationSequence=0
$record.observation_launches=@()
$script:readingClipboardModified=$false
$priorClipboard=$null
try{$priorClipboard=[WorkbenchClipboard]::GetDataObject()}catch{}

function JsonFile([string]$Path){
 for($i=0;$i-lt40;$i++){
  try{
   $raw=Get-Content -Raw -LiteralPath $Path -Encoding utf8
   if([string]::IsNullOrWhiteSpace($raw)){throw 'Observation write is in progress'}
   $parsed=$raw|ConvertFrom-Json
   if($null-eq$parsed){throw 'Observation is empty'}
   return $parsed
  }
  catch{Start-Sleep -Milliseconds 25}
 }
 throw "No complete observation: $Path"
}
function Reading {JsonFile $readingReport}
function UiState {JsonFile $uiReport}
function EnglishInputFor([IntPtr]$Handle){
 [uint32]$owner=0
 $thread=[ReadingInputNative]::GetWindowThreadProcessId($Handle,[ref]$owner)
 if($owner-ne$script:app.Id){throw 'Input-layout change refused for a window outside the test process'}
 $old=[ReadingInputNative]::GetKeyboardLayout($thread)
 $english=[ReadingInputNative]::LoadKeyboardLayout('00000409',0)
 [void][ReadingInputNative]::PostMessage($Handle,0x50,[IntPtr]::Zero,$english)
 Start-Sleep -Milliseconds 250
 return $old
}
function Keys([string]$Keys){
 [uint32]$owner=0
 $foreground=[MdcardUi]::GetForegroundWindow()
 [void][ReadingInputNative]::GetWindowThreadProcessId($foreground,[ref]$owner)
 $owned=$script:app-and!$script:app.HasExited-and$owner-eq$script:app.Id
 $event=@{utc=[DateTimeOffset]::UtcNow.ToString('o');qpc=[Diagnostics.Stopwatch]::GetTimestamp();pid=$script:app.Id;foreground_pid=$owner;foreground_hwnd=$foreground.ToInt64();owned=$owned;keys=$Keys}
 [IO.File]::AppendAllText((Join-Path $output 'native-key-input.jsonl'),($event|ConvertTo-Json -Compress)+"`n",[Text.UTF8Encoding]::new($false))
 if(!$owned){throw 'Keyboard input withheld because the foreground window is outside the owned test process'}
 [Windows.Forms.SendKeys]::SendWait($Keys)
 Start-Sleep -Milliseconds 300
}
function PathEquals([string]$First,[string]$Second){
 [string]::Equals(($First-replace'^\\\\\?\\',''),($Second-replace'^\\\\\?\\',''),[StringComparison]::OrdinalIgnoreCase)
}
function Paste([string]$Text){
 [WorkbenchClipboard]::SetText($Text)
 $script:readingClipboardModified=$true
 Start-Sleep -Milliseconds 120
 if([WorkbenchClipboard]::GetText()-cne$Text){throw 'Clipboard does not contain the exact requested test input'}
 Keys '^v'
}
function Fill([string]$Name,[string]$Text){
 Control $Name;Keys '^a';Keys '{BACKSPACE}'
 if($Text-match'^[\x00-\x7F]+$'){
  $literal=foreach($ch in $Text.ToCharArray()){
   if($ch-eq'{'){'{{}'}elseif($ch-eq'}'){'{}}'}elseif($ch-in@('+','^','%','~','(',')','[',']')){'{'+$ch+'}'}else{[string]$ch}
  }
  Keys ($literal-join'')
 }else{Paste $Text}
}
function WaitReading([scriptblock]$Predicate,[string]$Message,[int]$Seconds=15){
 $until=[DateTime]::UtcNow.AddSeconds($Seconds)
 while([DateTime]::UtcNow-lt$until){
  if($script:app.HasExited){throw "App exited before $Message"}
  $s=Reading;if(&$Predicate $s){return $s};Start-Sleep -Milliseconds 80
 }
 Shot 'failure';throw $Message
}
function UiClick([string]$Name){
 $until=[DateTime]::UtcNow.AddSeconds(8)
 do{
  $c=@((UiState).controls|Where-Object {$_.name-ceq$Name-and$_.enabled})|Select-Object -Last 1
  if($c){Click (($c.rect[0]+$c.rect[2])/2) (($c.rect[1]+$c.rect[3])/2);return}
  Start-Sleep -Milliseconds 80
 }while([DateTime]::UtcNow-lt$until)
 throw "No enabled native UI control $Name"
}
function FlushReadingProcessLogs {
 foreach($run in $script:readingProcessRuns){
  if($run.flushed-or!$run.process.HasExited){continue}
  $entry=@{pid=$run.process.Id;document=$run.document;started_utc=$run.started_utc;exit_code=$run.process.ExitCode;observed_exit_utc=[DateTimeOffset]::UtcNow.ToString('o')}
  $complete=$true
  foreach($stream in @('stdout','stderr')){
   $task=$run[$stream]
   if($task.Wait(1000)){
    $path=Join-Path $output ("process-$($run.process.Id).$stream.log")
    [IO.File]::WriteAllText($path,$task.Result,[Text.UTF8Encoding]::new($false))
    $entry[$stream]=@{file=[IO.Path]::GetFileName($path);sha256=(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()}
   }else{$entry[$stream]=@{capture_pending=$true};$complete=$false}
  }
  $entry|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $output "process-$($run.process.Id).exit.json") -Encoding utf8
  $run.flushed=$complete
 }
}
function StartReadingApp([string]$Document){
 FlushReadingProcessLogs
 # Each native launch has its own read-only observation files. Reusing and
 # deleting the previous report raced a Windows sharing handle in R25a. A
 # fresh path also makes a late observation from an exited process harmless.
 # Capture these bases here: precision drivers replace the helper's paths
 # after dot-sourcing it and before their first actual launch.
 if($null-eq$script:readingObservationBases){
  $script:readingObservationBases=@{report=$report;readingReport=$readingReport;uiReport=$uiReport}
 }
 $script:readingObservationSequence++
 foreach($name in @('report','readingReport','uiReport')){
  $base=$script:readingObservationBases[$name]
  $file=([IO.Path]::GetFileNameWithoutExtension($base))+('-launch-{0:D4}' -f $script:readingObservationSequence)+[IO.Path]::GetExtension($base)
  $fresh=Join-Path (Split-Path $base -Parent) $file
  if(Test-Path -LiteralPath $fresh){throw 'Observation launch path already exists; retain it and use a fresh stage'}
  Set-Variable -Name $name -Scope Script -Value $fresh
 }
 $launch=[Diagnostics.ProcessStartInfo]::new((Resolve-Path -LiteralPath $Exe).Path)
 $launch.UseShellExecute=$false;$launch.WorkingDirectory=$fixtures
 $launch.Environment['MDCARD_SETTINGS_PATH']=$settings
 if($WithoutSearchReport){[void]$launch.Environment.Remove('MDCARD_SEARCH_REPORT')}
 else{$launch.Environment['MDCARD_SEARCH_REPORT']=$report}
 $launch.Environment['MDCARD_UI_REPORT']=$uiReport
 $launch.Environment['MDCARD_READING_REPORT']=$readingReport
 $launch.Environment['MDCARD_READING_STATE_PATH']=$readingStore
 $launch.Environment['MDCARD_RENDERER_REPORT']=(Join-Path $output 'renderer.json')
 $renderer=Join-Path (Split-Path (Resolve-Path -LiteralPath $Exe).Path -Parent) 'renderer'
 if(!(Test-Path -LiteralPath $renderer -PathType Container)){$renderer=Join-Path $root 'dist\windows\renderer'}
 $launch.Environment['MDCARD_RENDERER_DIR']=$renderer
 $launch.ArgumentList.Add($Document)
 $launch.RedirectStandardOutput=$true;$launch.RedirectStandardError=$true
 $launch.Environment['RUST_BACKTRACE']='1'
 $script:app=[Diagnostics.Process]::Start($launch)
 $script:readingProcessRuns+=@{process=$script:app;document=$Document;started_utc=[DateTimeOffset]::UtcNow.ToString('o');stdout=$script:app.StandardOutput.ReadToEndAsync();stderr=$script:app.StandardError.ReadToEndAsync();flushed=$false}
 $record.observation_launches+=,@{sequence=$script:readingObservationSequence;pid=$script:app.Id;document=$Document;search_report=$report;reading_report=$readingReport;ui_report=$uiReport;renderer_dir=$renderer}
 $until=[DateTime]::UtcNow.AddSeconds(15)
 do{
  Start-Sleep -Milliseconds 100;$script:app.Refresh()
  if($script:app.HasExited){throw 'Test program exited during launch'}
  if($script:app.MainWindowHandle-ne0){break}
 }while([DateTime]::UtcNow-lt$until)
 if($script:app.MainWindowHandle-eq0){throw 'Test program did not create a native window'}
 # HWND creation precedes font installation and native event readiness.
 Start-Sleep -Milliseconds 1800
 $script:app.Refresh()
 [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,50,50,1550,1025,$true)
 $activated=$false
 for($attempt=0;$attempt-lt6;$attempt++){
  try{Click 500 20;$activated=$true;break}catch{Start-Sleep -Milliseconds 350;$script:app.Refresh()}
 }
 if(!$activated){throw 'Owned test window could not be activated after native event readiness'}
 $script:previousInputLayout=EnglishInputFor $script:app.MainWindowHandle
 $record.input_method='Physical SendKeys with English HKL requested only on owned test windows; original app layout restored before exit'
 $record.dpi=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)
 WaitReading {param($s)$null-ne$s.main_id} 'Initial reading report'|Out-Null
}
function Wheel([float[]]$Rect,[int]$Notches){
 if($Rect.Count-ne4){throw 'No observed viewport rectangle'}
 $b=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0
 [void][MdcardUi]::SetForegroundWindow($script:app.MainWindowHandle)
 if([MdcardUi]::GetForegroundWindow()-ne$script:app.MainWindowHandle){Click 500 20}
 if([MdcardUi]::GetForegroundWindow()-ne$script:app.MainWindowHandle){throw 'Wheel target does not own foreground'}
 # Rich diagram canvases intentionally consume wheel input for zoom. Aim near
 # the lower text region so this acceptance gesture scrolls the document.
 [void][MdcardUi]::SetCursorPos($b.Left+[int]((($Rect[0]+$Rect[2])/2)*$scale),$b.Top+[int](($Rect[1]+0.97*($Rect[3]-$Rect[1]))*$scale))
 Start-Sleep -Milliseconds 100
 $delta=if($Notches-gt0){[uint32]::MaxValue-119}else{[uint32]120}
 for($i=0;$i-lt[Math]::Abs($Notches);$i++){
  [MdcardUi]::mouse_event(0x800,0,0,$delta,[UIntPtr]::Zero)
  Start-Sleep -Milliseconds 55
 }
 Start-Sleep -Milliseconds 650
}
function QuickOpen([string]$Query,[string]$Path,[switch]$Secondary){
 Keys '^p';Fill 'quick-query' $Query
 $q=Await {param($s)!$s.query_pending-and@($s.quick|Where-Object {PathEquals $_.path $Path}).Count-gt0} "Quick target $Query"
 $chosen=-1
 for($i=0;$i-lt@($q.quick).Count;$i++){if(PathEquals $q.quick[$i].path $Path){$chosen=$i;break}}
 if($chosen-lt0){throw 'Expected Quick path disappeared'}
 for($i=0;$i-lt$chosen;$i++){Keys '{DOWN}'}
 Keys $(if($Secondary){'+{ENTER}'}else{'{ENTER}'})
 Await {param($s)!$s.quick_open-and!$s.busy} 'Quick opening finished'|Out-Null
}
function NativeOpen([string]$Path){
 Keys '^o';Start-Sleep -Milliseconds 700
 # The app-owned modal must have focus before any path input is delivered.
 $owner=[Windows.Automation.AutomationElement]::FromHandle([MdcardUi]::GetForegroundWindow())
 if($owner.Current.ProcessId-ne$script:app.Id-or$owner.Current.ClassName-ne'#32770'){
  throw 'No owned native open chooser; path input withheld'
 }
 EnglishInputFor ([MdcardUi]::GetForegroundWindow())|Out-Null
 Keys '%n';Keys '^a'
 if($Path-match'^[A-Za-z0-9:._/\\ -]+$'){Keys $Path}else{Paste $Path}
 Keys '{ENTER}'
 Start-Sleep -Milliseconds 600
}
function Observe([string]$Name,[switch]$Screenshot){
 $s=Reading;$s|ConvertTo-Json -Depth 30|Set-Content -LiteralPath (Join-Path $output "$Name.json") -Encoding utf8
 $record.snapshots+=@{name=$Name;file="$Name.json";observed=[DateTimeOffset]::UtcNow.ToString('o')}
 if($Screenshot){Shot $Name};return $s
}
function Require([bool]$Condition,[string]$Message){if(!$Condition){throw $Message}}
function Pass([string]$Name,$Evidence){$record.cases+=@{name=$Name;status='通过';evidence=$Evidence}}
function EndReadingApp{
 if($script:previousInputLayout){[void][ReadingInputNative]::PostMessage($script:app.MainWindowHandle,0x50,[IntPtr]::Zero,$script:previousInputLayout)}
 Keys '^q';if(!$script:app.WaitForExit(8000)){throw 'Clean exit did not finish'}
 FlushReadingProcessLogs
}
function View([string]$Name){(Reading).views.PSObject.Properties[$Name].Value}
function Position([string]$Name){(View $Name).position}
function RecordFor($Id){@((Reading).records|Where-Object {[string]$_.id-ceq[string]$Id})|Select-Object -First 1}
function SetMode([string]$Mode){
 UiClick $Mode
 $until=[DateTime]::UtcNow.AddSeconds(10)
 do{if((UiState).mode-ceq$Mode){return};Start-Sleep -Milliseconds 80}while([DateTime]::UtcNow-lt$until)
 throw "Mode $Mode was not shown"
}
function Settle([string]$Name){
 # Native resize/mode dispatch can publish a new rectangle before the resumed
 # observer publishes a position. A stale last position is not readiness.
 Start-Sleep -Milliseconds 650
 WaitReading {param($s)
  $v=$s.views.PSObject.Properties[$Name].Value
  $null-ne$v-and$null-ne$v.position-and$null-eq$v.pending-and
   $null-ne$v.viewport_rect-and
   [Math]::Abs($v.position.viewport[0]-($v.viewport_rect[2]-$v.viewport_rect[0]))-le2-and
   [Math]::Abs($v.position.viewport[1]-($v.viewport_rect[3]-$v.viewport_rect[1]))-le2
 } "$Name did not finish position restoration"|Out-Null
 Start-Sleep -Milliseconds 250
}
function WheelView([string]$Name,[int]$Notches){
 $v=View $Name
 $rect=$v.viewport_rect
 if(!$rect){$rect=$v.rect}
 Wheel $rect $Notches
 Settle $Name
}
function HorizontalWheelView([string]$Name){
 $v=View $Name;$rect=$v.viewport_rect;$b=Bounds
 $scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0
 [void][MdcardUi]::SetCursorPos($b.Left+[int]((($rect[0]+$rect[2])/2)*$scale),$b.Top+[int](($rect[1]+0.97*($rect[3]-$rect[1]))*$scale))
 foreach($delta in @([uint32]120,([uint32]::MaxValue-119))){
  for($i=0;$i-lt14;$i++){[MdcardUi]::mouse_event(0x1000,0,0,$delta,[UIntPtr]::Zero);Start-Sleep -Milliseconds 55}
  Settle $Name
  if((Position $Name).offset[0]-gt10){return}
 }
 throw 'Native horizontal wheel did not establish a nonzero horizontal offset'
}
function SameOffset($Expected,$Actual,[string]$Name){
 $dx=[Math]::Abs($Expected.offset[0]-$Actual.offset[0])
 $dy=[Math]::Abs($Expected.offset[1]-$Actual.offset[1])
 Require ($dx-le2-and$dy-le2) "$Name exceeded frozen 2 logical pixel tolerance: dx=$dx dy=$dy"
 return @{dx=$dx;dy=$dy;expected=$Expected;actual=$Actual}
}
function SameBlock($Expected,$Actual,[string]$Name){
 $before=$Expected.anchor.byte;$after=$Actual.anchor.byte
 $boundary='mapped Markdown block start'
 $document=State
 if((UiState).mode-eq'Write'-or($document.path-and[IO.Path]::GetExtension($document.path).ToLowerInvariant()-notin@('.md','.markdown','.mdown','.mkd','.mkdn'))){
  # Wrapped plain text exposes the first visible scalar. Acceptance is at the
  # original logical line/block, not an invented exact-character promise.
  $bytes=[Text.Encoding]::UTF8.GetBytes($document.text)
  $before=[Math]::Min($before,$bytes.Length);$after=[Math]::Min($after,$bytes.Length)
  while($before-gt0-and$bytes[$before-1]-ne10-and$bytes[$before-1]-ne13){$before--}
  while($after-gt0-and$bytes[$after-1]-ne10-and$bytes[$after-1]-ne13){$after--}
  $boundary='decoded UTF-8 original logical text line/block'
 }
 Require ($before-eq$after) "$Name changed source block: $($Expected.anchor.byte) -> $($Actual.anchor.byte), logical block $before -> $after"
 return @{source_byte=$before;anchor_before=$Expected.anchor.byte;anchor_after=$Actual.anchor.byte;boundary=$boundary;old_fraction=$Expected.anchor.fraction;new_fraction=$Actual.anchor.fraction;
  old_offset=$Expected.offset;new_offset=$Actual.offset;precision='block; fraction is approximate'}
}

# Scenario assertions are deliberately maintained in this independent harness;
# report values are read-only observations, never a way to drive the application.
if($OnlyReadingHelpers){return}
try{
 Add-Type -AssemblyName UIAutomationClient
 Add-Type -AssemblyName UIAutomationTypes
 $a=Join-Path $fixtures 'a.md';$b=Join-Path $fixtures 'b.md'
 StartReadingApp $a
 SetMode 'Read';Settle 'main/preview'
 $idA=(Reading).main_id
 Require ((Position 'main/preview').offset[1]-le2) 'A without history did not start at the top'
 WheelView 'main/preview' 48
 $late=Observe '01-a-late' -Screenshot
 $latePos=Position 'main/preview'
 Require ($latePos.anchor.byte-gt4000) 'A did not actually scroll into the document'
 $furthest=(RecordFor $idA).furthest
 WheelView 'main/preview' -12
 $lastA=Position 'main/preview'
 Require ($lastA.offset[1]-lt$latePos.offset[1]) 'Backtracking did not move to an earlier position'
 Require ((RecordFor $idA).furthest-ge$furthest) 'Furthest position was overwritten by current position'
 QuickOpen 'b.md' $b;SetMode 'Read';Settle 'main/preview'
 $idB=(Reading).main_id
 Require ([string]$idA-cne[string]$idB) 'A and B share a document ID'
 Require ((Position 'main/preview').offset[1]-le2) 'B inherited A scroll position'
 WheelView 'main/preview' 18;$lastB=Position 'main/preview'
 QuickOpen 'a.md' $a
 Require ((UiState).mode-eq'Read') 'Quick open ignored the recorded last active Read mode'
 SetMode 'Read';Settle 'main/preview'
 Require ([string](Reading).main_id-ceq[string]$idA) 'Ordinary reopen changed stable A identity'
 $aError=SameOffset $lastA (Position 'main/preview') 'A last-position restore'
 Pass 'A -> B -> A last position, independent current and furthest' $aError
 Observe '02-a-restored-last-position' -Screenshot|Out-Null
 QuickOpen 'b.md' $b;SetMode 'Read';Settle 'main/preview'
 Pass 'B independent view position' (SameOffset $lastB (Position 'main/preview') 'B restore')

 NativeOpen $a;SetMode 'Read';Settle 'main/preview'
 Require ([string](Reading).main_id-ceq[string]$idA) 'Native open changed A identity'
 Pass 'Native open shares restoration path' (SameOffset $lastA (Position 'main/preview') 'Native open A')
 SetMode 'Write';Settle 'main/source'
 $sourceRect=(View 'main/source').viewport_rect
 Click (($sourceRect[0]+$sourceRect[2])/2) (($sourceRect[1]+$sourceRect[3])/2)
 Keys '^{HOME}';Keys '+{END}'
 $selection=(UiState).selection
 $record.selection_before_switch=$selection
 Require ($selection[0]-ne$selection[1]) 'Source selection was not established'
 WheelView 'main/source' 24;$sourceA=Position 'main/source'
 Observe '03-source-before-switch' -Screenshot|Out-Null
 Require ($sourceA.offset[1]-gt500) 'Source view did not scroll away from the cursor'
 QuickOpen 'b.md' $b;QuickOpen 'a.md' $a;SetMode 'Write';Settle 'main/source'
 $sourceError=SameOffset $sourceA (Position 'main/source') 'Source position independent from cursor'
 $restoredSelection=(UiState).selection
 $record.selection_after_switch=$restoredSelection
 Require ($selection[0]-eq$restoredSelection[0]-and$selection[1]-eq$restoredSelection[1]) "Cursor/selection changed after reopen: [$selection] -> [$restoredSelection]"
 Pass 'Source scroll independent from exact cursor and selection' @{offset=$sourceError;selection=$restoredSelection}
 Observe '03-source-scroll-and-selection' -Screenshot|Out-Null

 SetMode 'Read';Settle 'main/preview'
 $wide=Position 'main/preview'
 [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,70,70,1040,800,$true)
 Settle 'main/preview';$narrow=Position 'main/preview'
 Pass 'Narrow width keeps the original content block' (SameBlock $wide $narrow 'Wide -> narrow')
 Observe '04-read-narrow-same-block' -Screenshot|Out-Null
 [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,50,50,1550,1025,$true)
 Settle 'main/preview'
 Pass 'Wide width keeps the original content block' (SameBlock $wide (Position 'main/preview') 'Narrow -> wide')
 Keys '^b';Settle 'main/preview'
 Pass 'Sidebar layout preserves content block' (SameBlock $wide (Position 'main/preview') 'Sidebar toggle')
 Keys '^b';Settle 'main/preview'
 $beforeRestart=Position 'main/preview'
 Observe '05-before-clean-restart' -Screenshot|Out-Null
 EndReadingApp
 Require (Test-Path -LiteralPath $readingStore) 'No independent persistent reading-state file was written'
 Copy-Item -LiteralPath $readingStore -Destination (Join-Path $output 'persisted-before-restart.json')
 StartReadingApp 'a.md';SetMode 'Read';Settle 'main/preview'
 Require ([string](Reading).main_id-ceq[string]$idA) 'Relative startup path did not associate the canonical A identity'
 $restartError=SameOffset $beforeRestart (Position 'main/preview') 'Restart offset'
 $restartBlock=SameBlock $beforeRestart (Position 'main/preview') 'Restart content block'
 Pass 'Clean restart and relative/absolute identity' @{offset=$restartError;block=$restartBlock}
 Observe '06-after-clean-restart' -Screenshot|Out-Null
 NativeOpen $b;SetMode 'Read';Settle 'main/preview'
 Pass 'B also survives restart independently' (SameOffset $lastB (Position 'main/preview') 'B restart')
 EndReadingApp
 $golden=Get-Content -Raw -LiteralPath (Join-Path $fixtures 'fixtures.json') -Encoding utf8|ConvertFrom-Json
 foreach($fixture in $golden.files){
  Require ((Get-FileHash -LiteralPath $fixture.path).Hash.ToLowerInvariant()-ceq$fixture.sha256) "Reading acceptance changed manuscript bytes: $($fixture.relative)"
 }
 $record.manuscript_bytes_unchanged=$true
 $record.passed=$true
}catch{
 $record.error=$_.Exception.Message;$record.error_stack=$_.ScriptStackTrace;$record.passed=$false
 if($script:app-and!$script:app.HasExited){try{Shot 'failure'}catch{}}
}finally{
 if($script:app-and!$script:app.HasExited){
  if($script:previousInputLayout){[void][ReadingInputNative]::PostMessage($script:app.MainWindowHandle,0x50,[IntPtr]::Zero,$script:previousInputLayout)}
  $script:app.Kill();$script:app.WaitForExit()
 }
 FlushReadingProcessLogs
 if($script:readingClipboardModified-and$priorClipboard){[WorkbenchClipboard]::SetDataObject($priorClipboard,$true)}
 $record.finished=[DateTimeOffset]::UtcNow.ToString('o')
 $record|ConvertTo-Json -Depth 30|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
}
$record|ConvertTo-Json -Depth 15
if(!$record.passed){throw 'Independent reading-state GUI acceptance failed; inspect result.json'}
