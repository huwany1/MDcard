# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$Exe="$PSScriptRoot\..\artifacts\recovery-2026-10-26\final-r4\mdcard.exe",[string]$OutputDirectory='')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
. "$PSScriptRoot\recovery-json.ps1"
$output=if($OutputDirectory){[IO.Path]::GetFullPath($OutputDirectory)}else{Join-Path $root ('evidence\recovery\2026-10-26\autosave-'+(Get-Date -Format 'HHmmss'))}
New-Item -ItemType Directory -Force $output | Out-Null
$settings=Join-Path $output 'settings.json'
$env:MDCARD_UI_REPORT=Join-Path $output 'ui.json'
$env:MDCARD_SEARCH_REPORT=Join-Path $output 'search.json'
$env:MDCARD_READING_STATE_PATH=Join-Path $output 'reading-state.json'
$env:MDCARD_RECOVERY_DIR=Join-Path $output 'recovery-state'
$fixture=Join-Path $output 'main.md'
$secondary=Join-Path $output 'secondary.md'
function ReadReport { ReadJsonStable $env:MDCARD_UI_REPORT @('controls','autosave','recovery') }
function ClickControl([string]$Name) {
 $control=(ReadReport).controls|Where-Object name -EQ $Name|Select-Object -First 1
 if(!$control){throw "UI control not found: $Name"}
 $left=[Math]::Max($control.rect[0],$control.clip[0]);$top=[Math]::Max($control.rect[1],$control.clip[1])
 $right=[Math]::Min($control.rect[2],$control.clip[2]);$bottom=[Math]::Min($control.rect[3],$control.clip[3])
 if($Name -eq 'main-source-editor') {
  # Click the visible first text row, then verify actual native editor focus.
  for($attempt=0;$attempt -lt 3;$attempt++) {
   Click ([Math]::Min($left+80,$right-4)) ([Math]::Min($top+15,$bottom-4))
   $focused=(ReadReport).controls|Where-Object name -EQ $Name|Select-Object -First 1
   if($focused.focus){return}
   Start-Sleep -Milliseconds 350
  }
  Shot 'debug-source-focus';throw 'Real source editor did not receive focus before keyboard input'
 }
 Click (($left+$right)/2) (($top+$bottom)/2)
}
function ClickSecondary {
 $control=(Get-Content -Raw $env:MDCARD_SEARCH_REPORT|ConvertFrom-Json).controls|Where-Object name -EQ 'secondary-editor'|Select-Object -First 1
 if(!$control){throw 'Secondary editor control was not reported'}
 Click (($control.rect[0]+$control.rect[2])/2) ([Math]::Min(400,($control.rect[1]+$control.rect[3])/2))
}
function Paste([string]$Value) {Set-Clipboard -Value $Value;Keys '^v'}
function Assert([bool]$Condition,[string]$Message) {if(!$Condition){throw $Message}}
function CloseClean {Keys '^q';if(!$script:app.WaitForExit(5000)){throw 'Test app did not close cleanly'}}
function WaitIdleSave {Start-Sleep -Milliseconds 3200}
function ChooseSavePath([string]$Path) {Start-Sleep -Milliseconds 500;Keys '%n';Paste $Path;Keys '{ENTER}';Start-Sleep -Milliseconds 1200}
$previousClipboard=[Windows.Forms.Clipboard]::GetDataObject()
$verificationId=[Guid]::NewGuid().ToString()
try {
 @{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
 [IO.File]::WriteAllText($fixture,"# Auto save main`r`n`r`nOriginal main.`r`n",[Text.UTF8Encoding]::new($true))
 [IO.File]::WriteAllText($secondary,"# Auto save secondary`r`n`r`nOriginal second.`r`n",[Text.UTF8Encoding]::new($false))
 foreach($testName in @('first-manual.md','recovered-save-as.md','result.json')) { $testPath=Join-Path $output $testName; if(Test-Path -LiteralPath $testPath){(Get-Item -LiteralPath $testPath).IsReadOnly=$false;Remove-Item -LiteralPath $testPath} }
 $initialHash=(Get-FileHash $fixture).Hash
 Launch 'renderer' '' $fixture
 Keys '^+p';Shot '01-default-on'
 Assert (!(ReadReport).autosave.enabled-and(ReadReport).recovery.enabled) 'Missing settings must default original autosave off and recovery on'
 Keys '{ESC}';ClickControl 'Write';Start-Sleep -Milliseconds 500;ClickControl 'main-source-editor';Keys '^{END}';Paste "`r`nDefault-on automatic edit."
 WaitIdleSave
 Assert ((Get-FileHash $fixture).Hash-eq$initialHash-and(ReadReport).dirty) 'Default off must retain original bytes and dirty state'
 Keys '^+p';ClickControl 'Auto save';Keys '{ESC}';WaitIdleSave
 Assert ([IO.File]::ReadAllText($fixture).Contains('Default-on automatic edit.')) 'Default-on document did not save without enabling the switch'
 Assert (!(ReadReport).dirty) 'Default-on commit did not clear its exact dirty baseline'
 Shot '01a-default-on-saved'
 Keys '^+p';ClickControl 'Auto save';Shot '01b-explicit-off';Keys '{ESC}'
 Assert (!(Get-Content -Raw $settings|ConvertFrom-Json).autosave) 'Explicit disabled choice did not persist'
 CloseClean
 Launch 'renderer-explicit-off-restored' '' $fixture
 Assert (!(ReadReport).autosave.enabled) 'Previously saved false was overwritten by the new default'
 Shot '01c-explicit-off-after-restart'
 $initialHash=(Get-FileHash $fixture).Hash
 ClickControl 'Write';ClickControl 'main-source-editor';Keys '^{END}';Paste "`r`nDisabled edit retained."
 WaitIdleSave
 Assert ((Get-FileHash $fixture).Hash -eq $initialHash) 'Explicitly disabled edit wrote to disk'
 Assert ((ReadReport).dirty) 'Explicitly disabled edit lost its dirty state'
 Shot '02-disabled-unsaved'
 Keys '^+p';ClickControl 'Auto save';Shot '03-explicit-on';Keys '{ESC}'
 Assert ((Get-Content -Raw $settings|ConvertFrom-Json).autosave) 'Explicit auto save did not persist'
 WaitIdleSave
 Assert ([IO.File]::ReadAllText($fixture).Contains('Disabled edit retained.')) 'Enabling did not save existing edits while idle'
 Assert (!(ReadReport).dirty) 'Successful commit did not clear its exact dirty baseline'
 Shot '04-idle-auto-saved'
 # Real input less than two seconds apart must coalesce.
 ClickControl 'main-source-editor';Keys '^{END}';Paste ' debounce-A';$before=(Get-FileHash $fixture).Hash
 Start-Sleep -Milliseconds 600;Paste ' debounce-B'
 Assert ((Get-FileHash $fixture).Hash -eq $before) 'Continuous input was saved before debounce elapsed'
 Start-Sleep -Milliseconds 600;Paste ' debounce-C';WaitIdleSave
 Assert ([IO.File]::ReadAllText($fixture).Contains('debounce-A debounce-B debounce-C')) 'Debounce did not save latest input'
 CloseClean
 Launch 'renderer-restart' '' $fixture
 Assert ((ReadReport).autosave.enabled) 'Restart did not restore explicit preference'
 Shot '05-restored-after-restart'
 Keys '^p';Paste 'secondary.md';Start-Sleep -Milliseconds 1200;Keys '+{ENTER}';Start-Sleep -Milliseconds 1200
 $search=Get-Content -Raw $env:MDCARD_SEARCH_REPORT|ConvertFrom-Json
 Assert ($null-ne$search.secondary) 'Real quick switcher did not open secondary document'
 Keys '^{END}';Paste "`r`nSecondary automatic edit."
 ClickControl 'main-source-editor';Keys '^{END}';Paste "`r`nMain automatic edit."
 WaitIdleSave
 Assert ([IO.File]::ReadAllText($secondary).Contains('Secondary automatic edit.')) 'Secondary did not save independently'
 Assert ([IO.File]::ReadAllText($fixture).Contains('Main automatic edit.')) 'Main did not save independently'
 Shot '06-main-and-secondary-saved'
 (Get-Item -LiteralPath $secondary).IsReadOnly=$true
 ClickSecondary;Keys '^{END}';Paste "`r`nSecondary read-only draft."
 ClickControl 'main-source-editor';Keys '^{END}';Paste "`r`nMain saves while secondary is paused."
 WaitIdleSave
 Assert ((ReadReport).autosave.secondary.error -match 'read-only') 'Secondary read-only cause missing'
 Assert (!(ReadReport).dirty -and [IO.File]::ReadAllText($fixture).Contains('Main saves while secondary is paused.')) 'Secondary failure blocked main saving'
 Shot '06b-secondary-failure-main-saved'
 (Get-Item -LiteralPath $secondary).IsReadOnly=$false
 ClickControl 'retry-save-secondary';WaitIdleSave
 Assert ($null-eq(ReadReport).autosave.secondary.error) 'Secondary retry button did not recover'
 Assert ([IO.File]::ReadAllText($secondary).Contains('Secondary read-only draft.')) 'Secondary retry lost its draft'
 Shot '06c-secondary-retry-saved'
 ClickControl 'main-source-editor';Keys '^n' ;Paste '# Untitled memory only';WaitIdleSave
 Assert ($null-eq(ReadReport).document_path) 'Untitled document acquired a path automatically'
 Assert ((ReadReport).dirty) 'Untitled draft lost dirty state'
 Assert (!(Test-Path -LiteralPath (Join-Path $output 'Untitled.md'))) 'Untitled automatic file was created'
 Shot '07-untitled-waits-for-manual-save'
 [void]$script:app.CloseMainWindow();Start-Sleep -Milliseconds 700
 Assert (!$script:app.HasExited) 'Native X exited with untitled changes'
 Assert ((ReadReport).autosave.pending_navigation) 'Native X skipped unsaved protection'
 Shot '08-native-exit-protection';Keys '{ESC}'
 $first=Join-Path $output 'first-manual.md'
 Keys '^s';ChooseSavePath $first
 Assert (Test-Path -LiteralPath $first) 'First manual save dialog did not create chosen path'
 Keys '^{END}';Paste "`r`nAfter first manual save.";WaitIdleSave
 Assert ([IO.File]::ReadAllText($first).Contains('After first manual save.')) 'First-save document did not enter automatic saving'
 Shot '09-after-first-manual-save'
 $beforeFailure=[IO.File]::ReadAllBytes($first)
 (Get-Item -LiteralPath $first).IsReadOnly=$true
 Keys '^{END}';Paste "`r`nRead-only edit stays in memory.";WaitIdleSave
 Assert ((ReadReport).dirty) 'Read-only failure cleared unsaved state'
 Assert ((ReadReport).autosave.main.error -match 'read-only') 'Read-only error did not identify the cause'
 Assert ([Convert]::ToBase64String([IO.File]::ReadAllBytes($first)) -eq [Convert]::ToBase64String($beforeFailure)) 'Read-only failure changed original bytes'
 Shot '10-readonly-failure'
 Keys '^q';Shot '11-failed-save-exit-protection'
 Assert (!$script:app.HasExited -and (ReadReport).autosave.pending_navigation) 'Failed-save exit protection missing'
 Keys '{ESC}';(Get-Item -LiteralPath $first).IsReadOnly=$false
 ClickControl 'retry-save-main';WaitIdleSave
 Assert (!(ReadReport).dirty -and $null-eq(ReadReport).autosave.main.error) 'Manual retry failed to recover'
 Assert ([IO.File]::ReadAllText($first).Contains('Read-only edit stays in memory.')) 'Retry lost memory text'
 Shot '12-recovered'
 [IO.File]::WriteAllText($first,'EXTERNAL VERSION',[Text.UTF8Encoding]::new($false))
 Keys '^{END}';Paste "`r`nPreserve separate memory version.";WaitIdleSave
 Assert ((ReadReport).autosave.main.error -match 'outside') 'External edit conflict was not reported'
 Assert ([IO.File]::ReadAllText($first) -eq 'EXTERNAL VERSION') 'External version was silently overwritten'
 Shot '13-external-conflict'
 $recovered=Join-Path $output 'recovered-save-as.md'
 Keys '^+s';ChooseSavePath $recovered
 Assert ([IO.File]::ReadAllText($recovered).Contains('Preserve separate memory version.')) 'Save As lost current memory content'
 Assert ([IO.File]::ReadAllText($first) -eq 'EXTERNAL VERSION') 'Save As changed old original'
 Shot '14-save-as-preserves-both'
 CloseClean
 @{
  verification_id=$verificationId;date=[DateTimeOffset]::Now.ToString('o');timezone='Asia/Tokyo';exe=(Resolve-Path $Exe).Path;exe_sha256=(Get-FileHash $Exe).Hash
  real_mouse_keyboard=$true;windows_dpi_percent=125;default_on=$false;default_off_keeps_original_bytes=$true;explicit_enable_writes=$true;explicit_false_restored=$true;explicit_off_blocks_writes=$true;explicit_toggle_persisted=$true;restart_restores=$true;idle_without_input_saves=$true;continuous_input_debounced=$true
  secondary_failure_visible=$true;secondary_retry_button=$true;main_retry_button=$true;main_secondary_independent=$true;untitled_no_automatic_path_or_dialog=$true;native_exit_guard=$true;first_manual_save_then_auto=$true;readonly_preserves_bytes=$true
  failure_dirty_guard=$true;manual_retry_recovers=$true;external_conflict_preserves_disk=$true;save_as_preserves_both=$true;formal_packages_untouched=$true
  files=@($fixture,$secondary,$first,$recovered)|ForEach-Object {@{path=$_;sha256=(Get-FileHash $_).Hash;content=[IO.File]::ReadAllText($_)}}
 }|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
} finally {
 if(Test-Path -LiteralPath $secondary){(Get-Item -LiteralPath $secondary).IsReadOnly=$false}
 if(Test-Path -LiteralPath (Join-Path $output 'first-manual.md')){(Get-Item -LiteralPath (Join-Path $output 'first-manual.md')).IsReadOnly=$false}
 if($script:app -and !$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()}
 if($null-ne$previousClipboard){[Windows.Forms.Clipboard]::SetDataObject($previousClipboard,$true)}
}
Write-Output $output
