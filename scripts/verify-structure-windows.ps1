# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$Exe="$PSScriptRoot\..\artifacts\structure\MDcard.exe")
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
$output=Join-Path $root 'evidence\structure\windows-final'
New-Item -ItemType Directory -Force $output | Out-Null
$settings=Join-Path $output 'settings.json'
$verificationId=[Guid]::NewGuid().ToString()
$fixture=Join-Path $output 'fixture.md'
$original="# Visual table and blocks`r`n`r`nAlpha 🦀 paragraph.`r`n`r`nBeta paragraph.`r`n`r`nGamma paragraph.`r`n`r`n| Name | Value |`r`n| :--- | ---: |`r`n| Apples | 10 |`r`n| Pears | 20 |`r`n`r`n- Parent one`r`n  - Child A`r`n- Parent two`r`n- Parent three`r`n`r`n``````rust`r`nlet protected = 1;`r`n```````r`n"
[IO.File]::WriteAllText($fixture,$original,[Text.UTF8Encoding]::new($true))
$initialHash=(Get-FileHash $fixture).Hash
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
function Paste([string]$Text) {Set-Clipboard -Value $Text;Keys '^v'}
function ReadSaved {Keys '^s';return [IO.File]::ReadAllText($fixture)}
function MustContain([string]$Text,[string]$Needle) {if(!$Text.Contains($Needle)){throw "Expected source missing: $Needle`nActual:`n$Text"}}
function CloseTest {Keys '^q';if(!$script:app.WaitForExit(5000)){throw 'Test app did not close cleanly'}}
function RightClick([float]$X,[float]$Y) {$r=Bounds;$s=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.;[void][MdcardUi]::SetCursorPos($r.Left+[int]($X*$s),$r.Top+[int]($Y*$s));[MdcardUi]::mouse_event(8,0,0,0,[UIntPtr]::Zero);[MdcardUi]::mouse_event(16,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 350}
function Drag([float]$X,[float]$Y,[float]$ToX,[float]$ToY,[string]$Capture) {
 $r=Bounds;$s=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.
 [void][MdcardUi]::SetCursorPos($r.Left+[int]($X*$s),$r.Top+[int]($Y*$s));[MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
 for($step=1;$step -le 16;$step++){[void][MdcardUi]::SetCursorPos($r.Left+[int](($X+($ToX-$X)*$step/16)*$s),$r.Top+[int](($Y+($ToY-$Y)*$step/16)*$s));Start-Sleep -Milliseconds 35}
 Shot $Capture
 [MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 450
}
$previousClipboard=$null
try {$previousClipboard=[Windows.Forms.Clipboard]::GetDataObject()} catch {Write-Warning 'Clipboard backup unavailable; test paste uses clipboard.'}
Remove-Item -LiteralPath (Join-Path $output 'result.json') -ErrorAction SilentlyContinue
try {
 Launch 'renderer' '' $fixture
 Keys '^+t';Shot '01-grid'
 Keys '^s'
 if((Get-FileHash $fixture).Hash -ne $initialHash){throw 'Opening visual structure modified untouched BOM/CRLF bytes'}
 Click 390 431;Keys '^a';Paste 'Edited 🦀 value | pipe';Keys '{TAB}';Keys '^a';Paste '42'
 $text=ReadSaved
 MustContain $text '| Edited 🦀 value \| pipe | 42 |'
 # Saving must retain the grid cursor, including continued editing and navigation.
 Paste 'x';$text=ReadSaved;MustContain $text '| Edited 🦀 value \| pipe | 42x |'
 Keys '^z';$text=ReadSaved;MustContain $text '| Edited 🦀 value \| pipe | 42 |'
 Keys '^+z';$text=ReadSaved;MustContain $text '| Edited 🦀 value \| pipe | 42x |'
 Keys '^z';Keys '{TAB}';Keys '^a';Paste 'Pears after save';$text=ReadSaved;MustContain $text '| Pears after save | 20 |'
 Keys '{TAB}';Keys '+{TAB}';Keys '^a';Paste 'Pears after ShiftTab';$text=ReadSaved;MustContain $text '| Pears after ShiftTab | 20 |'
 Shot '02-edited-grid'
 # The last visible body cell: Tab adds a row and focuses its first cell.
 Keys '{TAB}';Keys '{TAB}';Paste 'New final row';Keys '{TAB}';Paste '5'
 $text=ReadSaved;MustContain $text 'New final row';MustContain $text '5'
 if(([regex]::Matches($text,'(?m)^\|')).Count -ne 5){throw 'Last Tab added an unexpected number of rows'}
 Shot '03-tab-appended-row'
 # Delete the active final row, then continue typing in its surviving predecessor.
 Keys '^%{BACKSPACE}';Keys '^a';Paste 'surviving cell';$text=ReadSaved
 if($text.Contains('New final row')){throw 'Delete final row failed'}
 if(([regex]::Matches($text,'(?m)^\|')).Count -ne 4){throw 'Deleting last row left an extra body row'}
 MustContain $text 'surviving cell'
 # Column add/delete and alignment shortcuts operate from grid focus.
 Click 494 301;Keys '^a';Paste 'extra column';$text=ReadSaved;MustContain $text 'extra column'
 Click 602 301;Keys '^a';Paste '20';$text=ReadSaved
 if($text.Contains('extra column')){throw 'Delete final column failed'}
 Keys '^%2';$text=ReadSaved;MustContain $text '| :--- | :---: |'
 RightClick 340 360;Shot '04-column-menu';Keys '{ESC}'
 # Right click also keeps the correct target. First menu action inserts before column zero.
 RightClick 340 360;Click 400 382;$text=ReadSaved
 MustContain $text '|   | Name | Value |'
 Keys '^z';$text=ReadSaved
 MustContain $text '| Name | Value |'
 # Whole-table deletion is one reversible source operation.
 Click 433 263;$text=ReadSaved
 if($text.Contains('| Name | Value |')){throw 'Delete table failed'}
 Keys '^z';$text=ReadSaved;MustContain $text '| Name | Value |'
 # Real pointer drag, distant insertion order, visible insertion feedback.
 Click 372 190;Shot '05-blocks'
 Drag 282 375 400 470 '06-drag-target'
 $text=ReadSaved
 if(!($text.IndexOf('Beta paragraph.') -lt $text.IndexOf('Gamma paragraph.') -and $text.IndexOf('Gamma paragraph.') -lt $text.IndexOf('Alpha 🦀 paragraph.'))){throw 'Distant block drag did not insert after target'}
 Shot '07-blocks-reordered'
 # The moved block stays selected for repeated commands, rather than following an old AST index.
 Click 309 263;$text=ReadSaved
 if(!($text.IndexOf('Alpha 🦀 paragraph.') -lt $text.IndexOf('Gamma paragraph.'))){throw 'Move up lost selected block identity'}
 Keys '^z';Keys '^s'
 # Small native window must retain the structural controls and a scrollable body.
 [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,80,80,775,525,$true);Start-Sleep -Milliseconds 600;Shot '08-compact-blocks'
 CloseTest
 # Actual source shortcut wiring for list levels and subtree movement.
 $listPath=Join-Path $output 'list.md'
 $listOriginal="- a`r`n  - child`r`n- b`r`n- c`r`n"
 [IO.File]::WriteAllText($listPath,$listOriginal,[Text.UTF8Encoding]::new($false))
 Launch 'list-renderer' '' $listPath
 Click 400 205;Keys '^{HOME}';Keys '%{RIGHT}';Keys '^s'
 if([IO.File]::ReadAllText($listPath) -ne $listOriginal){throw 'First list item was incorrectly indented'}
 Shot '09-first-item-refused'
 Keys '{DOWN 2}';Keys '%{RIGHT}';Keys '^s'
 if([IO.File]::ReadAllText($listPath) -ne $listOriginal.Replace('- b', '  - b')){throw 'List indent shortcut failed'}
 Shot '10-list-indent'
 Keys '%{LEFT}';Keys '^s'
 if([IO.File]::ReadAllText($listPath) -ne $listOriginal){throw 'List outdent shortcut failed'}
 Keys '%{DOWN}';Keys '^s'
 if([IO.File]::ReadAllText($listPath) -ne "- a`r`n  - child`r`n- c`r`n- b`r`n"){throw 'List move down shortcut failed'}
 Keys '%{UP}';Keys '^s'
 if([IO.File]::ReadAllText($listPath) -ne $listOriginal){throw 'List move up shortcut failed'}
 Shot '11-list-restored'
 CloseTest
 # Clearing a borderless edge cell must keep a two-column editable table.
 $headerPath=Join-Path $output 'empty-edge.md'
 $headerOriginal="a | b`r`n--- | ---`r`nx | y"
 [IO.File]::WriteAllText($headerPath,$headerOriginal,[Text.UTF8Encoding]::new($false))
 Launch 'empty-edge-renderer' '' $headerPath
 Keys '^+t';Click 390 396;Keys '^a';Keys '{BACKSPACE}';Keys '^s'
 if([IO.File]::ReadAllText($headerPath) -ne "| | b`r`n--- | ---`r`nx | y"){throw 'Clearing borderless header lost table shape'}
 Paste 'replacement';Keys '^s';MustContain ([IO.File]::ReadAllText($headerPath)) 'replacement'
 Keys '^z';Keys '^z';Keys '^s'
 if([IO.File]::ReadAllText($headerPath) -ne $headerOriginal){throw 'Empty-header edits did not undo exactly'}
 Click 390 431;Keys '^a';Keys '{BACKSPACE}';Keys '^s'
 if([IO.File]::ReadAllText($headerPath) -ne "a | b`r`n--- | ---`r`n| | y"){throw 'Clearing borderless body shifted neighbor'}
 Shot '12-empty-edge-body'
 CloseTest
 $bytes=[IO.File]::ReadAllBytes($fixture)
 if($bytes[0] -ne 239 -or $bytes[1] -ne 187 -or $bytes[2] -ne 191){throw 'UTF-8 BOM was lost after visual edit'}
 @{
  verification_id=$verificationId;date='2026-10-19';timezone='Asia/Shanghai';exe=(Resolve-Path $Exe).Path;exe_sha256=(Get-FileHash $Exe).Hash
  real_mouse_keyboard=$true;untouched_bytes_exact=$true;bom_retained=$true;cell_edit_pipe_unicode=$true;save_retains_grid_focus=$true;tab_shift_tab=$true
  final_tab_adds_row=$true;delete_final_row_column_focus=$true;column_insert_before=$true;column_context_menu=$true;alignment_shortcut=$true;table_delete_undo=$true
  distant_pointer_drag=$true;block_identity_after_move=$true;compact_logical='620x420';test_windows_closed=$true
  list_indent_outdent_alt_move=$true;first_list_item_refused=$true
  empty_header_remains_editable=$true;empty_edge_body_keeps_neighbor_column=$true;empty_header_undo_exact=$true
 }|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
} finally {
 if($script:app -and !$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()}
 if($null-ne$previousClipboard){[Windows.Forms.Clipboard]::SetDataObject($previousClipboard,$true)}
}
Write-Output $output
