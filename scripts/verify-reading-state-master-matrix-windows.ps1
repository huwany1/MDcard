# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
 [Parameter(Mandatory=$true)][string]$Exe,
 [Parameter(Mandatory=$true)][ValidateSet('routes','formats','clear','rename','failure','crash','secondary','atomic','content','corrupt','guards','source','reflow','semantics','priorities','capacity','same-file')][string]$Scenario,
 [string]$Stage='master-r3',
 [string[]]$Documents=@('large.md','plain.txt','large-plain.txt','utf16.txt','gb18030.txt','mapping.md','mapping-large.md','long-paragraph.md')
)
$caseStage="$Stage-$Scenario"
. "$PSScriptRoot\verify-reading-state-master-windows.ps1" -Exe $Exe -Stage $caseStage -OnlyReadingHelpers
$record.matrix_harness_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant()
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

function CurrentDocument([string]$Path){
 $until=[DateTime]::UtcNow.AddSeconds(12)
 do{if(PathEquals (UiState).document_path $Path){return};Start-Sleep -Milliseconds 80}while([DateTime]::UtcNow-lt$until)
 throw "Target document did not become current: $Path"
}
function PreferencesBottom {
 Keys '^+p';Start-Sleep -Milliseconds 350
 # This wheel targets the actual native modal's center; no report is mutated.
 $content=(UiState).content_rect
 Wheel @([float]($content[2]/2-150),[float]($content[3]/2-100),[float]($content[2]/2+150),[float]($content[3]/2+100)) 18
}
function NativeSaveAs([string]$Path){
 Keys '^+s';Start-Sleep -Milliseconds 650
 $handle=[MdcardUi]::GetForegroundWindow()
 $owner=[Windows.Automation.AutomationElement]::FromHandle($handle)
 Require ($owner.Current.ProcessId-eq$script:app.Id-and$owner.Current.ClassName-eq'#32770') 'No owned native Save As chooser'
 EnglishInputFor $handle|Out-Null
 Keys '%n';Keys '^a';Keys $Path;Keys '{ENTER}'
 CurrentDocument $Path
}
function FinishMatrix {
 if($script:app-and!$script:app.HasExited){EndReadingApp}
 $golden=JsonFile (Join-Path $fixtures 'fixtures.json')
 foreach($fixture in $golden.files){
  if($record.allowed_renamed_source-and(PathEquals $fixture.path $record.allowed_renamed_source)){continue}
  $expectedHash=$fixture.sha256
  if($record.fixture_edits-and$record.fixture_edits.ContainsKey($fixture.relative)){$expectedHash=$record.fixture_edits[$fixture.relative]}
  Require ((Get-FileHash -LiteralPath $fixture.path).Hash.ToLowerInvariant()-ceq$expectedHash) "Manuscript bytes changed: $($fixture.relative)"
 }
 $record.manuscript_bytes_unchanged=$true
 $record.passed=$true
}

function SameReadingContent($ExpectedState,$ActualState,[string]$Name){
 $before=$ExpectedState.views.PSObject.Properties['main/preview'].Value
 $after=$ActualState.views.PSObject.Properties['main/preview'].Value
 $currentDocumentPath=(UiState).document_path
 $extension=[IO.Path]::GetExtension($currentDocumentPath).ToLowerInvariant()
 if($before.position.anchor.byte-eq$after.position.anchor.byte-or$extension-notin@('.md','.markdown','.mdown','.mkd','.mkdn')){
  return SameBlock $before.position $after.position $Name
 }
 # The first intersecting block may change at a subpixel boundary while the
 # same actual text stays within the frozen two-pixel tolerance. Verify that
 # stronger contract against the fixture bytes and both actual Galley rects.
 Require ($before.document_id-ceq$after.document_id-and$before.buffer_id-eq$after.buffer_id-and$before.revision-eq$after.revision) "$Name changed document, buffer or revision"
 $old=$before.geometry;$new=$after.geometry;$anchor=$before.position.text_anchor
 Require ($null-ne$old-and$null-ne$new-and$null-ne$anchor) "$Name changed blocks without actual text geometry"
 Require (!$old.clamped-and!$new.clamped-and$old.origin-ceq'galley-current-pass'-and$new.origin-ceq'galley-current-pass') "$Name requires unclamped actual layout observations"
 $range=$anchor.source.grapheme_range
 foreach($point in @($old,$new)){
  Require ($point.grapheme_range.start-eq$range.start-and$point.grapheme_range.end-eq$range.end-and$point.source_byte-eq$range.start-and$point.source_end-eq$range.end) "$Name did not retain the original text range"
 }
 $golden=JsonFile (Join-Path $fixtures 'fixtures.json')
 $fixture=@($golden.files|Where-Object {PathEquals $_.path $currentDocumentPath})
 Require ($fixture.Count-eq1) "$Name has no independent fixture definition"
 Require ((Get-FileHash -LiteralPath $fixture[0].path).Hash.ToLowerInvariant()-ceq$fixture[0].sha256) "$Name changed manuscript bytes"
 $reader=[IO.StreamReader]::new($fixture[0].path,[Text.Encoding]::GetEncoding($fixture[0].encoding),$true)
 try{$source=$reader.ReadToEnd()}finally{$reader.Dispose()}
 $utf8=[Text.UTF8Encoding]::new($false,$true);$bytes=$utf8.GetBytes($source)
 Require ($range.start-ge0-and$range.end-gt$range.start-and$range.end-le$bytes.Length) "$Name has invalid original text coordinates"
 $prefix=$utf8.GetString($bytes,0,[int]$range.start)
 $text=$utf8.GetString($bytes,[int]$range.start,[int]($range.end-$range.start))
 $boundaries=[Globalization.StringInfo]::ParseCombiningCharacters($source)
 $index=[Array]::BinarySearch($boundaries,[int]$prefix.Length)
 Require ($index-ge0) "$Name starts inside an original grapheme"
 $end=if($index+1-lt$boundaries.Length){$boundaries[$index+1]}else{$source.Length}
 Require ($end-eq$prefix.Length+$text.Length-and$text-ceq$anchor.source.grapheme-and$text-ceq$old.source_text-and$text-ceq$new.source_text) "$Name did not paint the complete original grapheme"
 $oldY=[double]$old.rect[1]-[double]$before.viewport_rect[1]
 $newY=[double]$new.rect[1]-[double]$after.viewport_rect[1]
 Require ([Math]::Abs($oldY-[double]$old.viewport_y)-le0.01-and[Math]::Abs($newY-[double]$new.viewport_y)-le0.01) "$Name has inconsistent actual viewport coordinates"
 $actualError=[Math]::Abs($newY-$oldY);$savedError=[Math]::Abs($newY-[double]$anchor.viewport_y)
 Require ($actualError-le2-and$savedError-le2) "$Name exceeded actual text tolerance: observed=$actualError saved=$savedError"
 return @{precision='original grapheme and actual layout';source_range=$range;text=$text;source_sha256=$fixture[0].sha256;before_viewport_y=$oldY;after_viewport_y=$newY;actual_error=$actualError;saved_error=$savedError;first_visible_block_before=$before.position.anchor.byte;first_visible_block_after=$after.position.anchor.byte;block_observation='first intersecting block changed; original text identity and actual coordinates verified'}
}

