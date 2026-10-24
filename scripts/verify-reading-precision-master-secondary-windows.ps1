# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
 [Parameter(Mandatory=$true)][string]$Exe,
 [string]$Stage='precision-b-secondary-r1',
 [ValidateSet('All','SameBuffers')][string]$SecondaryScenario='All'
)
. "$PSScriptRoot\verify-reading-precision-master-relocation-windows.ps1" -Exe $Exe -Stage $Stage -Mode Write -OnlyRelocationHelpers
$record.scenario=$SecondaryScenario
$record.harness_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant()
$record.precision_scope='independent original grapheme and actual ordinary/virtual secondary Galley; native divider/font/wrap/open/restart/external edit'
$key='secondary/source'
Copy-Item -LiteralPath (Join-Path $fixtures 'secondary.md') -Destination (Join-Path $fixtures 'secondary.txt')
[void](AddOrUpdateFixture (Join-Path $fixtures 'secondary.txt'))

function SideSelection {
 $rect=(View $key).viewport_rect
 Click (($rect[0]+$rect[2])/2) (($rect[1]+$rect[3])/2)
 Keys '^{HOME}';Keys '+{END}'
 $selection=(RecordFor (Reading).secondary_id).selections.PSObject.Properties[$key].Value
 Require ($selection.Count-eq2-and$selection[0]-ne$selection[1]) 'No independent secondary selection at the beginning'
 return $selection
}
function CheckSideSelection($Expected){
 $selection=(RecordFor (Reading).secondary_id).selections.PSObject.Properties[$key].Value
 Require ($selection[0]-eq$Expected[0]-and$selection[1]-eq$Expected[1]) 'Automatic secondary reading recovery changed cursor/selection'
}
function DragSideDivider([int]$Delta){
 $rect=(View $key).viewport_rect;$bounds=Bounds
 $scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0
 Require ([MdcardUi]::GetForegroundWindow()-eq$script:app.MainWindowHandle) 'Divider input lost owned foreground'
 $startX=$bounds.Left+[int](($rect[0]-16)*$scale)
 $screenY=$bounds.Top+[int]((($rect[1]+$rect[3])/2)*$scale)
 [void][MdcardUi]::SetCursorPos($startX,$screenY)
 Start-Sleep -Milliseconds 150
 [MdcardUi]::mouse_event(0x2,0,0,0,[UIntPtr]::Zero)
 for($step=1;$step-le12;$step++){
  [void][MdcardUi]::SetCursorPos($startX+[int]($Delta*$scale*$step/12),$screenY)
  Start-Sleep -Milliseconds 30
 }
 [MdcardUi]::mouse_event(0x4,0,0,0,[UIntPtr]::Zero)
 Settle $key
 $after=(View $key).viewport_rect
 Require ([Math]::Abs(($after[2]-$after[0])-($rect[2]-$rect[0]))-ge50) 'Native divider gesture did not actually change secondary width'
}

