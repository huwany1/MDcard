# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
 [Parameter(Mandatory=$true)][string]$Exe,
 [string]$Stage='precision-b-inapp-r1',
 [switch]$TargetHeadingRegression
)
. "$PSScriptRoot\verify-reading-precision-master-relocation-windows.ps1" -Exe $Exe -Stage $Stage -Mode Read -OnlyRelocationHelpers
$record.harness_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant()
$record.precision_scope='real source paste, source undo while preview remains separately scrolled; independent original-scalar edit provenance'
function WaitUi([scriptblock]$Condition,[string]$Label){
 $deadline=[DateTime]::UtcNow.AddSeconds(20)
 do{
  $snapshot=UiState
  if(&$Condition $snapshot){return $snapshot}
  Start-Sleep -Milliseconds 50
 }while([DateTime]::UtcNow-lt$deadline)
 throw $Label
}
function Utf16FromScalar([string]$Text,[long]$Scalar){
 $utf16=0;$count=[long]0
 while($count-lt$Scalar-and$utf16-lt$Text.Length){
  if([char]::IsHighSurrogate($Text[$utf16])-and$utf16+1-lt$Text.Length-and[char]::IsLowSurrogate($Text[$utf16+1])){$utf16+=2}else{$utf16++}
  $count++
 }
 Require ($count-eq$Scalar) 'Source cursor exceeds independent decoded scalar range'
 return $utf16
}