try{
 $a=Join-Path $fixtures 'a.md';$b=Join-Path $fixtures 'b.md'
 switch($Scenario){
  'routes'{
   # Process WorkingDirectory is the fixture root: this is a real relative-path
   # application launch, followed below by an absolute native chooser open.
   StartReadingApp 'a.md';SetMode 'Read';Settle 'main/preview'
   WheelView 'main/preview' 45;$savedA=Position 'main/preview';$idA=(Reading).main_id
   UiClick 'navigation:Files';UiClick 'tree:b.md';CurrentDocument $b
   SetMode 'Read';Settle 'main/preview';WheelView 'main/preview' 17;$savedB=Position 'main/preview'
   UiClick 'navigation:Recent';UiClick 'recent:a.md';CurrentDocument $a;SetMode 'Read';Settle 'main/preview'
   Pass 'File tree and Recent share file-state restoration' (SameOffset $savedA (Position 'main/preview') 'Recent A')
   Observe '01-recent-a' -Screenshot|Out-Null
   $sameRoot=Join-Path $fixtures 'same.md';$sameNested=Join-Path $fixtures 'nested\same.md'
   NativeOpen $sameRoot;CurrentDocument $sameRoot;SetMode 'Read';Settle 'main/preview'
   $rootId=(Reading).main_id;WheelView 'main/preview' 20;$rootPos=Position 'main/preview'
   NativeOpen $sameNested;CurrentDocument $sameNested;SetMode 'Read';Settle 'main/preview'
   $nestedId=(Reading).main_id
   Require ([string]$rootId-cne[string]$nestedId) 'Different directories with same filename share an ID'
   Require ((Position 'main/preview').offset[1]-le2) 'Nested same.md inherited the root same.md scroll'
   WheelView 'main/preview' 8;$nestedPos=Position 'main/preview'
   NativeOpen $sameRoot;CurrentDocument $sameRoot;SetMode 'Read';Settle 'main/preview'
   Pass 'Same basename in different directories stays independent' (SameOffset $rootPos (Position 'main/preview') 'Root same.md')
   NativeOpen $sameNested;CurrentDocument $sameNested;SetMode 'Read';Settle 'main/preview'
   Pass 'Nested same-name position restores separately' (SameOffset $nestedPos (Position 'main/preview') 'Nested same.md')
   Observe '02-nested-same-name' -Screenshot|Out-Null
   NativeOpen $b;CurrentDocument $b;SetMode 'Read';Settle 'main/preview'
   Pass 'Open chooser preserves previous tree-opened B history' (SameOffset $savedB (Position 'main/preview') 'Tree B via chooser')
   NativeOpen $a;CurrentDocument $a;SetMode 'Read';Settle 'main/preview'
   Require ([string](Reading).main_id-ceq[string]$idA) 'Relative startup and absolute chooser changed the stable file identity'
   Pass 'Relative startup and absolute native chooser share identity and history' (SameOffset $savedA (Position 'main/preview') 'Relative to absolute A')
   NativeOpen $b;CurrentDocument $b;SetMode 'Read';Settle 'main/preview'
   Keys '^+f';Fill 'find-query' 'BLOCK-A-0001';Fill 'include' '**/*.md'
   Await {param($s)!$s.query_pending-and$s.results-eq1} 'Unique search result must be ready'|Out-Null
   Control 'result-0-0';CurrentDocument $a;SetMode 'Write';Settle 'main/source'
   $source=Position 'main/source';$selection=(UiState).selection
   $text=(State).text;$start=[Math]::Min($selection[0],$selection[1]);$length=[Math]::Abs($selection[0]-$selection[1])
   # ASCII marker occurs after CJK/emoji. Convert using .NET Rune enumeration
   # rather than treating UTF-16 code units as egui Unicode scalar positions.
   $scalars=@($text.EnumerateRunes()|ForEach-Object {$_.ToString()})
   $selected=if($length-gt0){($scalars[$start..($start+$length-1)]-join'')}else{''}
   Require ($selected-ceq'BLOCK-A-0001') "Search selected wrong source text: $selected"
   Require ($source.offset[1]-lt500) 'Search hit lost priority to far history'
   Start-Sleep -Milliseconds 1700
   Require ((Position 'main/source').offset[1]-lt500) 'Late history restoration pulled away from the search hit'
   Pass 'Search open wins over history and selects original Unicode-aware source' @{selection=$selection;source_position=$source}
   Observe '03-search-priority' -Screenshot|Out-Null
   UiClick 'navigation:Outline';UiClick 'outline:0';Settle 'main/source'
   Require ((Position 'main/source').offset[1]-le2) 'Outline top heading lost navigation priority'
   Pass 'Outline navigation remains at the requested heading' (Position 'main/source')
  }
  'formats'{
   foreach($name in $Documents){
    $document=Join-Path $fixtures $name
    StartReadingApp $document;SetMode 'Read';Settle 'main/preview'
    WheelView 'main/preview' 26;$wideState=Observe "$name-wide" -Screenshot;$wide=$wideState.views.PSObject.Properties['main/preview'].Value.position
    Require ($wide.offset[1]-gt100) "$name did not scroll into content"
    [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,70,70,1040,800,$true);Settle 'main/preview'
    $narrowState=Observe "$name-narrow" -Screenshot
    $block=SameReadingContent $wideState $narrowState "$name width"
    [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,50,50,1550,1025,$true);Settle 'main/preview'
    $back=SameReadingContent $wideState (Observe "$name-back" -Screenshot) "$name width return"
    SetMode 'Split';Settle 'main/preview'
    $split=SameReadingContent $wideState (Observe "$name-split" -Screenshot) "$name Split"
    SetMode 'Read';Settle 'main/preview';$restart=Position 'main/preview'
    Observe ($name-replace'\.','-') -Screenshot|Out-Null
    EndReadingApp
    StartReadingApp $document;SetMode 'Read';Settle 'main/preview'
    $offset=SameOffset $restart (Position 'main/preview') "$name restart"
    $restoredBlock=SameBlock $restart (Position 'main/preview') "$name restart block"
    Pass "$name width, split and clean restart" @{width=$block;width_return=$back;split=$split;restart=$offset;restart_block=$restoredBlock}
    EndReadingApp
   }
  }
  'clear'{
   StartReadingApp $a;SetMode 'Read';Settle 'main/preview';WheelView 'main/preview' 33
   $idA=(Reading).main_id
   QuickOpen 'b.md' $b;SetMode 'Read';Settle 'main/preview';WheelView 'main/preview' 18;$savedB=Position 'main/preview'
   QuickOpen 'a.md' $a;SetMode 'Read';Settle 'main/preview'
   PreferencesBottom;UiClick "Clear this document's reading positions";Start-Sleep -Milliseconds 1500
   Require (@((RecordFor $idA).views.PSObject.Properties).Count-eq0) 'Clear current document was repopulated while idle'
   Observe '01-cleared-single-in-settings' -Screenshot|Out-Null
   Keys '{ESC}';QuickOpen 'b.md' $b;SetMode 'Read';Settle 'main/preview'
   Pass 'Single clear retains B' (SameOffset $savedB (Position 'main/preview') 'B after clearing A')
   QuickOpen 'a.md' $a;SetMode 'Read';Settle 'main/preview'
   Require ((Position 'main/preview').offset[1]-le2) 'Cleared A did not reopen at the top'
   Pass 'Single clear survives idle frames and reopening' (Position 'main/preview')
   PreferencesBottom;UiClick 'Clear all reading positions';Start-Sleep -Milliseconds 1200;Keys '{ESC}'
   EndReadingApp
   StartReadingApp $b;SetMode 'Read';Settle 'main/preview'
   Require ((Position 'main/preview').offset[1]-le2) 'Clear-all history was resurrected after restart'
   Pass 'Clear all survives restart' (Position 'main/preview')
  }
  'rename'{
   StartReadingApp $a;SetMode 'Read';Settle 'main/preview';WheelView 'main/preview' 33
   $oldId=(Reading).main_id;$originalPos=Position 'main/preview'
   $copy=Join-Path $fixtures 'copied.md';NativeSaveAs $copy;Settle 'main/preview'
   $newId=(Reading).main_id
   Require ([string]$oldId-cne[string]$newId) 'Save As did not assign the copied document its own ID'
   Pass 'Native Save As inherits current position' (SameOffset $originalPos (Position 'main/preview') 'Save As position')
   Require (Test-Path -LiteralPath $a) 'Save As removed original file'
   NativeOpen $a;CurrentDocument $a;SetMode 'Read';Settle 'main/preview'
   Pass 'Save As preserves original history' (SameOffset $originalPos (Position 'main/preview') 'Original after Save As')
   # The native egui menu button predates control-rectangle diagnostics.
   Click 169 62;UiClick 'Rename document…'
   $input=@((UiState).controls|Where-Object name -eq 'rename-filename')|Select-Object -Last 1
   Require ($null-ne$input) 'No visible rename filename input'
   Click (($input.rect[0]+$input.rect[2])/2) (($input.rect[1]+$input.rect[3])/2)
   Keys '^a';Keys 'renamed.md';UiClick 'Rename'
   $renamed=Join-Path $fixtures 'renamed.md';CurrentDocument $renamed
   Require ([string](Reading).main_id-ceq[string]$oldId) 'Internal rename changed stable document ID'
   Require (!(Test-Path -LiteralPath $a)-and(Test-Path -LiteralPath $renamed)) 'Internal rename file paths are inconsistent'
   $record.allowed_renamed_source=$a
   $golden=JsonFile (Join-Path $fixtures 'fixtures.json')
   $originalHash=($golden.files|Where-Object relative -eq 'a.md').sha256
   Require ((Get-FileHash -LiteralPath $renamed).Hash.ToLowerInvariant()-ceq$originalHash) 'Rename changed original bytes'
   EndReadingApp;StartReadingApp $renamed;SetMode 'Read';Settle 'main/preview'
   Require ([string](Reading).main_id-ceq[string]$oldId) 'Renamed identity did not survive restart'
   Pass 'Internal rename migrates persistent history and exact source bytes' (SameOffset $originalPos (Position 'main/preview') 'Renamed restart')
   Observe '01-renamed-restarted' -Screenshot|Out-Null
  }
  'failure'{
   $obstruction=Join-Path $scratch 'not-a-directory'
   [IO.File]::WriteAllText($obstruction,'BLOCKER')
   $readingStore=Join-Path $obstruction 'reading-state.json'
   StartReadingApp $a;SetMode 'Read';Settle 'main/preview';WheelView 'main/preview' 30;$savedA=Position 'main/preview'
   WaitReading {param($s)$null-ne$s.writes.error} 'Failed write did not report an inspectable error'|Out-Null
   QuickOpen 'b.md' $b;QuickOpen 'a.md' $a;SetMode 'Read';Settle 'main/preview'
   Pass 'Failed persistence retains independent memory state' (SameOffset $savedA (Position 'main/preview') 'Memory after failed disk write')
   PreferencesBottom;Observe '01-reading-write-failure-in-settings' -Screenshot|Out-Null
   Require ((Reading).writes.error.Length-gt0) 'Write failure disappeared without a successful write'
   Keys '{ESC}'
   Pass 'Writing to an invalid parent is visible and manuscripts are preserved' @{error=(Reading).writes.error;failed_path=$readingStore}
  }
  'crash'{
   StartReadingApp $a;SetMode 'Read';Settle 'main/preview';WheelView 'main/preview' 36
   WaitReading {param($s)$s.writes.last_success_ms-gt0-and!$s.dirty} 'No successful persistent record before abnormal exit'|Out-Null
   $persisted=JsonFile $readingStore;$reading=Reading
   $key=(RecordFor $reading.main_id).path
   $saved=$persisted.records.PSObject.Properties[$key].Value.views.'main/preview'
   $lastSuccess=$reading.writes.last_success_ms
   Observe '01-before-abnormal-exit' -Screenshot|Out-Null
    $killRequested=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    $age=$killRequested-[long]$saved.updated_ms
    $persistAge=$killRequested-[long]$lastSuccess
   $script:app.Kill();$script:app.WaitForExit()
   StartReadingApp $a;SetMode 'Read';Settle 'main/preview'
    Pass 'Abnormal exit restores latest successful disk record' @{record_age_ms=$age;persist_completed_age_ms=$persistAge;kill_request_ms=$killRequested;record_updated_ms=$saved.updated_ms;age_boundary='Persisted position timestamp to native termination request; successful-write completion age is separate';offset=(SameOffset $saved (Position 'main/preview') 'Abnormal exit');block=(SameBlock $saved (Position 'main/preview') 'Abnormal exit block')}
   Observe '02-after-abnormal-restart' -Screenshot|Out-Null
  }
  'secondary'{
   StartReadingApp $a;SetMode 'Read';Settle 'main/preview';WheelView 'main/preview' 36;$savedA=Position 'main/preview'
   QuickOpen 'b.md' $b -Secondary
   Settle 'secondary/source';WheelView 'secondary/source' 24;$savedB=Position 'secondary/source'
   $idB=(Reading).secondary_id
   Observe '01-main-read-and-secondary-source' -Screenshot|Out-Null
   QuickOpen 'same.md' (Join-Path $fixtures 'same.md') -Secondary
   Settle 'secondary/source'
   Require ((Position 'secondary/source').offset[1]-le2) 'New secondary document inherited B position'
   QuickOpen 'b.md' $b -Secondary;Settle 'secondary/source'
   Require ([string](Reading).secondary_id-ceq[string]$idB) 'Secondary ordinary reopen changed DocumentId'
   Pass 'Secondary file positions restore independently' (SameOffset $savedB (Position 'secondary/source') 'Secondary B')
   # Opening the side panel changes main width, so the main comparison is at block precision.
   Pass 'Secondary pane does not overwrite main reading anchor' (SameBlock $savedA (Position 'main/preview') 'Main with secondary')
   Observe '02-secondary-b-restored' -Screenshot|Out-Null
  }
  'atomic'{
   StartReadingApp $a;SetMode 'Read';Settle 'main/preview';WheelView 'main/preview' 24
   WaitReading {param($s)$s.writes.last_success_ms-gt0-and!$s.dirty} 'Initial successful reading-state write'|Out-Null
   Start-Sleep -Milliseconds 400
   $previousHash=(Get-FileHash -LiteralPath $readingStore).Hash
   $previousWrites=(Reading).writes.writes
   # An actual Windows handle denies replacement of the existing valid record.
   $fileLock=[IO.File]::Open($readingStore,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
   WheelView 'main/preview' 15;$newMemory=Position 'main/preview'
   WaitReading {param($s)$null-ne$s.writes.error} 'Atomic replacement failure was not reported'|Out-Null
   Require ((Get-FileHash -LiteralPath $readingStore).Hash-ceq$previousHash) 'Failed atomic replacement damaged the previous valid record'
   Pass 'Real Windows replacement denial preserves the previous valid state' @{previous_sha256=$previousHash;error=(Reading).writes.error;memory=$newMemory}
   Observe '01-atomic-replacement-denied' -Screenshot|Out-Null
   $fileLock.Dispose();$fileLock=$null
   WaitReading {param($s)$s.writes.writes-gt$previousWrites-and$null-eq$s.writes.error-and!$s.dirty} 'Retained memory was not persisted after the obstruction was removed'|Out-Null
   EndReadingApp;StartReadingApp $a;SetMode 'Read';Settle 'main/preview'
   Pass 'Write retry recovers the retained latest memory state' (SameOffset $newMemory (Position 'main/preview') 'Retry restart')
  }
  'content'{
   StartReadingApp $a;SetMode 'Read';Settle 'main/preview';WheelView 'main/preview' 35
   $old=Position 'main/preview';EndReadingApp
   $original=[IO.File]::ReadAllText($a)
   $prefix="# Inserted intro 中文🙂`n`nNew material before the old document.`n`n"
   [IO.File]::WriteAllText($a,$prefix+$original,[Text.UTF8Encoding]::new($false))
   $record.fixture_edits=@{'a.md'=(Get-FileHash -LiteralPath $a).Hash.ToLowerInvariant()}
   StartReadingApp $a;SetMode 'Read';Settle 'main/preview'
   $after=Position 'main/preview';$expectedByte=$old.anchor.byte+[Text.Encoding]::UTF8.GetByteCount($prefix)
   Require ($after.anchor.byte-eq$expectedByte) "Inserted prefix did not relocate the same original block: expected $expectedByte got $($after.anchor.byte)"
   Pass 'Changed content relocates a unique original paragraph by context' @{old=$old;new=$after;tier=(View 'main/preview').reason;expected_source_byte=$expectedByte}
   Observe '01-insertion-context-relocation' -Screenshot|Out-Null
   EndReadingApp
   [IO.File]::WriteAllText($a,"# Short replacement`n`n原锚点已删除🙂。`n",[Text.UTF8Encoding]::new($false))
   $record.fixture_edits['a.md']=(Get-FileHash -LiteralPath $a).Hash.ToLowerInvariant()
   StartReadingApp $a;SetMode 'Read';Settle 'main/preview'
   $fallback=Position 'main/preview';$view=View 'main/preview';$tier=$view.reason
   if($view.relocation){
    # Current precision reports distinguish the grade from its human-readable
    # reason. Verify the grade and lack of exactness, not the old reason label.
    Require (!$view.relocation.exact-and$view.relocation.tier-in@('block','heading','approximate','invalid')) "Deleted anchor incorrectly claims precision: $($view.relocation.tier)"
    Require (![string]::IsNullOrWhiteSpace($view.relocation.reason)) 'Deleted anchor omitted its downgrade reason'
   }else{
    Require ($tier-in@('approximate','nearby-heading','empty')) "Deleted anchor did not report a fallback tier: $tier"
   }
   Require ($fallback.anchor.byte-le[Text.Encoding]::UTF8.GetByteCount([IO.File]::ReadAllText($a))) 'Deleted anchor points outside shortened source'
   Require ($fallback.offset[1]-le2) 'Short replacement did not clamp the scroll position'
   Pass 'Deleted anchor and shortened document safely degrade without exactness claim' @{position=$fallback;tier=$tier;decision=$view.relocation}
   Observe '02-deleted-anchor-safe-fallback' -Screenshot|Out-Null
  }
  'corrupt'{
   $storeBase=$readingStore
   foreach($bad in @('malformed','future','negative','huge','missing')){
    $readingStore="$storeBase-$bad"
    switch($bad){
     'malformed' {[IO.File]::WriteAllText($readingStore,'{')}
     'future' {[IO.File]::WriteAllText($readingStore,'{"version":999,"records":{}}')}
     'negative' {[IO.File]::WriteAllText($readingStore,'{"version":1,"records":{"invalid":{"id":42,"path":"invalid","views":{"main/preview":{"offset":[0,-1]}}}}}')}
     'huge' {[IO.File]::WriteAllText($readingStore,'{"version":1,"records":{"invalid":{"id":42,"path":"invalid","views":{"main/preview":{"offset":[0,1e38]}}}}}')}
    }
    StartReadingApp $a;SetMode 'Read';Settle 'main/preview'
    Require ((Position 'main/preview').offset[1]-le2) "$bad reading state did not degrade to the top"
    Pass "$bad reading-state file starts safely at the top" (Position 'main/preview')
    EndReadingApp
   }
  }
  'guards'{
   StartReadingApp $a;SetMode 'Write';Settle 'main/source';WheelView 'main/source' 25
   $idA=(Reading).main_id;$saved=Position 'main/source'
   Keys '^o';Start-Sleep -Milliseconds 600;Keys '{ESC}';Settle 'main/source'
   Require ([string](Reading).main_id-ceq[string]$idA) 'Cancelled native open changed document ID'
   Pass 'Cancelled native chooser preserves the old document and source position' (SameOffset $saved (Position 'main/source') 'Open cancellation')
   Keys '^p';Keys '{ESC}';Settle 'main/source'
   Pass 'Cancelled Quick Switcher preserves source position' (SameOffset $saved (Position 'main/source') 'Quick cancellation')
   $broken=Join-Path $fixtures 'broken.md';[IO.File]::WriteAllBytes($broken,[byte[]]@(255,254,255))
   NativeOpen $broken;UiClick 'OK';Settle 'main/source'
   Require ([string](Reading).main_id-ceq[string]$idA) 'Failed target open changed old identity'
   Pass 'Target decoding failure keeps the original document usable' (SameOffset $saved (Position 'main/source') 'Target open failure')
   $rect=(View 'main/source').viewport_rect
   Click (($rect[0]+$rect[2])/2) (($rect[1]+$rect[3])/2);Keys 'UNSAVED'
   Require ((UiState).dirty) 'Dirty buffer was not established before guard acceptance'
   $dirtyText=(State).text;$dirtyPosition=Position 'main/source'
   Keys '^o';UiClick 'Cancel';Settle 'main/source'
   Require ((State).text-ceq$dirtyText) 'Unsaved-open cancellation changed source text'
   Pass 'Unsaved-open cancellation retains edits and position' (SameOffset $dirtyPosition (Position 'main/source') 'Dirty cancellation')
   $originalHash=(Get-FileHash -LiteralPath $a).Hash
   $fileLock=[IO.File]::Open($a,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
   Keys '^o'
   $saveControl=@((UiState).controls|Where-Object name -eq 'Save changes')|Select-Object -Last 1
   if($saveControl){UiClick 'Save changes'}else{
    # The existing primary button is drawn immediately after Discard and is
    # visible in the guard screenshot; it predates diagnostic registration.
    $discard=@((UiState).controls|Where-Object name -eq 'Discard')|Select-Object -Last 1
    Require ($null-ne$discard) 'No actual unsaved-document modal'
    Click ($discard.rect[2]+58) (($discard.rect[1]+$discard.rect[3])/2)
   }
   Start-Sleep -Milliseconds 450
   Observe '01-save-failure-keeps-manuscript' -Screenshot|Out-Null
   Require ([string](Reading).main_id-ceq[string]$idA) 'Failed old-document save switched identity'
   Require ((State).text-ceq$dirtyText-and(UiState).dirty) 'Failed save lost dirty source'
   Require ((Get-FileHash -LiteralPath $a).Hash-ceq$originalHash) 'Failed save changed original disk bytes'
   UiClick 'OK';UiClick 'Cancel';$fileLock.Dispose();$fileLock=$null;Settle 'main/source'
   Pass 'Real save replacement denial blocks switching and retains original bytes' (SameOffset $dirtyPosition (Position 'main/source') 'Failed old save')
   Keys '^n';UiClick 'Discard'
   $draftA=(Reading).main_id;Require ($null-eq(UiState).document_path) 'New document is not an unnamed draft'
   Keys '^n';$draftB=(Reading).main_id
   Require ([string]$draftA-cne[string]$draftB) 'Two unnamed drafts share a document ID'
   Pass 'New unnamed drafts receive independent runtime IDs' @{first=$draftA;second=$draftB;restart_content_recovery='后续阶段，未声明'}
  }
  'source'{
   foreach($name in @('large.md','large-plain.txt','plain.txt')){
    $path=Join-Path $fixtures $name
    StartReadingApp $path;SetMode 'Write';Settle 'main/source'
    UiClick 'source-wrap';UiClick 'source-wrap-off';Settle 'main/source'
    $rect=(View 'main/source').viewport_rect
    Click (($rect[0]+$rect[2])/2) (($rect[1]+$rect[3])/2);Keys '^{HOME}';Keys '+{END}'
    $selection=(UiState).selection
    $record.source_selection_expected=@{document=$name;selection=$selection}
    Observe ($name+'-selection-at-top')|Out-Null
    Require ($selection[0]-ne$selection[1]) "$name did not establish a real source selection"
    WheelView 'main/source' 25
    HorizontalWheelView 'main/source';$saved=Position 'main/source'
    Observe ($name+'-selection-before-switch')|Out-Null
    QuickOpen 'b.md' $b;QuickOpen $name $path;Settle 'main/source'
    Require ((UiState).mode-eq'Write') "$name did not restore its last source view"
    $restored=(UiState).selection
    $record.source_selection_after_switch=$restored
    Require ($selection[0]-eq$restored[0]-and$selection[1]-eq$restored[1]) "$name changed cursor/selection direction after switching: [$selection] -> [$restored]"
    $switch=SameOffset $saved (Position 'main/source') "$name source switch"
    EndReadingApp;StartReadingApp $path;SetMode 'Write';Settle 'main/source'
    $restored=(UiState).selection
    Require ($selection[0]-eq$restored[0]-and$selection[1]-eq$restored[1]) "$name changed cursor/selection after restart"
    Pass "$name source viewport, cursor and selection restore separately" @{switch=$switch;restart=(SameOffset $saved (Position 'main/source') "$name source restart");selection=$selection}
    Observe ($name-replace'\.','-') -Screenshot|Out-Null
    EndReadingApp
   }
  }
  'reflow'{
   StartReadingApp $a;SetMode 'Read';Settle 'main/preview';WheelView 'main/preview' 33;$saved=Position 'main/preview'
   Keys '^+p';Start-Sleep -Milliseconds 250
   $slider=@((UiState).controls|Where-Object name -eq 'reading-font-size')|Select-Object -Last 1
   Require ($null-ne$slider) 'No real font-size slider rectangle'
   Click ($slider.rect[0]+0.43*($slider.rect[2]-$slider.rect[0])) (($slider.rect[1]+$slider.rect[3])/2)
   Keys '{ESC}';Settle 'main/preview'
   Require ((Position 'main/preview').layout-cne$saved.layout) 'Font input did not actually change layout'
   Pass 'Font-size change preserves the original reading block' (SameBlock $saved (Position 'main/preview') 'Font size')
   SetMode 'Split';Settle 'main/preview'
   Pass 'Split reflow preserves the original reading block' (SameBlock $saved (Position 'main/preview') 'Font and Split')
   SetMode 'Write';Settle 'main/source';WheelView 'main/source' 26;$source=Position 'main/source'
   UiClick 'source-wrap';UiClick 'source-wrap-off';Settle 'main/source'
   Pass 'Disabling source wrapping retains the original logical paragraph' (SameBlock $source (Position 'main/source') 'Wrap off')
   UiClick 'source-wrap';UiClick 'source-wrap-on';Settle 'main/source'
   Pass 'Enabling source wrapping retains the original logical paragraph' (SameBlock $source (Position 'main/source') 'Wrap on')
   Observe '01-font-size-and-wrapping' -Screenshot|Out-Null
  }
  'semantics'{
   StartReadingApp $a;SetMode 'Read';Settle 'main/preview'
   WheelView 'main/preview' 220;$end=Position 'main/preview'
   WheelView 'main/preview' 6;$endAgain=Position 'main/preview'
   Require ([Math]::Abs($end.offset[1]-$endAgain.offset[1])-le0.1) 'The end-of-document test did not actually reach the scroll limit'
   $id=(Reading).main_id;$recordAtEnd=RecordFor $id
   Require ($recordAtEnd.read-eq$false) 'Scrolling to the end automatically marked the document read'
   Pass 'Actual bottom remains independent from read status' @{end=$end;confirmed_limit=$endAgain;read=$recordAtEnd.read;furthest=$recordAtEnd.furthest;percentage_ui='not displayed; diagnostic label is current position'}
   WheelView 'main/preview' -35;$last=Position 'main/preview'
   Require ($last.offset[1]-lt$end.offset[1]) 'Backtracking did not leave the end'
   QuickOpen 'b.md' $b;QuickOpen 'a.md' $a;SetMode 'Read';Settle 'main/preview'
   Pass 'Last position after returning from the true bottom wins over furthest' (SameOffset $last (Position 'main/preview') 'Last after bottom')
   Observe '01-returned-from-bottom' -Screenshot|Out-Null
  }
  'same-file'{
   StartReadingApp $a;SetMode 'Write';Settle 'main/source'
   QuickOpen 'b.md' $b -Secondary;Settle 'secondary/source'
   NativeOpen $b;CurrentDocument $b;SetMode 'Write';Settle 'main/source';Settle 'secondary/source'
   $ids=Reading
   Require ([string]$ids.main_id-ceq[string]$ids.secondary_id) 'The same-file fixture did not share its stable file identity'
   $original=[IO.File]::ReadAllText($b)
   $rect=(View 'main/source').viewport_rect
   Click (($rect[0]+$rect[2])/2) (($rect[1]+$rect[3])/2);Keys '^{HOME}';Paste '主🙂'
   Await {param($s)$s.text.StartsWith('主🙂')} 'Main buffer edit'|Out-Null
   $rect=(View 'secondary/source').viewport_rect
   Click (($rect[0]+$rect[2])/2) (($rect[1]+$rect[3])/2);Keys '^{HOME}';Paste 'S'
   Await {param($s)$s.secondary.text.StartsWith('S')} 'Secondary buffer edit'|Out-Null
   WheelView 'main/source' 12;WheelView 'secondary/source' 10
   $main=Position 'main/source';$side=Position 'secondary/source';$buffers=State;$version=Reading
   Require ($buffers.text-ceq('主🙂'+$original)-and$buffers.secondary.text-ceq('S'+$original)) 'The independent buffer edits leaked between panes'
   Require ([string]$main.fingerprint-cne[string]$side.fingerprint) 'Distinct same-file buffers shared a reading fingerprint'
   if($version.PSObject.Properties.Name-contains'main_revision'){
    Require ($version.main_revision-eq$version.secondary_revision) 'The same-file fixture did not reach equal local revisions'
   }
   foreach($pair in @(@($buffers.text,$main),@($buffers.secondary.text,$side))){
    $bytes=[Text.Encoding]::UTF8.GetBytes($pair[0]);$byte=[int]$pair[1].anchor.byte
    Require ($byte-ge0-and$byte-le$bytes.Length) 'Same-file anchor exceeded its own buffer'
    if($byte-lt$bytes.Length){Require (($bytes[$byte]-band0xC0)-ne0x80) 'Same-file anchor split a UTF-8 scalar'}
   }
   Pass 'Same file and equal revisions retain independent Unicode buffer indexes and pane positions' @{main=$main;secondary=$side;main_revision=$version.main_revision;secondary_revision=$version.secondary_revision;document_id=$ids.main_id}
   Observe '01-independent-same-file-buffers' -Screenshot|Out-Null
   # Remove only the test prefixes through the visible editors. A caret move
   # may itself occupy an egui undo snapshot, so cleanup does not assume a
   # particular undo coalescing boundary unrelated to reading-state isolation.
   $rect=(View 'secondary/source').viewport_rect
   Click (($rect[0]+$rect[2])/2) (($rect[1]+$rect[3])/2);Keys '^{HOME}';Keys '+{RIGHT}';Keys '{DELETE}'
   Await {param($s)$s.secondary.text-ceq$original-and!$s.secondary.dirty} 'Remove secondary test prefix'|Out-Null
   $rect=(View 'main/source').viewport_rect
   Click (($rect[0]+$rect[2])/2) (($rect[1]+$rect[3])/2);Keys '^{HOME}';Keys '+{RIGHT 2}';Keys '{DELETE}'
   Await {param($s)$s.text-ceq$original} 'Remove main test prefix'|Out-Null
   WheelView 'secondary/source' 10;$beforeSave=Position 'secondary/source';$beforeId=(Reading).secondary_id
   $rect=(View 'secondary/source').viewport_rect
   Click (($rect[0]+$rect[2])/2) (($rect[1]+$rect[3])/2);Keys '^s'
   Await {param($s)!$s.busy-and!$s.secondary.dirty} 'Secondary native save'|Out-Null
   Settle 'secondary/source'
   Require ([string](Reading).secondary_id-ceq[string]$beforeId) 'Worker save clone changed stable file identity'
   Pass 'Secondary worker save keeps observation and viewport active after buffer replacement' (SameOffset $beforeSave (Position 'secondary/source') 'Saved worker clone')
  }
  'capacity'{
   StartReadingApp $a;SetMode 'Read';Settle 'main/preview';WheelView 'main/preview' 25
   $saved=Position 'main/preview';$idA=(Reading).main_id;EndReadingApp
   $disk=Get-Content -LiteralPath $readingStore -Raw|ConvertFrom-Json -AsHashtable
   $key=@($disk.records.Keys)[0];$template=$disk.records[$key];$oldKeys=@()
   # Seed inactive history through the public on-disk schema, then exercise real
   # native openings and persistence. No live report or control state is altered.
   for($i=0;$i-lt511;$i++){
    $inactive=$template|ConvertTo-Json -Depth 25|ConvertFrom-Json -AsHashtable
    $inactive.id=100000+$i;$inactive.path=(Join-Path $fixtures ("inactive-{0:D4}.md"-f$i)).Replace('\','/').ToLowerInvariant()
    $inactive.updated_ms=1000+$i;$inactive.views=@{};$inactive.selections=@{};$inactive.calibration=@{}
    $disk.records[$inactive.path]=$inactive
    $oldKeys+=,$inactive.path
   }
   $disk|ConvertTo-Json -Depth 30 -Compress|Set-Content -LiteralPath $readingStore -Encoding utf8NoBOM
   $seeded=(Get-Item -LiteralPath $readingStore).Length
   StartReadingApp $a;Settle 'main/preview'
   Require ([string](Reading).main_id-ceq[string]$idA) 'Capacity load lost the real active document identity'
   Pass 'History at record capacity still restores the real document' (SameOffset $saved (Position 'main/preview') 'Capacity A')
   QuickOpen 'b.md' $b -Secondary;Settle 'secondary/source';WheelView 'secondary/source' 12
   WaitReading {param($s)!$s.dirty-and$s.writes.writes-gt0} 'Capacity eviction did not persist'|Out-Null
   $after=Get-Content -LiteralPath $readingStore -Raw|ConvertFrom-Json -AsHashtable
   $active=Reading
   Require ($after.records.Count-le512-and$active.records.Count-le512) 'Record count exceeded its documented budget'
   Require ((Get-Item -LiteralPath $readingStore).Length-le4MB) 'State bytes exceeded the documented budget'
   Require (!$after.records.ContainsKey($oldKeys[0])) 'Oldest inactive history was not evicted'
   Require (@($active.records|Where-Object {[string]$_.id-ceq[string]$idA}).Count-eq1) 'Capacity eviction removed the active main record'
   Require (@($active.records|Where-Object {[string]$_.id-ceq[string]$active.secondary_id}).Count-eq1) 'Capacity eviction removed the active secondary record'
   Pass 'Native openings evict inactive history and preserve both active panes within limits' @{seeded_bytes=$seeded;records=$after.records.Count;bytes=(Get-Item -LiteralPath $readingStore).Length;main_id=$idA;secondary_id=$active.secondary_id}
   Observe '01-capacity-active-panes' -Screenshot|Out-Null
  }
  'priorities'{
   $giant=Join-Path $fixtures 'pending-large.md'
   [IO.File]::WriteAllText($giant,([IO.File]::ReadAllText((Join-Path $fixtures 'large.md'))*20),[Text.UTF8Encoding]::new($false))
   $giantHash=(Get-FileHash -LiteralPath $giant).Hash
   StartReadingApp $a;SetMode 'Read';Settle 'main/preview';WheelView 'main/preview' 33
   function FastTreeClick($control){
    Require ([MdcardUi]::GetForegroundWindow()-eq$script:app.MainWindowHandle) 'Rapid click does not own foreground'
    $bounds=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0
    [void][MdcardUi]::SetCursorPos($bounds.Left+[int]((($control.rect[0]+$control.rect[2])/2)*$scale),$bounds.Top+[int]((($control.rect[1]+$control.rect[3])/2)*$scale))
    # Give native pointer motion a frame before pressing a different widget.
    # These remain rapid physical clicks; bypassing motion dispatch can omit a
    # click entirely and would not exercise an actual A/B/C transition.
    Start-Sleep -Milliseconds 45
    [MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
   }
   $controls=(UiState).controls
   $middle=@($controls|Where-Object name -eq 'tree:large.md')|Select-Object -Last 1
   $last=@($controls|Where-Object name -eq 'tree:b.md')|Select-Object -Last 1
   Require ($null-ne$middle-and$null-ne$last) 'Rapid-switch file entries are not visible'
   $clock=[Diagnostics.Stopwatch]::StartNew()
   FastTreeClick $middle;Start-Sleep -Milliseconds 60;$middleObservation=Reading;$middlePath=(UiState).document_path
   Require (PathEquals $middlePath (Join-Path $fixtures 'large.md')) 'The rapid middle click was not dispatched before the final click'
   FastTreeClick $last;$interval=$clock.Elapsed.TotalMilliseconds
   $c=$b;CurrentDocument $c;SetMode 'Read';Settle 'main/preview'
   $position=Position 'main/preview';$lastId=(Reading).main_id
   Start-Sleep -Milliseconds 1500
   Require ([string](Reading).main_id-ceq[string]$lastId-and(PathEquals (UiState).document_path $c)) 'Late opening result replaced the final document'
   Pass 'Rapid native A to large B to C keeps the final document and its viewport' @{native_click_interval_ms=$interval;middle_path=$middlePath;middle_document_id=$middleObservation.main_id;tail=(SameOffset $position (Position 'main/preview') 'Rapid switch tail');controlled_late_worker='separate explicit-delay core regression'}
   Observe '01-rapid-switch-tail' -Screenshot|Out-Null
   QuickOpen 'pending-large.md' $giant;SetMode 'Read';Settle 'main/preview'
   WheelView 'main/preview' 34;$history=Position 'main/preview';$largeId=[string](Reading).main_id
   QuickOpen 'b.md' $b;Settle 'main/preview';$fallback=(View 'main/preview').viewport_rect
   . "$PSScriptRoot/reading-state-restore-observer.ps1"
   $watch=[ReadingRestoreObserver]::Start($script:app.MainWindowHandle,$readingReport,$largeId,[double[]]$fallback)
   UiClick 'tree:pending-large.md';CurrentDocument $giant
   Require ($watch.Wait(6000)) 'Native restore observer did not finish'
   $observed=$watch.Result|ConvertFrom-Json
   Require $observed.observed "Could not observe pending restore: $($observed.error)"
   $observed.frame|ConvertTo-Json -Depth 30|Set-Content -LiteralPath (Join-Path $output 'observed-before-native-wheel.json') -Encoding utf8
   $pendingSeen=$true;$firstWheelMs=$observed.pending_to_native_wheel_ms
   for($i=1;$i-lt12;$i++){Start-Sleep -Milliseconds 30;[MdcardUi]::mouse_event(0x800,0,0,([uint32]::MaxValue-119),[UIntPtr]::Zero)}
   Settle 'main/preview'
   WheelView 'main/preview' 12
   Require ((View 'main/preview').reason-eq'user-scroll'-and$null-eq(View 'main/preview').pending) 'Native wheel did not cancel the pending automatic reflow'
   $chosen=Position 'main/preview';Start-Sleep -Milliseconds 1500
   Require ([Math]::Abs($chosen.offset[1]-$history.offset[1])-gt2) 'Cancellation fixture accidentally chose the historical offset'
   Require ((Get-FileHash -LiteralPath $giant).Hash-ceq$giantHash) 'Pending-load acceptance changed its manuscript bytes'
   Pass 'User wheel cancels observed pending large-document restoration and the page stays at the chosen position' @{observed_pending=$pendingSeen;pending_observed_to_first_native_wheel_ms=$firstWheelMs;reason=(View 'main/preview').reason;historical=$history;fixture_bytes=(Get-Item -LiteralPath $giant).Length;fixture_sha256=$giantHash;tail=(SameOffset $chosen (Position 'main/preview') 'User scroll tail')}
   Observe '02-user-cancelled-reflow' -Screenshot|Out-Null
  }
 }
 FinishMatrix
}catch{
 $record.error=$_.Exception.Message;$record.error_stack=$_.ScriptStackTrace;$record.passed=$false
 if($script:app-and!$script:app.HasExited){try{Shot 'failure'}catch{}}
}finally{
 if($fileLock){$fileLock.Dispose()}
 if($script:app-and!$script:app.HasExited){
  if($script:previousInputLayout){[void][ReadingInputNative]::PostMessage($script:app.MainWindowHandle,0x50,[IntPtr]::Zero,$script:previousInputLayout)}
  $script:app.Kill();$script:app.WaitForExit()
 }
 if($script:readingClipboardModified-and$priorClipboard){[WorkbenchClipboard]::SetDataObject($priorClipboard,$true)}
 $record.finished=[DateTimeOffset]::UtcNow.ToString('o')
 $record|ConvertTo-Json -Depth 30|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
}
$record|Select-Object verification_id,stage,passed,error,cases|ConvertTo-Json -Depth 15
if(!$record.passed){throw "Independent GUI matrix failed: $Scenario"}
