# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
 [Parameter(Mandatory=$true)][string]$Exe,
 [Parameter(Mandatory=$true)][ValidateSet('reflow','lifecycle','mapping','boundaries')][string]$Scenario,
 [string]$Stage='precision-a-r1',
 [string[]]$Documents=@('precision.md','precision.txt','large.md','large.txt'),
 [ValidateSet('Read','Write')][string]$Mode='Read',
 [ValidateSet('Start','Middle','Late')][string]$TargetRegion='Start',
 [switch]$OnlyPrecisionHelpers
)
$caseStage="$Stage-$Scenario-$($Mode.ToLower())"
. "$PSScriptRoot\verify-reading-state-master-windows.ps1" -Exe $Exe -Stage $caseStage -OnlyReadingHelpers
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$output=Join-Path $root "evidence\reading-precision\master\$caseStage"
$scratch=Join-Path $root "artifacts\reading-precision\master\$caseStage"
$fixtures=Join-Path $scratch 'workspace'
if(Test-Path -LiteralPath (Join-Path $output 'result.json')){throw 'Use a fresh stage; previous evidence is immutable'}
New-Item -ItemType Directory -Force $output,$scratch|Out-Null
python "$PSScriptRoot\generate-reading-precision-master-gui.py" --output $fixtures
if($LASTEXITCODE-ne0){throw 'Independent precision fixture generation failed'}
$settings=Join-Path $scratch 'settings.json'
$report=Join-Path $output 'search.json'
$readingReport=Join-Path $output 'reading.json'
$uiReport=Join-Path $output 'ui.json'
$readingStore=Join-Path $scratch 'reading-state.json'
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|
 ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$record.stage=$caseStage
$record.started=[DateTimeOffset]::UtcNow.ToString('o')
$record.precision_scope='actual original grapheme plus current-pass Galley viewport geometry; no block interpolation'
$record.snapshots=@();$record.cases=@()
$record.harness_sha256=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'verify-reading-precision-master-windows.ps1')).Hash.ToLowerInvariant()
$record.fixture_generator_sha256=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'generate-reading-precision-master-gui.py')).Hash.ToLowerInvariant()
$oldGeometryEnv=$env:MDCARD_READING_GEOMETRY_REPORT
$env:MDCARD_READING_GEOMETRY_REPORT='1'
$key=if($Mode-eq'Write'){'main/source'}else{'main/preview'}
$script:precisionBoundaryCache=@{}

