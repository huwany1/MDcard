# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
 [Parameter(Mandatory=$true)][string]$Exe,
 [string]$Stage='precision-b-future-clear-r1'
)
. "$PSScriptRoot/verify-reading-state-master-windows.ps1" -Exe $Exe -Stage $Stage -OnlyReadingHelpers
$record.harness_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant()
function Store {
 [IO.File]::ReadAllText($readingStore,[Text.Encoding]::UTF8)|ConvertFrom-Json
}
function StoreRecord($Value,[string]$Path){
 @($Value.records.PSObject.Properties.Value|Where-Object {PathEquals $_.path $Path})|Select-Object -First 1
}
function IsCleared($Value,[string]$Path){
 $r=StoreRecord $Value $Path
 return ($null-eq$r-or@($r.views.PSObject.Properties).Count-eq0)
}
function PreferencesBottom {
 Keys '^+p';Start-Sleep -Milliseconds 350
 $content=(UiState).content_rect
 Wheel @([float]($content[2]/2-150),[float]($content[3]/2-100),[float]($content[2]/2+150),[float]($content[3]/2+100)) 20
}
try{
 $a=Join-Path $fixtures 'a.md';$b=Join-Path $fixtures 'b.md'
 $aHash=(Get-FileHash -LiteralPath $a).Hash;$bHash=(Get-FileHash -LiteralPath $b).Hash
 foreach($kind in @('single','all')){
 foreach($postClear in @('restart','same-session')){
  $caseName="$kind-$postClear"
  $readingStore=Join-Path $scratch "$caseName-state.json"
  StartReadingApp $b;SetMode 'Read';Settle 'main/preview';WheelView 'main/preview' 27
  $oldB=Position 'main/preview';EndReadingApp
  StartReadingApp $a;SetMode 'Read';Settle 'main/preview';WheelView 'main/preview' 33
  $oldA=Position 'main/preview';EndReadingApp
  Require ($oldA.offset[1]-gt500-and$oldB.offset[1]-gt500) 'Real native wheel failed to establish both original positions'
  $seed=Store;$future=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()+3600000
  foreach($entry in $seed.records.PSObject.Properties.Value){
   $entry.updated_ms=$future;$entry.activity_ms=$future
   foreach($position in $entry.views.PSObject.Properties.Value){$position.updated_ms=$future}
   foreach($summary in $entry.restore_status.PSObject.Properties.Value){$summary.updated_ms=$future}
  }
  $seed|ConvertTo-Json -Depth 65|Set-Content -LiteralPath $readingStore -Encoding utf8
  Copy-Item -LiteralPath $readingStore -Destination (Join-Path $output "$caseName-valid-future-fixture.json")
  StartReadingApp $a;SetMode 'Read';Settle 'main/preview'
  Require ($null-eq(Reading).writes.error) 'The +1h state must be accepted before testing clear'
  Require ([Math]::Abs((Position 'main/preview').offset[1]-$oldA.offset[1])-le2) 'Valid future fixture did not restore the real old position'
  PreferencesBottom
  UiClick $(if($kind-eq'all'){'Clear all reading positions'}else{"Clear this document's reading positions"})
  Start-Sleep -Milliseconds 1600
  Require ($null-eq(Reading).writes.error) 'Explicit native clear failed persistence'
  $after=Store
  Require (IsCleared $after $a) 'Disk retained A views after acknowledged native clear'
  if($kind-eq'all'){Require (IsCleared $after $b) 'Global clear retained inactive B on disk'}
  Observe "$caseName-native-clear-disk-checked" -Screenshot|Out-Null
  Copy-Item -LiteralPath $readingStore -Destination (Join-Path $output "$caseName-after-clear.json")
  Keys '{ESC}'
  if($postClear-eq'same-session'){
   # The active page remains usable after clearing. A real wheel event in
   # this same process must persist again even if its old sent stamp was ahead.
   WheelView 'main/preview' 19;$sameSession=Position 'main/preview'
   Require ($sameSession.offset[1]-gt300) 'The original instance did not capture new native reading after clear'
   EndReadingApp
   StartReadingApp $a;SetMode 'Read';Settle 'main/preview'
   $actual=Position 'main/preview';$sameError=[Math]::Abs($actual.offset[1]-$sameSession.offset[1])
   Require ($sameError-le2) 'Same-instance reading after clear was filtered out before persistence'
   Observe "$caseName-new-reading-survives-restart" -Screenshot|Out-Null
   EndReadingApp
   if($kind-eq'all'){
    StartReadingApp $b;SetMode 'Read';Settle 'main/preview'
    Require ((Position 'main/preview').offset[1]-le2) 'New reading in A revived the globally cleared inactive B'
    EndReadingApp
   }
   Pass "Valid future $kind clear permits immediate new reading in the original process" @{old_a=$oldA;old_b=$oldB;new_position=$sameSession;reopened=$actual;new_reading_offset_error_logical_px=$sameError;post_clear_strategy=$postClear;disk_checked_before_input=$true}
   continue
  }
  EndReadingApp
  # For global clear, reopen the previously inactive B, not the active A whose
  # next bind could hide a stale disk record by writing a temporary top view.
  $target=if($kind-eq'all'){$b}else{$a}
  StartReadingApp $target;SetMode 'Read';Settle 'main/preview'
  $clearedPosition=Position 'main/preview'
  Require ($clearedPosition.offset[1]-le2) 'Cleared valid-future history resurrected after normal restart'
  Observe "$caseName-restart-at-top" -Screenshot|Out-Null
  WheelView 'main/preview' 19;$newPosition=Position 'main/preview'
  Require ($newPosition.offset[1]-gt300) 'A fresh real user scroll did not establish a new position after clear'
  EndReadingApp
  StartReadingApp $target;SetMode 'Read';Settle 'main/preview'
  $reopened=Position 'main/preview';$dy=[Math]::Abs($reopened.offset[1]-$newPosition.offset[1])
  Require ($dy-le2) 'The clear barrier prevented new real reading from persisting'
  Observe "$caseName-new-reading-survives-restart" -Screenshot|Out-Null
  Pass "Valid future $kind clear is durable and new native reading remains persistable" @{old_a=$oldA;old_b=$oldB;future_fixture_ms=$future;cleared_restart=$clearedPosition;new_position=$newPosition;reopened=$reopened;new_reading_offset_error_logical_px=$dy;inactive_target=($kind-eq'all');disk_checked_before_close=$true}
  EndReadingApp
 }
 }
 Require ((Get-FileHash -LiteralPath $a).Hash-ceq$aHash-and(Get-FileHash -LiteralPath $b).Hash-ceq$bHash) 'Reading-state clear changed original manuscripts'
 $record.manuscript_bytes_unchanged=$true;$record.passed=$true
}catch{
 $record.passed=$false;$record.error=$_.Exception.Message;$record.error_stack=$_.ScriptStackTrace
 if($script:app-and!$script:app.HasExited){try{Observe 'failure-observation' -Screenshot|Out-Null}catch{}}
}finally{
 if($script:app-and!$script:app.HasExited){
  if($script:previousInputLayout){[void][ReadingInputNative]::PostMessage($script:app.MainWindowHandle,0x50,[IntPtr]::Zero,$script:previousInputLayout)}
  $script:app.Kill();$script:app.WaitForExit()
 }
 if($script:readingClipboardModified-and$priorClipboard){[WorkbenchClipboard]::SetDataObject($priorClipboard,$true)}
 $record.finished=[DateTimeOffset]::UtcNow.ToString('o')
 $record|ConvertTo-Json -Depth 65|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
}
$record|Select-Object verification_id,stage,passed,error,@{n='assertions';e={$_.cases.Count}}|ConvertTo-Json
if(!$record.passed){throw 'Independent native future-clear acceptance failed; inspect result.json'}
