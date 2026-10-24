# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe,[string]$Stage='final-r9-menu')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\workbench-clipboard.ps1"
. "$PSScriptRoot\verify-search-master-windows.ps1" -Exe $Exe -LoadFunctionsOnly
. "$PSScriptRoot\workbench-native-input.ps1"
$output=Join-Path $root "evidence\workbench-redesign-2026-10-22\component-states-$Stage"
$fixtures=Join-Path $output 'workspace';New-Item -ItemType Directory -Force $output,$fixtures|Out-Null
$settings=Join-Path $output 'settings.json';$report=Join-Path $output 'state.json';$a=Join-Path $fixtures 'states.md'
$env:MDCARD_UI_REPORT=Join-Path $output 'ui.json'
$original="# Component states`n`n中文 untouched source.";[IO.File]::WriteAllText($a,$original,$utf8)
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$prior=[WorkbenchClipboard]::GetDataObject();$result=@{verification_id=[Guid]::NewGuid().ToString();binary_sha256=(Get-FileHash $Exe).Hash;passed=$false}
function UiState {for($i=0;$i-lt30;$i++){try{return Get-Content -Raw -LiteralPath $env:MDCARD_UI_REPORT|ConvertFrom-Json}catch{Start-Sleep -Milliseconds 30}};throw 'UI report unavailable'}
function UiClick([string]$Name){$response=@((UiState).controls|Where-Object {$_.name-ceq$Name-and$_.enabled})|Select-Object -Last 1;if(!$response){throw "No enabled UI control $Name"};Click (($response.rect[0]+$response.rect[2])/2) (($response.rect[1]+$response.rect[3])/2)}
try{
 StartReview
 $save=@((UiState).controls|Where-Object {$_.name-ceq'Save'-and$_.enabled})|Select-Object -First 1
 if(!$save){throw 'Save control unavailable'}
 $bounds=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0
 $x=$bounds.Left+[int](($save.rect[0]+$save.rect[2])*.5*$scale);$y=$bounds.Top+[int](($save.rect[1]+$save.rect[3])*.5*$scale)
 [void][MdcardUi]::SetCursorPos($x,$y);Start-Sleep -Milliseconds 900;Shot '01-save-hover-tooltip'
 [MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 120;Shot '02-save-pressed';[MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 200
 $result.physical_hover_and_press_captured=$true
 Click 450 250
 Click 230 62;UiClick 'Global search · Ctrl+Shift+F'
 Await {param($s)$s.open} 'Global search actually open before controls'|Out-Null
 Control 'regex';Fill 'find-query' '['
 $errorState=Await {param($s)!$s.query_pending-and[bool]$s.error} 'Invalid regex produces inline error'
 $result.regex_error_visible=[bool]$errorState.error;Shot '03-invalid-regex-error'
 Fill 'find-query' '(?m)^Component';Await {param($s)!$s.query_pending-and!$s.error} 'Corrected regex recovers'|Out-Null
 $result.error_recoverable=$true;Keys '{ESC}'
 Keys '^p';Fill 'quick-query' 'absolutely-no-matching-file-972985'
 Await {param($s)$s.quick_open-and@($s.quick).Count-eq0} 'No-result Quick state'|Out-Null
 $result.quick_empty_visible=$true;Shot '04-quick-empty-keyboard-focus';Keys '{ESC}'
 $result.source_unchanged=((State).text-ceq$original-and[IO.File]::ReadAllText($a)-ceq$original-and!(UiState).dirty)
 Keys '^q';$result.clean_exit=$script:app.WaitForExit(5000)
 $result.passed=$result.physical_hover_and_press_captured-and$result.regex_error_visible-and$result.error_recoverable-and$result.quick_empty_visible-and$result.source_unchanged-and$result.clean_exit
}catch{$result.error=$_.Exception.Message;Shot 'failure'}
finally{[MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero);if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()};if($prior){[WorkbenchClipboard]::SetDataObject($prior,$true)};$result|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8}
$result|ConvertTo-Json -Depth 6
if(!$result.passed){throw 'Required component-state checks failed; inspect result.json'}