function JsonFile([string]$Path){
 $lastFailure=''
 for($attempt=0;$attempt-lt80;$attempt++){
  try{
   # Copy bytes quickly before parsing, instead of a slow PowerShell content
   # pipeline racing the observer's next frame. This never writes to the app.
   $raw=[IO.File]::ReadAllText($Path,[Text.Encoding]::UTF8)
   if([string]::IsNullOrWhiteSpace($raw)){throw 'Observer is between frames'}
   return ($raw|ConvertFrom-Json)
  }catch{$lastFailure=$_.Exception.Message;Start-Sleep -Milliseconds 20}
 }
 throw "No complete observation: $Path ($lastFailure)"
}
function CurrentDocument([string]$Path){
 $until=[DateTime]::UtcNow.AddSeconds(20)
 do{if(PathEquals (UiState).document_path $Path){return};Start-Sleep -Milliseconds 80}while([DateTime]::UtcNow-lt$until)
 throw "Target did not become current: $Path"
}
function Settle([string]$Name){
 Start-Sleep -Milliseconds 650
 WaitReading {param($s)
  $v=$s.views.PSObject.Properties[$Name].Value
  if($null-ne$v-and$null-ne$v.failed_restore){
   throw "$Name explicitly downgraded: $($v.failed_restore.reason), passes=$($v.failed_restore.passes)"
  }
  $null-ne$v-and$null-ne$v.position-and$null-eq$v.pending-and$null-ne$v.viewport_rect-and
   [Math]::Abs($v.position.viewport[0]-($v.viewport_rect[2]-$v.viewport_rect[0]))-le2-and
   [Math]::Abs($v.position.viewport[1]-($v.viewport_rect[3]-$v.viewport_rect[1]))-le2
 } "$Name did not finish actual position restoration"|Out-Null
 Start-Sleep -Milliseconds 250
}
function Point([string]$Key,[string]$ExpectedSource){
 WaitReading {param($s)
  $v=$s.views.PSObject.Properties[$Key].Value
  $null-ne$v-and$null-eq$v.pending-and$null-ne$v.geometry-and
   $null-ne$v.viewport_rect-and$v.geometry.origin-eq'galley-current-pass'
 } "No current-pass text geometry for $Key" 25|Out-Null
 $v=View $Key;$g=$v.geometry
 Require ($g.precision-eq'text') "Target has lower precision: $($g.precision)"
 Require ($g.frame-gt0) 'Geometry has no current-frame identity'
 $buffers=JsonFile $report
 $isSecondary=$Key.StartsWith('secondary/',[StringComparison]::Ordinal)
 $source=if($isSecondary){$buffers.secondary.text}else{$buffers.text}
 $bytes=[Text.Encoding]::UTF8.GetBytes($source)
 $decodedDigest=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()
 if($PSBoundParameters.ContainsKey('ExpectedSource')){
  Require ($source-ceq$ExpectedSource) 'Unsaved source differs from the independent native-edit expectation'
 }else{
  $fixtureManifest=JsonFile (Join-Path $fixtures 'precision-fixtures.json')
  $currentPath=if($isSecondary){$buffers.secondary.path}else{(UiState).document_path}
  $original=@($fixtureManifest.files|Where-Object {PathEquals $_.path $currentPath})|Select-Object -First 1
  Require ($null-ne$original) 'Observed source does not belong to the independently generated fixture set'
  Require ($decodedDigest-ceq$original.decoded_utf8_sha256) 'Application decoded source differs from independent original text'
 }
 $start=[int64]$g.source_byte;$end=[int64]$g.source_end
 Require ($start-ge0-and$end-gt$start-and$end-le$bytes.Length) 'Observed original source range is invalid'
 $decoded=[Text.UTF8Encoding]::new($false,$true).GetString($bytes,[int]$start,[int]($end-$start))
 Require ($decoded-ceq$g.source_text) 'Geometry original source text disagrees with independently decoded manuscript'
 $beforeText=[Text.UTF8Encoding]::new($false,$true).GetString($bytes,0,[int]$start)
 if(!$script:precisionBoundaryCache.ContainsKey($decodedDigest)){
  $script:precisionBoundaryCache[$decodedDigest]=[Globalization.StringInfo]::ParseCombiningCharacters($source)
 }
 $precisionBoundaries=$script:precisionBoundaryCache[$decodedDigest]
 $utf16Start=$beforeText.Length;$utf16End=$utf16Start+$decoded.Length
 Require ([Array]::BinarySearch([int[]]$precisionBoundaries,[int]$utf16Start)-ge0) 'Observed original range starts inside a Unicode grapheme according to independent .NET segmentation'
 Require ($utf16End-eq$source.Length-or[Array]::BinarySearch([int[]]$precisionBoundaries,[int]$utf16End)-ge0) 'Observed original range ends inside a Unicode grapheme according to independent .NET segmentation'
 $expectedLogicalLine=[regex]::Matches($beforeText,"`n").Count
 Require ($g.logical_line-eq$expectedLogicalLine) 'Observed zero-based original logical line disagrees with independent source newline counting'
 Require ($g.grapheme_range.start-eq$start-and$g.grapheme_range.end-eq$end) 'Geometry is not a complete original grapheme range'
 Require ([Math]::Abs(($g.rect[1]-$v.viewport_rect[1])-$g.viewport_y)-le0.05) 'Viewport y is not derived from the actual observed rectangle'
 Require ($g.rect[2]-gt$v.viewport_rect[0]-and$g.rect[0]-lt$v.viewport_rect[2]-and
          $g.rect[3]-gt$v.viewport_rect[1]-and$g.rect[1]-lt$v.viewport_rect[3]) 'The precise target is outside the actual reading viewport'
 return $g
}
function SamePoint($Expected,[string]$Key,[string]$Name,[switch]$AllowClamp,[switch]$FreshProcess){
 Settle $Key;$actual=Point $Key
 Require (!$Expected.clamped) "$Name has a clamped baseline and requires a separate boundary experiment"
 if($actual.clamped){
  Require ($AllowClamp-and![string]::IsNullOrWhiteSpace($actual.clamp_reason)) "$Name is boundary-clamped and must be reported separately"
 }
 Require ($actual.source_byte-eq$Expected.source_byte-and$actual.source_end-eq$Expected.source_end) "$Name changed the original grapheme range"
 Require ($actual.source_text-ceq$Expected.source_text-and$actual.text-ceq$Expected.text) "$Name changed the target text"
 $dy=[Math]::Abs($actual.viewport_y-$Expected.viewport_y)
 if(!$actual.clamped){Require ($dy-le2) "$Name exceeded frozen 2 logical px: dy=$dy"}
 if(!$FreshProcess){Require ($actual.frame-gt$Expected.frame) "$Name reused stale geometry"}
 Start-Sleep -Milliseconds 2100
 $tail=Point $Key
 Require ($tail.source_byte-eq$Expected.source_byte) "$Name changed its target during the stable tail"
 $tailReference=if($actual.clamped){$actual.viewport_y}else{$Expected.viewport_y}
 Require ([Math]::Abs($tail.viewport_y-$tailReference)-le2) "$Name drifted outside precision tolerance during the stable tail"
 Require ($tail.clamped-eq$actual.clamped) "$Name changed its boundary classification during the stable tail"
 Require ($tail.compensations-eq$actual.compensations) "$Name kept automatically compensating after reaching tolerance"
 return @{expected=$Expected;actual=$actual;dy_logical_px=$dy;clamped=$actual.clamped;reason=$actual.clamp_reason;excluded_from_unclamped_precision=$actual.clamped;stable_tail_ms=2100;tail=$tail}
}
function OpenFixture([string]$Name){
 $path=Join-Path $fixtures $Name
 StartReadingApp $path;SetMode $Mode;Settle $key
 if($Mode-eq'Write'){
  # Large files default to no wrap. Exercise real wrapped source layout from
  # the first saved anchor, rather than calling an unchanged layout a reflow.
  UiClick 'source-wrap';UiClick 'source-wrap-on';Settle $key
 }
 return $path
}
function SourceSelection {
 $r=(View 'main/source').viewport_rect
 Click (($r[0]+$r[2])/2) (($r[1]+$r[3])/2)
 Keys '^{HOME}';Keys '+{END}'
 $selection=(UiState).selection
 Require ($selection[0]-ne$selection[1]) 'No source selection was established'
 return $selection
}
function CheckSelection($Expected){
 $actual=(UiState).selection
 Require ($actual[0]-eq$Expected[0]-and$actual[1]-eq$Expected[1]) 'Reading restoration changed source cursor/selection'
}
function FontChange {
 Keys '^+p';Start-Sleep -Milliseconds 300
 $slider=@((UiState).controls|Where-Object name -eq 'reading-font-size')|Select-Object -Last 1
 Require ($null-ne$slider) 'No actual font-size slider'
 $preferences=JsonFile $settings
 $fraction=if($preferences.font_size-ge19){0.16}else{0.56}
 Click ($slider.rect[0]+$fraction*($slider.rect[2]-$slider.rect[0])) (($slider.rect[1]+$slider.rect[3])/2)
 Keys '{ESC}';Settle $key
}
function NativeLargeWheel([string]$Key,[int]$Direction){
 $rect=(View $Key).viewport_rect;$bounds=Bounds
 $scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0
 Require ([MdcardUi]::GetForegroundWindow()-eq$script:app.MainWindowHandle) 'Native boundary wheel does not own foreground'
 [void][MdcardUi]::SetCursorPos($bounds.Left+[int]((($rect[0]+$rect[2])/2)*$scale),$bounds.Top+[int](($rect[1]+0.97*($rect[3]-$rect[1]))*$scale))
 # Keep each native WM_MOUSEWHEEL delta within its signed 16-bit field.
 $delta=if($Direction-gt0){[uint32]::MaxValue-23999}else{[uint32]24000}
 [MdcardUi]::mouse_event(0x800,0,0,$delta,[UIntPtr]::Zero)
 Settle $Key
}
function MappingWheel {
 $currentView=View $key;$rect=$currentView.viewport_rect;$bounds=Bounds
 $scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0
 Require ([MdcardUi]::GetForegroundWindow()-eq$script:app.MainWindowHandle) 'Mapping wheel does not own foreground'
 # A formula canvas legitimately consumes wheel input for zoom. Hit a real
 # currently painted text glyph instead; these coordinates do not set scroll.
 $word=@($currentView.visible_text|Where-Object {
  $_.rect[0]-ge$rect[0]-and$_.rect[2]-le$rect[2]-and$_.rect[1]-ge$rect[1]-and$_.rect[3]-le$rect[3]
 })|Select-Object -First 1
 Require ($null-ne$word) 'Mapping wheel needs a visible text hit target outside formula canvases'
 [void][MdcardUi]::SetCursorPos($bounds.Left+[int]((($word.rect[0]+$word.rect[2])/2)*$scale),$bounds.Top+[int]((($word.rect[1]+$word.rect[3])/2)*$scale))
 for($notch=0;$notch-lt6;$notch++){
  [MdcardUi]::mouse_event(0x800,0,0,([uint32]::MaxValue-119),[UIntPtr]::Zero)
  Start-Sleep -Milliseconds 55
 }
 Settle $key
}
if($OnlyPrecisionHelpers){return}

