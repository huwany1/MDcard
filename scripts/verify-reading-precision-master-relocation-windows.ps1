# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
 [Parameter(Mandatory=$true)][string]$Exe,
 [string]$Stage='precision-b-r1',
 [ValidateSet('Read','Write')][string]$Mode='Read',
 [ValidateSet('precision.md','precision.txt','large.md','large.txt')][string]$BaseDocument='precision.md',
 [string[]]$Kinds=@('prefix-insert','prefix-delete','move-rename','split-merge','rewrite-left','rewrite-right','duplicate-all','delete-section','unrelated','empty'),
 [long]$TargetSourceByte=-1,
 [switch]$OnlyRelocationHelpers
)
. "$PSScriptRoot\verify-reading-precision-master-windows.ps1" -Exe $Exe -Scenario lifecycle -Stage $Stage -Mode $Mode -OnlyPrecisionHelpers
$record.harness_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant()
$record.transform_sha256=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'transform-reading-precision-master-gui.py')).Hash.ToLowerInvariant()
$record.precision_scope='independent edit provenance, original Unicode grapheme and actual current-pass viewport geometry after external edits'
$record.base_document=$BaseDocument

function AddOrUpdateFixture([string]$Path){
 $manifest=JsonFile (Join-Path $fixtures 'precision-fixtures.json')
 $bytes=[IO.File]::ReadAllBytes($Path)
 $decoded=[Text.UTF8Encoding]::new($false,$true).GetString($bytes)
 $digest=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()
 $entry=@{relative=[IO.Path]::GetFileName($Path);path=$Path;encoding='utf-8';bytes=$bytes.Length;
  decoded_utf8_bytes=$bytes.Length;decoded_utf8_sha256=$digest;sha256=$digest}
 $manifest.files=@($manifest.files|Where-Object {!(PathEquals $_.path $Path)})+@($entry)
 $manifest|ConvertTo-Json -Depth 30|Set-Content -LiteralPath (Join-Path $fixtures 'precision-fixtures.json') -Encoding utf8
 return $digest
}
function FineNativeWheel([int]$Delta){
 $v=View $key;$rect=$v.viewport_rect;$bounds=Bounds
 $scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0
 Require ([MdcardUi]::GetForegroundWindow()-eq$script:app.MainWindowHandle) 'Fine native wheel lost owned foreground'
 [void][MdcardUi]::SetCursorPos($bounds.Left+[int](($rect[0]+20)*$scale),$bounds.Top+[int](($rect[3]-15)*$scale))
 $encoded=if($Delta-lt0){[uint32]([long][uint32]::MaxValue+1+$Delta)}else{[uint32]$Delta}
 [MdcardUi]::mouse_event(0x800,0,0,$encoded,[UIntPtr]::Zero)
 Start-Sleep -Milliseconds 100
}
if($OnlyRelocationHelpers){return}