try{
 $path=Join-Path $fixtures 'precision.md'
 $original=[IO.File]::ReadAllText($path,[Text.Encoding]::UTF8)
 $originalHash=(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()
 [void](OpenFixture 'precision.md');SetMode 'Split';Settle 'main/source';Settle 'main/preview'
 $sourceRect=(View 'main/source').viewport_rect
 Click (($sourceRect[0]+$sourceRect[2])/2) (($sourceRect[1]+$sourceRect[3])/2)
 Keys '^{HOME}'
 $insertedPrefix=("新增审核前言：文档身份与阅读位置分别记录，正文编辑不应把另一个视口拉回光标。`n`n"*900)
 $prefixPath=Join-Path $scratch 'native-paste-prefix.txt'
 [IO.File]::WriteAllText($prefixPath,$insertedPrefix,[Text.UTF8Encoding]::new($false))
 Paste $insertedPrefix
 WaitReading {param($s)$s.main_revision-gt0} 'Actual source paste revision'|Out-Null
 Require ((State).text-ceq($insertedPrefix+$original)) 'Native source paste did not create the independently expected old version'
 Keys '^s'
 WaitUi {param($s)!$s.dirty} 'Source paste saved'|Out-Null
 [void](AddOrUpdateFixture $path)
 # No click follows: native wheel leaves keyboard focus in the source editor.
 $prefixBytes=[Text.Encoding]::UTF8.GetByteCount($insertedPrefix)
 $originalBytes=[Text.Encoding]::UTF8.GetByteCount($original)
 for($attempt=0;$attempt-lt80;$attempt++){
  $candidate=Point 'main/preview'
  $originalRatio=($candidate.source_byte-$prefixBytes)/[double]$originalBytes
  if($originalRatio-ge0.55-and$originalRatio-lt0.80){break}
  if($originalRatio-ge0.80){WheelView 'main/preview' -12}else{NativeLargeWheel 'main/preview' 1}
 }
 Require ($originalRatio-ge0.55-and$originalRatio-lt0.80) 'Native wheel did not establish a surviving interior target after the real pasted prefix'
 if($TargetHeadingRegression){
  # Revisit the actual R16 failure through native scrolling. The heading is
  # selected by the independent original, never injected into product state.
  $heading=$original.IndexOf('## 操作章节 0009',[StringComparison]::Ordinal)
  Require ($heading-ge0) 'The independently generated regression heading is absent'
  $targetByte=[Text.Encoding]::UTF8.GetByteCount($insertedPrefix+$original.Substring(0,$heading+3))
  $fineSamples=@();$lastDirection=0;$fineStep=12
  for($fine=0;$fine-lt240;$fine++){
   $candidate=Point 'main/preview'
   $fineSamples+=@{source_byte=$candidate.source_byte;offset=(Position 'main/preview').offset}
   if($candidate.source_byte-eq$targetByte){break}
   $direction=if($candidate.source_byte-lt$targetByte){-1}else{1}
   if($lastDirection-ne0-and$lastDirection-ne$direction){$fineStep=3}
   $amount=if([Math]::Abs($candidate.source_byte-$targetByte)-gt1000){120}else{$fineStep}
   FineNativeWheel ($direction*$amount);$lastDirection=$direction
  }
  Require ($candidate.source_byte-eq$targetByte-and$candidate.source_text-ceq'操') 'Native fine wheel did not reach the original failing heading grapheme'
  $record.original_regression_target_source_byte=$targetByte
  $record.fine_native_wheel_samples=$fineSamples
 }
 $before=Point 'main/preview';$beforeRevision=(Reading).main_revision
 Require ($before.source_byte-gt[Text.Encoding]::UTF8.GetByteCount($insertedPrefix)) 'Old preview target is still in the soon-to-be-deleted prefix'
 Observe 'before-actual-source-undo' -Screenshot|Out-Null
 $answerPath=Join-Path $output 'source-undo-oracle.json'
 $expectedNew=Join-Path $scratch 'independent-expected-new.md'
 python "$PSScriptRoot\transform-reading-precision-master-gui.py" --source $path --byte $before.source_byte --end-byte $before.source_end --kind undo-prefix --prefix-file $prefixPath --output $expectedNew --answer $answerPath
 Require ($LASTEXITCODE-eq0) 'Independent undo edit-provenance transform failed'
 $answer=JsonFile $answerPath
 Require ($answer.new_sha256-ceq$originalHash-and$answer.character_edit_ratio-ge0.30) 'Independent expected source or major-edit amount is incorrect'

 Keys '^z'
 WaitReading {param($s)$s.main_revision-gt$beforeRevision} 'Real source undo changed content revision'|Out-Null
 Require ((State).text-ceq$original) 'Real native Ctrl+Z did not undo exactly the independent source paste'
 $selectionAfterInput=(UiState).selection;$focusAfterInput=(UiState).focused
 $manifest=JsonFile (Join-Path $fixtures 'precision-fixtures.json')
 foreach($entry in $manifest.files){if(PathEquals $entry.path $path){$entry.decoded_utf8_sha256=$originalHash;$entry.decoded_utf8_bytes=[Text.Encoding]::UTF8.GetByteCount($original)}}
 $manifest|ConvertTo-Json -Depth 30|Set-Content -LiteralPath (Join-Path $fixtures 'precision-fixtures.json') -Encoding utf8
 Settle 'main/preview';$actual=Point 'main/preview';$decision=(View 'main/preview').relocation
 Require ($null-ne$decision-and$decision.exact-eq$true-and$decision.tier-eq'text') 'In-app source undo did not produce an observable precise version-relocation decision'
 Require ($actual.source_byte-eq$answer.expected_source_byte-and$actual.source_end-eq$answer.expected_source_end) 'In-app relocation disagrees with independent original-scalar provenance'
 Require ($actual.source_text-ceq$answer.original_grapheme-and!$actual.clamped) 'In-app restore changed original text or unexpectedly hit a boundary'
 $deltaY=[Math]::Abs($actual.viewport_y-$before.viewport_y)
 Require ($deltaY-le2) "In-app relocation exceeded frozen viewport tolerance: $deltaY"
 Start-Sleep -Milliseconds 2100;$tail=Point 'main/preview'
 Require ($tail.source_byte-eq$answer.expected_source_byte-and[Math]::Abs($tail.viewport_y-$before.viewport_y)-le2) 'In-app relocation drifted during its stable tail'
 Require ($tail.compensations-eq$actual.compensations) 'In-app relocation kept compensating after convergence'
 CheckSelection $selectionAfterInput
 Require ((UiState).focused-ceq$focusAfterInput) 'Automatic preview restoration stole source keyboard focus'
 Pass 'Real in-app undo preserves the independent preview text anchor without changing post-input cursor, selection or focus' @{old=$before;actual=$actual;tail=$tail;oracle=$answer;decision=$decision;dy_logical_px=$deltaY;stable_tail_ms=2100;selection_after_user_input=$selectionAfterInput;focused_after_user_input=$focusAfterInput}
 Observe 'after-actual-source-undo' -Screenshot|Out-Null
 # Prove real keyboard ownership after automatic restoration, beyond the
 # app's own focused-pane flag. These are user edits in the owned fixture.
 $low=Utf16FromScalar $original ([Math]::Min($selectionAfterInput[0],$selectionAfterInput[1]))
 $high=Utf16FromScalar $original ([Math]::Max($selectionAfterInput[0],$selectionAfterInput[1]))
 $expectedTyped=$original.Substring(0,$low)+'K'+$original.Substring($high)
 Keys 'K'
 Require ((State).text-ceq$expectedTyped) 'Source editor did not retain actual keyboard focus and recorded cursor/selection after preview restoration'
 Keys '^z'
 Require ((State).text-ceq$original) 'Actual source focus probe did not undo atomically'
 Pass 'Actual post-restore typing still targets source and one undo restores original bytes' @{native_input='K then CtrlZ';actual_source_equals_independent_original=$true}
 Keys '^s'
 WaitUi {param($s)!$s.dirty} 'Restored original source saved'|Out-Null
 Require ((Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()-ceq$originalHash) 'The controlled source undo changed original manuscript bytes'
 [void](AddOrUpdateFixture $path)
 EndReadingApp
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
if(!$record.passed){throw 'Independent in-app relocation GUI acceptance failed; inspect result.json'}