try{
 switch($Scenario){
  'reflow'{
   foreach($name in $Documents){
    $path=OpenFixture $name
    $selection=if($Mode-eq'Write'){SourceSelection}else{$null}
    WheelView $key 31
    if($TargetRegion-ne'Start'){
     $goalRatio=if($TargetRegion-eq'Middle'){0.45}else{0.82}
     $targetBytes=[Text.Encoding]::UTF8.GetByteCount((State).text)
     $regionSamples=@()
     for($attempt=0;$attempt-lt80;$attempt++){
      $candidate=Point $key
      $ratio=$candidate.source_byte/[double]$targetBytes
      $regionSamples+=@{source_byte=$candidate.source_byte;ratio=$ratio;offset=(Position $key).offset;geometry=$candidate}
      $record.long_region=@{region=$TargetRegion;observations=$regionSamples;actual_ratio=$ratio}
      if($ratio-ge$goalRatio-and$ratio-lt0.96){break}
      NativeLargeWheel $key $(if($ratio-ge0.96){-1}else{1})
     }
     Require ($ratio-ge$goalRatio-and$ratio-lt0.96) 'Native scrolling did not establish an interior target in the requested long-content region'
     $record.long_region=@{region=$TargetRegion;observations=$regionSamples;actual_ratio=$ratio}
    }
    $saved=Point $key;$startLayout=(Position $key).layout
    Require ((Position $key).offset[1]-gt500) 'Fixture did not reach a nontrivial reading position'
    Observe (($name-replace'\.','-')+'-before') -Screenshot|Out-Null
    foreach($size in @(@(1040,800),@(1550,1025),@(1135,880),@(1550,1025))){
     [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,50,50,$size[0],$size[1],$true)
     Pass "$name $Mode resize $($size-join'x')" (SamePoint $saved $key 'Repeated width reflow')
    }
    Keys '^b';Pass "$name sidebar toggle" (SamePoint $saved $key 'Sidebar hidden')
    Keys '^b';Pass "$name sidebar return" (SamePoint $saved $key 'Sidebar shown')
    FontChange
    Require ((Position $key).layout-cne$startLayout) 'Font input did not change the layout signature'
    Pass "$name font change" (SamePoint $saved $key 'Font-size reflow')
    SetMode 'Split';Pass "$name Split" (SamePoint $saved $key 'Split reflow')
    SetMode $Mode;Pass "$name return $Mode" (SamePoint $saved $key 'Return from split')
    if($Mode-eq'Write'){
     UiClick 'source-wrap';UiClick 'source-wrap-off';Pass "$name nowrap" (SamePoint $saved $key 'Disable wrapping' -AllowClamp)
     SetMode 'Split';Pass "$name nowrap Split" (SamePoint $saved $key 'Split without wrapping' -AllowClamp)
     SetMode 'Write';Pass "$name nowrap return Write" (SamePoint $saved $key 'Return without wrapping' -AllowClamp)
     UiClick 'source-wrap';UiClick 'source-wrap-on';Pass "$name wrap" (SamePoint $saved $key 'Enable wrapping')
     CheckSelection $selection
    }
    Observe (($name-replace'\.','-')+'-after') -Screenshot|Out-Null
    EndReadingApp
   }
  }
  'lifecycle'{
   foreach($name in $Documents){
    $path=OpenFixture $name
    $selection=if($Mode-eq'Write'){SourceSelection}else{$null}
    WheelView $key 28;$point=Point $key;$position=Position $key;$id=(Reading).main_id
    QuickOpen 'secondary.md' (Join-Path $fixtures 'secondary.md');SetMode $Mode;Settle $key;WheelView $key 13
    $second=Point $key;$secondPosition=Position $key
    QuickOpen $name $path;SetMode $Mode
    Pass "$name A-B-A precise text" (SamePoint $point $key 'File switching')
    Pass "$name A-B-A unchanged offsets" (SameOffset $position (Position $key) 'File switching offsets')
    if($selection){CheckSelection $selection}
    EndReadingApp
    StartReadingApp $path;SetMode $Mode;Settle $key
    # Pass counters restart with the process, so only compare real geometry and
    # original text here, not cross-process numeric frame ordering.
    $restart=Point $key
    Require ($restart.source_byte-eq$point.source_byte-and$restart.text-ceq$point.text) 'Restart changed the precise target'
    Require ([Math]::Abs($restart.viewport_y-$point.viewport_y)-le2) 'Restart exceeded text geometry tolerance'
    Require ([string](Reading).main_id-ceq[string]$id) 'Restart changed stable document identity'
    Pass "$name clean restart" @{point=$restart;offset=(SameOffset $position (Position $key) 'Restart offset')}
    if($selection){CheckSelection $selection}
    QuickOpen 'secondary.md' (Join-Path $fixtures 'secondary.md');SetMode $Mode;Settle $key
    Pass "$name second document restart" (SameOffset $secondPosition (Position $key) 'Second document restart')
    QuickOpen $name $path;SetMode $Mode;Settle $key;WheelView $key 7
    WaitReading {param($s)!$s.dirty-and$s.writes.last_success_ms-gt0} 'Last successful persistence'|Out-Null
    $persistedPoint=Point $key
    $disk=JsonFile $readingStore
    $persistedRecord=@($disk.records.PSObject.Properties|ForEach-Object Value|Where-Object {[string]$_.id-ceq[string](Reading).main_id})|Select-Object -First 1
    Require ($null-ne$persistedRecord) 'No successful disk record for the current document'
    $persisted=$persistedRecord.views.PSObject.Properties[$key].Value
    Require ($null-ne$persisted) 'No successful disk record for the current view'
    [void](SameOffset $persisted (Position $key) 'Disk state before abnormal exit')
    $at=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    $age=$at-[int64]$persisted.updated_ms;$writeAge=$at-[int64](Reading).writes.last_success_ms
    $script:app.Kill();$script:app.WaitForExit()
    StartReadingApp $path;SetMode $Mode;Settle $key
    $crash=Point $key
    Require ($crash.source_byte-eq$persistedPoint.source_byte) 'Abnormal restart changed last persisted precise anchor'
    Require ([Math]::Abs($crash.viewport_y-$persistedPoint.viewport_y)-le2) 'Abnormal restart changed target geometry'
    Pass "$name abnormal exit" @{record_age_ms=$age;successful_write_age_ms=$writeAge;point=$crash;offset=(SameOffset $persisted (Position $key) 'Abnormal restart offsets')}
    Observe (($name-replace'\.','-')+'-abnormal-restart') -Screenshot|Out-Null
    EndReadingApp
   }
  }
  'mapping'{
   Require ($Mode-eq'Read') 'Preview mapping is a Read-mode scenario'
   $mappingName=if($Documents-contains'mapping-large.md'){'mapping-large.md'}else{'mapping.md'}
   $path=OpenFixture $mappingName
   $golden=JsonFile (Join-Path $fixtures ($mappingName-replace'\.md$','-golden.json'))
   Require ((Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()-ceq$golden.source_sha256) 'Independent mapping landmarks belong to a different original manuscript'
   $bytes=[IO.File]::ReadAllBytes($path)
   $mappingOriginal=[Text.UTF8Encoding]::new($false,$true).GetString($bytes)
   $mappingBoundaries=[Globalization.StringInfo]::ParseCombiningCharacters($mappingOriginal)
   $unicodeMapped=@{}
   $seen=@{};$seenAfterConversion=@{};$mappingSamples=@();$turn=0
   $afterConversionByte=$golden.landmarks[10].source_start
   do{
    Settle $key
    $view=View $key
    foreach($g in @($view.visible_text)){
     Require ($g.origin-eq'galley-current-pass') 'Mapping observation is not a real current-pass Galley'
     $landmark=@($golden.landmarks|Where-Object {$g.source_byte-ge$_.source_start-and$g.source_end-le$_.source_end})|Select-Object -First 1
     if(!$landmark){continue}
     $decoded=[Text.UTF8Encoding]::new($false,$true).GetString($bytes,[int]$g.source_byte,[int]($g.source_end-$g.source_byte))
     Require ($decoded-ceq$g.source_text-and$decoded-ceq$g.text) 'Actual rendered landmark does not map to its independent original-source grapheme'
     $mappingUtf16Start=([Text.UTF8Encoding]::new($false,$true).GetString($bytes,0,[int]$g.source_byte)).Length
     $mappingUtf16End=$mappingUtf16Start+$decoded.Length
     Require ([Array]::BinarySearch([int[]]$mappingBoundaries,[int]$mappingUtf16Start)-ge0) 'A rendered landmark starts inside an independently segmented original grapheme'
     Require ($mappingUtf16End-eq$mappingOriginal.Length-or[Array]::BinarySearch([int[]]$mappingBoundaries,[int]$mappingUtf16End)-ge0) 'A rendered landmark ends inside an independently segmented original grapheme'
     if($decoded-in@('🙂','é','👩🏽‍💻','👍🏿')){$unicodeMapped[$decoded]=$true}
     Require ($g.source_byte-eq$g.grapheme_range.start-and$g.source_end-eq$g.grapheme_range.end) 'Mapped landmark split an original grapheme'
     Require ($g.rect[2]-gt$view.viewport_rect[0]-and$g.rect[0]-lt$view.viewport_rect[2]-and
              $g.rect[3]-gt$view.viewport_rect[1]-and$g.rect[1]-lt$view.viewport_rect[3]) 'Reported mapping landmark is not visible'
     $seen[$landmark.kind]=$true
     if($landmark.source_start-ge$afterConversionByte){$seenAfterConversion[$landmark.kind]=$true}
     $mappingSamples+=@{kind=$landmark.kind;landmark=$landmark;actual=$g}
    }
    $record.mapping_observations=$mappingSamples
    $record.mapping_kinds=@($seen.Keys)
    $record.mapping_after_conversion_kinds=@($seenAfterConversion.Keys)
    $record.mapping_unicode_clusters=@($unicodeMapped.Keys)
    if(($turn-eq0)-or($seen.Count-eq10-and$seenAfterConversion.Count-eq10)){Observe "mapping-$turn" -Screenshot|Out-Null}
    if($seen.Count-eq10-and$seenAfterConversion.Count-eq10){break}
    $beforeWheel=(Position $key).offset[1]
    MappingWheel;$turn++
    Require ((Position $key).offset[1]-gt$beforeWheel+2) 'Native mapping wheel did not advance the viewport before full coverage'
   }while($turn-lt45)
   Require ($seen.Count-eq10) "Missing preview landmark categories: only $($seen.Keys-join',')"
   Require ($seenAfterConversion.Count-eq10) 'Did not verify every category after converted formula nodes changed preview offsets'
   Require ($unicodeMapped.Count-eq4) 'The actual GUI mapping observations did not include every expected emoji/combining/skin/ZWJ grapheme'
   Pass 'Actual preview glyphs map to independent original UTF8 landmarks, including after generated nodes' @{kinds=$seen.Keys;after_conversion_kinds=$seenAfterConversion.Keys;observations=$mappingSamples;physical_scroll_groups=$turn}
   EndReadingApp
  }
  'boundaries'{
   Require ($Mode-eq'Read') 'This boundary matrix exercises the reading viewport'
   [void](OpenFixture 'empty.md')
   Require ((Position $key).offset[0]-eq0-and(Position $key).offset[1]-eq0) 'An empty manuscript has a nonzero reading offset'
   Require ($null-eq(View $key).geometry-and$null-eq(Position $key).text_anchor) 'Empty manuscript invented a visible text anchor'
   Pass 'Empty manuscript has no visible word and safely starts at top' @{position=(Position $key)}
   Observe 'empty-safe' -Screenshot|Out-Null
   EndReadingApp

   $nontext=OpenFixture 'nontext.md'
   Start-Sleep -Milliseconds 1500
   WheelView $key 40
   Require ((Position $key).offset[1]-gt1000) 'The real local PNG did not create a tall, scrollable viewport'
   Require (@((View $key).visible_text).Count-eq0) 'High-image viewport still contains mapped text; fixture did not reach the intended case'
   Require ($null-eq(View $key).geometry-and$null-eq(Position $key).text_anchor) 'Scrolling into a nontext viewport resurrected the old offscreen text anchor'
   $saved=Position $key
   Observe 'nontext-block-fallback' -Screenshot|Out-Null
   QuickOpen 'secondary.md' (Join-Path $fixtures 'secondary.md');SetMode 'Read';Settle $key
   QuickOpen 'nontext.md' $nontext;SetMode 'Read';Settle $key
   Require ($null-eq(Position $key).text_anchor) 'Block-only history was silently promoted from an old offscreen text anchor'
   Pass 'A fully nontext viewport persists and restores explicit block fallback' @{position=(Position $key);offset=(SameOffset $saved (Position $key) 'Image-only history')}
   EndReadingApp
   StartReadingApp $nontext;SetMode 'Read';Settle $key
   Require ($null-eq(Position $key).text_anchor) 'Image-only restart resurrected a stale text pin'
   Pass 'Nontext fallback survives restart' (SameOffset $saved (Position $key) 'Image-only restart')
   EndReadingApp

   [void](OpenFixture 'precision.md')
   Require ((Position $key).offset[1]-le2) 'A new document did not begin at top'
   [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,50,50,775,1025,$true)
   Settle $key
   $endSamples=@();$lastEnd=-1.0;$stableEnd=0
   for($attempt=0;$attempt-lt12-and$stableEnd-lt2;$attempt++){
    NativeLargeWheel $key 1
    $currentEnd=(Position $key).offset[1]
    $endSamples+=@{offset=$currentEnd;observed=[DateTimeOffset]::UtcNow.ToString('o')}
    if($lastEnd-ge0-and[Math]::Abs($currentEnd-$lastEnd)-le2){$stableEnd++}else{$stableEnd=0}
    $lastEnd=$currentEnd
   }
   $record.native_end_observations=$endSamples
   Require ($stableEnd-ge2) 'Repeated real wheel gestures did not converge at the actual document end'
   Require (@((View $key).visible_runs|Where-Object {$_.source_text-match'source-00089'}).Count-gt0) 'The independently known final paragraph is not actually visible at the measured end'
   $narrow=Point $key
   [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,50,50,1550,1025,$true)
   Settle $key;$wide=Point $key
   Require ($wide.source_byte-eq$narrow.source_byte-and$wide.source_text-ceq$narrow.source_text) 'Boundary reflow lost original text identity'
   $boundaryDelta=[Math]::Abs($wide.viewport_y-$narrow.viewport_y)
   if($wide.clamped){
    Require (![string]::IsNullOrWhiteSpace($wide.clamp_reason)) 'Boundary clamp has no inspectable reason'
   }else{Require ($boundaryDelta-le2) 'Unclamped document-end target exceeded text precision tolerance'}
   Pass 'Document-end reflow reports measured clamp separately from ordinary exact samples' @{boundary_clamped=$wide.clamped;reason=$wide.clamp_reason;dy_logical_px=$boundaryDelta;before=$narrow;after=$wide;excluded_from_unclamped_precision=$wide.clamped}
   Observe 'document-end-clamp' -Screenshot|Out-Null
   EndReadingApp
  }
 }
 $golden=JsonFile (Join-Path $fixtures 'precision-fixtures.json')
 foreach($fixture in $golden.files){
  Require ((Get-FileHash -LiteralPath $fixture.path).Hash.ToLowerInvariant()-ceq$fixture.sha256) "Manuscript changed: $($fixture.relative)"
 }
 $record.manuscript_bytes_unchanged=$true;$record.passed=$true
}catch{
 $record.error=$_.Exception.Message;$record.error_stack=$_.ScriptStackTrace;$record.passed=$false
 if($script:app){$record.process_before_cleanup=@{pid=$script:app.Id;exited=$script:app.HasExited};if($script:app.HasExited){$record.process_before_cleanup.exit_code=$script:app.ExitCode}}
 if($script:app-and!$script:app.HasExited){try{Shot 'failure'}catch{}}
}finally{
 if($script:app-and!$script:app.HasExited){
  if($script:previousInputLayout){[void][ReadingInputNative]::PostMessage($script:app.MainWindowHandle,0x50,[IntPtr]::Zero,$script:previousInputLayout)}
  $script:app.Kill();$script:app.WaitForExit()
 }
 FlushReadingProcessLogs
 $env:MDCARD_READING_GEOMETRY_REPORT=$oldGeometryEnv
 if($script:readingClipboardModified-and$priorClipboard){[WorkbenchClipboard]::SetDataObject($priorClipboard,$true)}
 $record.finished=[DateTimeOffset]::UtcNow.ToString('o')
 $record|ConvertTo-Json -Depth 45|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
}
$record|Select-Object verification_id,stage,passed,error,@{n='assertions';e={$_.cases.Count}}|ConvertTo-Json
if(!$record.passed){throw 'Independent text-precision GUI acceptance failed; inspect result.json'}