try{
 foreach($kind in $Kinds){
  $extension=[IO.Path]::GetExtension($BaseDocument)
  $name="relocate-$kind$extension";$path=Join-Path $fixtures $name
  Copy-Item -LiteralPath (Join-Path $fixtures $BaseDocument) -Destination $path
  $oldHash=AddOrUpdateFixture $path
  [void](OpenFixture $name)
  $selection=if($Mode-eq'Write'){SourceSelection}else{$null}
  # Establish a genuine interior target from real wheel events. An
  # assumed number of notches cannot prove the same region across document
  # sizes, line heights and virtualization paths.
  $decodedBytes=[Text.Encoding]::UTF8.GetByteCount((State).text)
  $regionSamples=@()
  for($attempt=0;$attempt-lt80;$attempt++){
   $candidate=Point $key;$ratio=$candidate.source_byte/[double]$decodedBytes
   $regionSamples+=@{source_byte=$candidate.source_byte;ratio=$ratio;offset=(Position $key).offset}
   if($ratio-ge0.60-and$ratio-lt0.80){break}
   if($ratio-ge0.80){WheelView $key -12}else{NativeLargeWheel $key 1}
  }
  Require ($ratio-ge0.60-and$ratio-lt0.80) 'Actual old target did not reach the required interior region'
  if($TargetSourceByte-ge0){
   # This selects the known previous GUI regression through real wheel input.
   # It sets no product state; the transformed answer still comes solely from
   # independently retained original-scalar edit provenance.
   $fineSamples=@();$lastDirection=0;$fineStep=12
   for($fine=0;$fine-lt240;$fine++){
    $point=Point $key
    $fineSamples+=@{source_byte=$point.source_byte;offset=(Position $key).offset}
    if($point.source_byte-eq$TargetSourceByte){break}
    $direction=if($point.source_byte-lt$TargetSourceByte){-1}else{1}
    if($lastDirection-ne0-and$lastDirection-ne$direction){$fineStep=3}
    $amount=if([Math]::Abs($point.source_byte-$TargetSourceByte)-gt1000){120}else{$fineStep}
    FineNativeWheel ($direction*$amount);$lastDirection=$direction
   }
   Require ($point.source_byte-eq$TargetSourceByte) 'Real fine wheel did not reach the original regression grapheme'
   $record.regression_target_source_byte=$TargetSourceByte;$record.fine_native_wheel_samples=$fineSamples
  }
  $record.last_region_observations=$regionSamples
  $before=Point $key
  Require (!$before.clamped) 'Select a non-clamped old reading target'
  Require ($before.source_byte-gt30000) 'Actual old target is too early for the major-prefix deletion fixture'
  $oldPosition=Position $key;$id=[string](Reading).main_id
  Observe "$kind-before" -Screenshot|Out-Null
  EndReadingApp

  $answerPath=Join-Path $output "$kind-oracle.json"
  python "$PSScriptRoot\transform-reading-precision-master-gui.py" --source $path --byte $before.source_byte --end-byte $before.source_end --kind $kind --answer $answerPath
  Require ($LASTEXITCODE-eq0) 'Independent original-manuscript edit transform failed'
  $answer=JsonFile $answerPath
  $newHash=AddOrUpdateFixture $path
  Require ($answer.old_sha256-ceq$oldHash-and$answer.new_sha256-ceq$newHash) 'Edit oracle and actual manuscript hashes disagree'

  StartReadingApp $path;SetMode $Mode;Settle $key
  Require ([string](Reading).main_id-ceq$id) 'External edit changed stable DocumentId'
  $decision=(View $key).relocation
  Require ($null-ne$decision) 'No independent observable last version-relocation decision'
  if($answer.category-eq'eligible'){
   $actual=Point $key
   Require ($decision.exact-eq$true-and$decision.tier-eq'text') 'A distinguishable surviving target was downgraded instead of precisely restored'
   Require ($actual.source_byte-eq$answer.expected_source_byte-and$actual.source_end-eq$answer.expected_source_end) 'Restored original range disagrees with independent edit provenance'
   Require ($actual.source_text-ceq$answer.original_grapheme) 'Restored target is a different original grapheme'
   $dy=[Math]::Abs($actual.viewport_y-$before.viewport_y)
   Require (!$actual.clamped-and$dy-le2) "Relocation viewport error exceeds frozen precision: $dy"
   Start-Sleep -Milliseconds 2100
   $tail=Point $key
   Require ($tail.source_byte-eq$answer.expected_source_byte-and[Math]::Abs($tail.viewport_y-$before.viewport_y)-le2) 'Relocation drifted during the stable observation interval'
   Require ($tail.compensations-eq$actual.compensations) 'Relocation continued compensating after convergence'
   Pass "$kind precise relocation" @{old=$before;actual=$actual;tail=$tail;dy_logical_px=$dy;decision=$decision;oracle=$answer;stable_tail_ms=2100}
  }else{
   Require (!$decision.exact) 'Ambiguous or deleted original reading target was falsely declared exact'
   Require (![string]::IsNullOrWhiteSpace($decision.reason)) 'A downgraded result has no inspectable reason'
   Pass "$kind explicit safe downgrade" @{old=$before;position=(Position $key);decision=$decision;oracle=$answer}
  }
  # A shortened/deleted manuscript may legitimately clamp a saved selection;
  # compare unchanged coordinate selections only in surviving-target cases.
  if($selection-and$answer.category-eq'eligible'){CheckSelection $selection}
  Observe "$kind-after" -Screenshot|Out-Null
  EndReadingApp
 }
 $manifest=JsonFile (Join-Path $fixtures 'precision-fixtures.json')
 foreach($fixture in $manifest.files){
  Require ((Get-FileHash -LiteralPath $fixture.path).Hash.ToLowerInvariant()-ceq$fixture.sha256) "Application modified an original manuscript: $($fixture.relative)"
 }
 $record.manuscript_bytes_match_independent_edits=$true;$record.passed=$true
}catch{
 $record.error=$_.Exception.Message;$record.error_stack=$_.ScriptStackTrace;$record.passed=$false
 if($script:app-and!$script:app.HasExited){try{Shot 'failure'}catch{}}
}finally{
 if($script:app-and!$script:app.HasExited){
  if($script:previousInputLayout){[void][ReadingInputNative]::PostMessage($script:app.MainWindowHandle,0x50,[IntPtr]::Zero,$script:previousInputLayout)}
  $script:app.Kill();$script:app.WaitForExit()
 }
 $env:MDCARD_READING_GEOMETRY_REPORT=$oldGeometryEnv
 if($script:readingClipboardModified-and$priorClipboard){[WorkbenchClipboard]::SetDataObject($priorClipboard,$true)}
 $record.finished=[DateTimeOffset]::UtcNow.ToString('o')
 $record|ConvertTo-Json -Depth 45|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
}
$record|Select-Object verification_id,stage,passed,error,@{n='assertions';e={$_.cases.Count}}|ConvertTo-Json
if(!$record.passed){throw 'Independent major-edit GUI acceptance failed; inspect result.json'}
