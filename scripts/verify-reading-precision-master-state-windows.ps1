# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
 [Parameter(Mandatory=$true)][string]$Exe,
 [string]$Stage='precision-b-state-r1'
)
. "$PSScriptRoot\verify-reading-state-master-windows.ps1" -Exe $Exe -Stage $Stage -OnlyReadingHelpers
$record.harness_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant()
function PreferencesBottom {
 Keys '^+p';Start-Sleep -Milliseconds 350
 $content=(UiState).content_rect
 Wheel @([float]($content[2]/2-150),[float]($content[3]/2-100),[float]($content[2]/2+150),[float]($content[3]/2+100)) 20
}
function SummaryEntries($Id){
 @((RecordFor $Id).restore_status.PSObject.Properties)
}
try{
 $a=Join-Path $fixtures 'a.md'
 $original=[IO.File]::ReadAllBytes($a)
 StartReadingApp $a;SetMode 'Read';Settle 'main/preview';WheelView 'main/preview' 33
 $id=(Reading).main_id;$saved=Position 'main/preview'
 EndReadingApp
 $baseline=[IO.File]::ReadAllBytes($readingStore)
 Require ($saved.offset[1]-gt500) 'State migration fixture has no meaningful saved position'
 foreach($version in @(1,2)){
  $old=[Text.Encoding]::UTF8.GetString($baseline)|ConvertFrom-Json
  $old.version=$version
  foreach($entry in $old.records.PSObject.Properties){
   $entry.Value.PSObject.Properties.Remove('activity_ms')
   $entry.Value.PSObject.Properties.Remove('restore_status')
  }
  $old|ConvertTo-Json -Depth 55|Set-Content -LiteralPath $readingStore -Encoding utf8
  StartReadingApp $a;SetMode 'Read';Settle 'main/preview'
  $actual=Position 'main/preview'
  $dx=[Math]::Abs($actual.offset[0]-$saved.offset[0]);$dy=[Math]::Abs($actual.offset[1]-$saved.offset[1])
  Require ($dx-le2-and$dy-le2) "Schema $version did not restore the existing valid position"
  EndReadingApp
  $disk=JsonFile $readingStore
  Require ($disk.version-eq3) 'Migrated state did not atomically persist schema 3'
  Pass "Schema $version migration preserves identity and offsets" @{document_id=$id;actual=$actual;dx=$dx;dy=$dy;disk_version=$disk.version}
 }
 # A deleted target exercises a real, visible downgrade and its durable summary.
 [IO.File]::WriteAllBytes($a,[byte[]]@())
 StartReadingApp $a;SetMode 'Read';Settle 'main/preview'
 WaitReading {param($s)$r=@($s.records|Where-Object {[string]$_.id-ceq[string]$id})[0];@($r.restore_status.PSObject.Properties).Count-gt0} 'No durable restore summary after deleting the actual manuscript target'|Out-Null
 $summaries=SummaryEntries $id
 Require (@($summaries|Where-Object {$_.Value.tier-eq'invalid'}).Count-gt0) 'Empty manuscript did not retain the invalid precision grade'
 PreferencesBottom;UiClick 'reading-restore-status';Start-Sleep -Milliseconds 300
 Observe 'deleted-target-visible-summary' -Screenshot|Out-Null
 Pass 'A deleted manuscript exposes a user-visible restoration grade and reason' @{summaries=$summaries.Value;control='reading-restore-status';screenshot='deleted-target-visible-summary.png'}
 Keys '{ESC}';EndReadingApp
 StartReadingApp $a;SetMode 'Read';Settle 'main/preview'
 Require ((SummaryEntries $id).Count-gt0) 'Recent restoration summary disappeared across restart'
 PreferencesBottom;UiClick 'reading-restore-status';Start-Sleep -Milliseconds 250
 Observe 'restart-visible-summary' -Screenshot|Out-Null
 UiClick "Clear this document's reading positions";Start-Sleep -Milliseconds 1400
 Require ((SummaryEntries $id).Count-eq0) 'Clear left a restoration summary or a late task recreated it'
 Observe 'cleared-visible-summary' -Screenshot|Out-Null
 Keys '{ESC}';EndReadingApp
 StartReadingApp $a;SetMode 'Read';Settle 'main/preview'
 Require ((SummaryEntries $id).Count-eq0) 'Cleared restoration summary resurrected after restart'
 EndReadingApp
 Pass 'User clears remove persistent restoration summaries without resurrection' @{document_id=$id;restart_checked=$true}
 [IO.File]::WriteAllBytes($a,$original)
 foreach($kind in @('future-version','broken-json','illegal-anchor','extreme-record-time','extreme-clear-time')){
  $payload=switch($kind){
   'future-version' {'{"version":999,"records":{},"independent_sentinel":"preserve these bytes"}'}
   'broken-json' {'{"version":3,"records":{"do not truncate":'}
   'illegal-anchor' {'{"version":3,"records":{"bad":{"id":42,"views":{"main/preview":{"offset":[0,-1]}}}}}'}
   'extreme-record-time' {
    $invalidTime=[Text.Encoding]::UTF8.GetString($baseline)|ConvertFrom-Json
    foreach($entry in $invalidTime.records.PSObject.Properties){$entry.Value.updated_ms=[uint64]::MaxValue;$entry.Value.activity_ms=[uint64]::MaxValue}
    $invalidTime|ConvertTo-Json -Depth 55
   }
   'extreme-clear-time' {
    $invalidTime=[Text.Encoding]::UTF8.GetString($baseline)|ConvertFrom-Json
    $invalidTime.cleared_ms=[uint64]::MaxValue
    $invalidTime|ConvertTo-Json -Depth 55
   }
  }
  [IO.File]::WriteAllText($readingStore,$payload,[Text.UTF8Encoding]::new($false))
  $oldHash=(Get-FileHash -LiteralPath $readingStore).Hash
  StartReadingApp $a;SetMode 'Read';Settle 'main/preview'
  Require ((Position 'main/preview').offset[1]-le2) 'Invalid state did not safely start at the top'
  WheelView 'main/preview' 18;Start-Sleep -Milliseconds 1300
  Require ((Position 'main/preview').offset[1]-gt100) 'Persistence failure prevented normal in-memory reading'
  Require (![string]::IsNullOrWhiteSpace((Reading).writes.error)) 'Rejected state write has no inspectable error'
  PreferencesBottom;Observe "$kind-visible-error" -Screenshot|Out-Null
  Keys '{ESC}';EndReadingApp
  Require ((Get-FileHash -LiteralPath $readingStore).Hash-ceq$oldHash) 'A failed/unsupported state write destroyed the previous file'
  Pass "$kind refuses disk replacement and retains in-memory reading" @{state_sha256=$oldHash;screenshot="$kind-visible-error.png"}
 }
 Require ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([IO.File]::ReadAllBytes($a)))-ceq[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($original))) 'State-only checks changed manuscript bytes'
 $record.manuscript_unchanged=$true;$record.passed=$true
}catch{
 $record.passed=$false;$record.error=$_.Exception.Message;$record.error_stack=$_.ScriptStackTrace
 if($script:app-and!$script:app.HasExited){try{Shot 'failure'}catch{}}
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
if(!$record.passed){throw 'Independent state and visible-summary acceptance failed; inspect result.json'}
