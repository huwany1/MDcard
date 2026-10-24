# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
 [Parameter(Mandatory=$true)][string]$Exe,
 [string]$Stage='precision-b-worker-r1',
 [ValidateRange(400000,1200000)][int]$HeadingCount=1200000,
 [ValidateRange(1,60)][int]$ButtonHoldMs=1
)
. "$PSScriptRoot\verify-reading-state-master-windows.ps1" -Exe $Exe -Stage $Stage -OnlyReadingHelpers -WithoutSearchReport
. "$PSScriptRoot/reading-task-finish-observer.ps1"
$record.harness_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant()
$record.finish_observer_sha256=(Get-FileHash -LiteralPath "$PSScriptRoot/reading-task-finish-observer.ps1").Hash.ToLowerInvariant()
$record.observation_mode='Reading task and native UI reports enabled; unrelated full-source search diagnostic disabled. Previous full diagnostic attempts retained separately.'
function FastNativeClick([float[]]$Rect){
 $bounds=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0
 Require ([MdcardUi]::GetForegroundWindow()-eq$script:app.MainWindowHandle) 'Native cancellation target does not own foreground'
 $physicalX=$bounds.Left+[int](($Rect[0]+$Rect[2])*.5*$scale)
 $physicalY=$bounds.Top+[int](($Rect[1]+$Rect[3])*.5*$scale)
 $motion=[Diagnostics.Stopwatch]::GetTimestamp()
 [void][MdcardUi]::SetCursorPos($physicalX,$physicalY)
 # Dispatch the real pointer motion before pressing a different native widget.
 # This matches the independent rapid-switch driver; motion alone does not
 # request cancellation. Measure cancellation from the subsequent button down.
 Start-Sleep -Milliseconds 45
 $start=[Diagnostics.Stopwatch]::GetTimestamp()
 [MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
 # Pointer motion already crossed a native frame. Deliver the same short
 # physical click used by the original driver and Linux acceptance. The actual
 # hold remains inside the 100ms button-down to observed-exit measurement.
 Start-Sleep -Milliseconds $ButtonHoldMs
 [MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
 $record.native_clicks+=@{rect=$Rect;physical_x=$physicalX;physical_y=$physicalY;pointer_motion_ticks=$motion;requested_motion_settle_ms=45;start_ticks=$start;end_ticks=[Diagnostics.Stopwatch]::GetTimestamp();requested_hold_ms=$ButtonHoldMs}
 return $start
}
try{
 $record.native_clicks=@()
 $dense=Join-Path $fixtures '00-heavy-index.md';$tiny=Join-Path $fixtures '01-tiny-index.md'
 # Use actual input large enough for this machine's background scan to remain
 # running through an independent observation and a real native click. Earlier
 # 400000-heading attempts finished naturally; they remain failed evidence.
 $builder=[Text.StringBuilder]::new($HeadingCount*12)
 for($i=0;$i-lt$HeadingCount;$i++){[void]$builder.AppendFormat("# h{0:d6}`n`n",$i)}
 [IO.File]::WriteAllText($dense,$builder.ToString(),[Text.UTF8Encoding]::new($false))
 [IO.File]::WriteAllText($tiny,"# Newly selected small document`n`nNormal body after a real native tree selection.",[Text.UTF8Encoding]::new($false))
 $denseHash=(Get-FileHash -LiteralPath $dense).Hash;$tinyHash=(Get-FileHash -LiteralPath $tiny).Hash
 StartReadingApp $tiny;SetMode 'Read';Settle 'main/preview'
 UiClick 'navigation:Files'
 $deadline=[DateTime]::UtcNow.AddSeconds(10)
 do{
  $controls=(UiState).controls
  $heavyRect=@($controls|Where-Object {$_.name-eq'tree:00-heavy-index.md'})[0].rect
  $tinyRect=@($controls|Where-Object {$_.name-eq'tree:01-tiny-index.md'})[0].rect
  if($heavyRect-and$tinyRect){break};Start-Sleep -Milliseconds 10
 }while([DateTime]::UtcNow-lt$deadline)
 Require ($heavyRect-and$tinyRect) 'Both independently created documents are absent from the actual file tree'
 [void](FastNativeClick $heavyRect)
 $currentUi=UiState
 $currentTiny=@($currentUi.controls|Where-Object {$_.name-eq'tree:01-tiny-index.md'})|Select-Object -First 1
 Require ($null-ne$currentTiny) 'The cancellation target is absent from the current native file-tree layout'
 $tinyRect=$currentTiny.rect
 $record.cancellation_target_rect=$tinyRect
 $timer=[Diagnostics.Stopwatch]::StartNew();$polls=@();$last=[Diagnostics.Stopwatch]::GetTimestamp();$task=$null
 do{
  $stamp=[Diagnostics.Stopwatch]::GetTimestamp();$state=Reading
  $polls+=@{ticks=$stamp;interval_ms=($stamp-$last)*1000.0/[Diagnostics.Stopwatch]::Frequency};$last=$stamp
  $task=@($state.background_tasks|Where-Object {$_.kind-eq'source-index'-and$_.running-and$_.progress_checkpoints-ge100-and$_.cancel_requested_monotonic_ns-eq0})|Select-Object -Last 1
  if($task){break};Start-Sleep -Milliseconds 1
 }while($timer.Elapsed.TotalSeconds-lt12)
 Require ($null-ne$task) 'No naturally running source-index computation was independently observed'
 $before=$state
 $finishWatch=[ReadingTaskFinishObserver]::Start($readingReport,[long]$task.task_id)
 $injection=FastNativeClick $tinyRect
 $record.heading_count=$HeadingCount
 $record.running_task_observation=$task
 $record.native_input_start_stopwatch_ticks=$injection
 Require ($finishWatch.Wait(2000)) 'Independent native task-finish observer timed out'
 $finishObservation=$finishWatch.Result|ConvertFrom-Json
 $finishObservation|ConvertTo-Json -Depth 35|Set-Content -LiteralPath (Join-Path $output 'native-finish-observation.json') -Encoding utf8
 Require ($finishObservation.observed-and$finishObservation.cancelled_while_running) 'No actual running computation was canceled before its completion'
 $state=$finishObservation.raw|ConvertFrom-Json
 $finished=@($state.background_tasks|Where-Object {$_.task_id-eq$task.task_id})|Select-Object -First 1
 $end=[long]$finishObservation.read_completed_ticks
 Require ($null-ne$finished) 'The native document switch did not stop the running index computation'
 $upper=($end-$injection)*1000.0/[Diagnostics.Stopwatch]::Frequency
 $record.native_to_observed_worker_exit_upper_bound_ms=$upper
 $record.finished_task=$finished
 $before|ConvertTo-Json -Depth 45|Set-Content -LiteralPath (Join-Path $output 'running-before-native-tree.json') -Encoding utf8
 $state|ConvertTo-Json -Depth 45|Set-Content -LiteralPath (Join-Path $output 'after-native-cancellation.json') -Encoding utf8
 Require ($finished.progress_checkpoints_at_cancel-ge100) 'The canceled task had not entered its actual source scan'
 Require ($finished.cancel_to_worker_exit_wall_ms-le100-and$upper-le100) 'Actual native cancellation exceeded the frozen 100 ms response budget'
 WaitReading {param($s)@($s.records|Where-Object {[string]$_.id-ceq[string]$s.main_id-and$_.path-like'*/01-tiny-index.md'}).Count-eq1} 'New small document was not committed after old computation cancellation'|Out-Null
 $available=([Diagnostics.Stopwatch]::GetTimestamp()-$injection)*1000.0/[Diagnostics.Stopwatch]::Frequency
 Observe 'new-small-document-after-native-cancellation' -Screenshot|Out-Null
 Pass 'Real native file-tree input cancels running source indexing' @{task_before=$task;task_after=$finished;native_to_observed_worker_exit_upper_bound_ms=$upper;new_document_observed_ms=$available;polls=$polls;source_bytes=(Get-Item -LiteralPath $dense).Length}
 EndReadingApp
 Require ((Get-FileHash -LiteralPath $dense).Hash-ceq$denseHash-and(Get-FileHash -LiteralPath $tiny).Hash-ceq$tinyHash) 'Reading cancellation changed manuscript bytes'
 $record.passed=$true
}catch{
 $record.passed=$false;$record.error=$_.Exception.Message;$record.error_stack=$_.ScriptStackTrace
 if($before){$before|ConvertTo-Json -Depth 45|Set-Content -LiteralPath (Join-Path $output 'running-before-native-tree.json') -Encoding utf8}
 if($polls){$record.polls=$polls}
 if($script:app-and!$script:app.HasExited){try{Observe 'failure-observation' -Screenshot|Out-Null}catch{}}
}finally{
 if($script:app-and!$script:app.HasExited){
  if($script:previousInputLayout){[void][ReadingInputNative]::PostMessage($script:app.MainWindowHandle,0x50,[IntPtr]::Zero,$script:previousInputLayout)}
  $script:app.Kill();$script:app.WaitForExit()
 }
 if($script:readingClipboardModified-and$priorClipboard){[WorkbenchClipboard]::SetDataObject($priorClipboard,$true)}
 $record.finished=[DateTimeOffset]::UtcNow.ToString('o')
 $record|ConvertTo-Json -Depth 55|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
}
$record|Select-Object verification_id,stage,passed,error,@{n='assertions';e={$_.cases.Count}}|ConvertTo-Json
if(!$record.passed){throw 'Independent native worker cancellation failed; inspect result.json'}