try{
 if($SecondaryScenario-eq'All'){
 foreach($name in @('secondary.md','secondary.txt','large.md','large.txt')){
  $mainPath=Join-Path $fixtures 'precision.md';$path=Join-Path $fixtures $name
  $oldHash=(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()
  StartReadingApp $mainPath;SetMode 'Write'
  UiClick 'source-wrap';UiClick 'source-wrap-on';SetMode 'Read';Settle 'main/preview'
  WheelView 'main/preview' 12;$main=Point 'main/preview'
  QuickOpen $name $path -Secondary;Settle $key
  $selection=SideSelection
  WheelView $key 18;$point=Point $key
  Require (!$point.clamped-and$point.source_byte-gt100) 'Secondary baseline is not an interior real text target'
  $identity=[string](Reading).secondary_id
  Observe "$name-before" -Screenshot|Out-Null
  DragSideDivider -100
  Pass "$name wider secondary" (SamePoint $point $key 'Widen secondary')
  CheckSideSelection $selection
  DragSideDivider 100
  Pass "$name secondary width return" (SamePoint $point $key 'Restore secondary width')
  FontChange
  Pass "$name secondary font" (SamePoint $point $key 'Secondary actual font reflow')
  CheckSideSelection $selection
  Pass "$name main pane remains independent" (SamePoint $main 'main/preview' 'Main after secondary reflow')
  SetMode 'Write';UiClick 'source-wrap';UiClick 'source-wrap-off'
  Pass "$name secondary nowrap" (SamePoint $point $key 'Secondary nowrap' -AllowClamp)
  CheckSideSelection $selection
  UiClick 'source-wrap';UiClick 'source-wrap-on'
  Pass "$name secondary wrap" (SamePoint $point $key 'Secondary wrap')
  CheckSideSelection $selection
  Observe "$name-before-secondary-switch" -Screenshot|Out-Null
  $saved=Position $key
  QuickOpen 'mapping.md' (Join-Path $fixtures 'mapping.md') -Secondary;Settle $key
  QuickOpen $name $path -Secondary;Settle $key
  Require ([string](Reading).secondary_id-ceq$identity) 'Secondary ordinary reopen changed DocumentId'
  Pass "$name secondary C then A" (SamePoint $point $key 'Secondary reopen')
  Pass "$name secondary offset C then A" (SameOffset $saved (Position $key) 'Secondary offset reopen')
  CheckSideSelection $selection
  Observe "$name-reflow-and-reopen" -Screenshot|Out-Null
  EndReadingApp
  StartReadingApp $mainPath
  QuickOpen $name $path -Secondary;Settle $key
  Pass "$name secondary restart" (SamePoint $point $key 'Secondary restart' -FreshProcess)
  Pass "$name secondary restart offsets" (SameOffset $saved (Position $key) 'Secondary offset restart')
  CheckSideSelection $selection
  Observe "$name-restart" -Screenshot|Out-Null
  EndReadingApp
  Require ((Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()-ceq$oldHash) 'Read-only secondary test changed original manuscript bytes'

  if($name-in@('secondary.md','secondary.txt')){
   $answerPath=Join-Path $output "$name-major-prefix-oracle.json"
   python "$PSScriptRoot\transform-reading-precision-master-gui.py" --source $path --byte $point.source_byte --end-byte $point.source_end --kind prefix-insert --answer $answerPath
   Require ($LASTEXITCODE-eq0) 'Independent secondary major edit failed'
   $answer=JsonFile $answerPath
   Require ($answer.character_edit_ratio-ge0.30) 'Secondary external edit is not major under the frozen scalar definition'
   [void](AddOrUpdateFixture $path)
   StartReadingApp $mainPath;QuickOpen $name $path -Secondary;Settle $key
   $actual=Point $key;$decision=(View $key).relocation
   Require ([string](Reading).secondary_id-ceq$identity) 'External change replaced stable secondary DocumentId'
   Require ($decision.exact-and$decision.tier-eq'text') 'Secondary major edit failed precise relocation'
   Require ($actual.source_byte-eq$answer.expected_source_byte-and$actual.source_end-eq$answer.expected_source_end-and$actual.source_text-ceq$answer.original_grapheme) 'Secondary relocation disagrees with independent original scalar provenance'
   $dy=[Math]::Abs($actual.viewport_y-$point.viewport_y)
   Require (!$actual.clamped-and$dy-le2) 'Secondary major edit exceeded actual viewport tolerance'
   Start-Sleep -Milliseconds 2100;$tail=Point $key
   Require ($tail.source_byte-eq$actual.source_byte-and[Math]::Abs($tail.viewport_y-$point.viewport_y)-le2-and$tail.compensations-eq$actual.compensations) 'Secondary relocation failed stable tail'
   Pass "$name secondary major external edit" @{old=$point;actual=$actual;tail=$tail;dy_logical_px=$dy;oracle=$answer;decision=$decision;stable_tail_ms=2100}
   Observe "$name-major-prefix" -Screenshot|Out-Null
   EndReadingApp
  }
 }
 }
 # Same file, equal revision numbers, distinct unsaved sources. Validate each
 # actual glyph against independently constructed native-edit expectations.
 $samePath=Join-Path $fixtures 'precision.md'
 $original=[IO.File]::ReadAllText($samePath,[Text.Encoding]::UTF8)
 $sameHash=(Get-FileHash -LiteralPath $samePath).Hash.ToLowerInvariant()
 StartReadingApp (Join-Path $fixtures 'mapping.md');SetMode 'Write';Settle 'main/source'
 QuickOpen 'precision.md' $samePath -Secondary;Settle $key
 NativeOpen $samePath;CurrentDocument $samePath;SetMode 'Write';Settle 'main/source';Settle $key
 $rect=(View 'main/source').viewport_rect
 Click (($rect[0]+$rect[2])/2) (($rect[1]+$rect[3])/2);Keys '^{HOME}';Paste '主🙂'
 $rect=(View $key).viewport_rect
 Click (($rect[0]+$rect[2])/2) (($rect[1]+$rect[3])/2);Keys '^{HOME}';Paste 'S'
 $versions=Reading
 Require ([string]$versions.main_id-ceq[string]$versions.secondary_id) 'Equal-file precision fixture does not share DocumentId'
 Require ($versions.main_revision-eq$versions.secondary_revision) 'Equal-file precision fixture did not reach equal independent revisions'
 $mainExpected='主🙂'+$original;$sideExpected='S'+$original
 WheelView 'main/source' 12;WheelView $key 18
 $mainPoint=Point 'main/source' -ExpectedSource $mainExpected
 $sidePoint=Point $key -ExpectedSource $sideExpected
 Require ([string](Position 'main/source').fingerprint-cne[string](Position $key).fingerprint) 'Different same-file buffers share a fingerprint'
 Require ([string](Position 'main/source').text_anchor.source.source_fingerprint-cne[string](Position $key).text_anchor.source.source_fingerprint) 'Different same-file buffers share original-text proof'
 FontChange
 foreach($pair in @(
  @{view='main/source';expected=$mainExpected;point=$mainPoint},
  @{view=$key;expected=$sideExpected;point=$sidePoint}
 )){
  Settle $pair.view;$actual=Point $pair.view -ExpectedSource $pair.expected
  $dy=[Math]::Abs($actual.viewport_y-$pair.point.viewport_y)
  Require ($actual.source_byte-eq$pair.point.source_byte-and$actual.source_end-eq$pair.point.source_end-and$actual.source_text-ceq$pair.point.source_text-and!$actual.clamped-and$dy-le2) 'Same-file equal-revision buffers mixed text identity or viewport geometry'
  Start-Sleep -Milliseconds 2100;$tail=Point $pair.view -ExpectedSource $pair.expected
  Require ($tail.source_byte-eq$actual.source_byte-and[Math]::Abs($tail.viewport_y-$pair.point.viewport_y)-le2-and$tail.compensations-eq$actual.compensations) 'Same-file buffer precision drifted during stable tail'
  Pass "Same file equal revision $($pair.view) keeps independent exact text" @{old=$pair.point;actual=$actual;tail=$tail;dy_logical_px=$dy;stable_tail_ms=2100;main_revision=$versions.main_revision;secondary_revision=$versions.secondary_revision;document_id=$versions.main_id}
 }
 Observe 'same-file-different-source-equal-revision' -Screenshot|Out-Null
 # Match the existing same-file regression's visible cleanup. An egui caret
 # move can occupy an undo snapshot; this fixture tests reading isolation,
 # and does not impose a new one-press undo coalescing contract.
 $rect=(View $key).viewport_rect
 Click (($rect[0]+$rect[2])/2) (($rect[1]+$rect[3])/2);Keys '^{HOME}';Keys '+{RIGHT}';Keys '{DELETE}'
 Await {param($s)$s.secondary.text-ceq$original-and!$s.secondary.dirty} 'Remove secondary test prefix'|Out-Null
 $rect=(View 'main/source').viewport_rect
 Click (($rect[0]+$rect[2])/2) (($rect[1]+$rect[3])/2);Keys '^{HOME}';Keys '+{RIGHT 2}';Keys '{DELETE}'
 Await {param($s)$s.text-ceq$original} 'Remove main test prefix'|Out-Null
 $record.cleanup='Visible native Home/selection/Delete removes only independent test prefixes; original disk hash checked below. Undo regression has its own actual in-app case.'
 EndReadingApp
 Require ((Get-FileHash -LiteralPath $samePath).Hash.ToLowerInvariant()-ceq$sameHash) 'Equal-file buffer precision test changed original disk bytes'
 $record.passed=$true
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
if(!$record.passed){throw 'Independent secondary precision GUI failed; inspect retained result.json'}
