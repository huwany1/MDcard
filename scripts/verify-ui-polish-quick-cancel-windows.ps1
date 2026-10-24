# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe,[string]$Stage='final')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-search-master-windows.ps1" -Exe $Exe -LoadFunctionsOnly
$output=Join-Path $root "evidence\ui-polish-2026-10-22\master\quick-cancel-$Stage"
$fixtures=Join-Path $output 'workspace';New-Item -ItemType Directory -Force $output,$fixtures|Out-Null
$settings=Join-Path $output 'settings.json';$report=Join-Path $output 'state.json';$a=Join-Path $fixtures 'original.md';$b=Join-Path $fixtures 'destination.md'
$env:MDCARD_UI_REPORT=Join-Path $output 'ui.json'
$original="# Main`n`n中文 original-selection";$other="# Destination`n`n中文 destination-token";$dirty=$original+'-dirty'
[IO.File]::WriteAllText($a,$original,$utf8);[IO.File]::WriteAllText($b,$other,$utf8)
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$priorClipboard=[Windows.Forms.Clipboard]::GetDataObject();$record=@{verification_id=[Guid]::NewGuid().ToString();binary_sha256=(Get-FileHash -LiteralPath $Exe).Hash;stage=$Stage}
function UiState {for($i=0;$i-lt30;$i++){try{return Get-Content -Raw -LiteralPath $env:MDCARD_UI_REPORT|ConvertFrom-Json}catch{Start-Sleep -Milliseconds 30}};throw 'UI report unavailable'}
function UiClick([string]$Name){$r=@((UiState).controls|Where-Object {$_.name-ceq$Name-and$_.enabled})|Select-Object -Last 1;if(!$r){throw "No enabled control $Name"};Click (($r.rect[0]+$r.rect[2])/2) (($r.rect[1]+$r.rect[3])/2)}
try {
 StartReview;Click 450 250;Keys '^{END}';Paste '-dirty';Await {param($s)$s.text-ceq$dirty} 'Dirty original source'|Out-Null
 Keys '+{HOME}';[Windows.Forms.Clipboard]::SetText('SENTINEL');Keys '^c';Same ([Windows.Forms.Clipboard]::GetText()) '中文 original-selection-dirty' 'Original selection established'
 Keys '^p';Fill 'quick-query' 'destination.md';Await {param($s)@($s.quick).Count-eq1} 'Destination indexed'|Out-Null;Keys '{ENTER}'
 UiClick 'Cancel';$record.cancel_keeps_quick_open=(State).quick_open;Shot '01-cancel-navigation'
 Keys '{ESC}';[Windows.Forms.Clipboard]::SetText('SENTINEL-AFTER-CANCEL');Keys '^c'
 $record.cancel_restores_focus_and_selection=([Windows.Forms.Clipboard]::GetText()-ceq'中文 original-selection-dirty')
 $record.cancel_retains_dirty_source=((State).text-ceq$dirty-and(UiState).dirty)
 $record.cancel_keeps_both_files=([IO.File]::ReadAllText($a)-ceq$original-and[IO.File]::ReadAllText($b)-ceq$other)
 Shot '02-original-selection-restored'
 Keys '^z';Await {param($s)$s.text-ceq$original} 'Undo original dirty edit'|Out-Null
 Keys '^p';Fill 'quick-query' 'destination.md';Await {param($s)@($s.quick).Count-eq1} 'Destination available again'|Out-Null;Keys '{ENTER}'
 Await {param($s)!$s.quick_open-and!$s.busy-and$s.path.EndsWith('destination.md')-and$s.text-ceq$other} 'Successful primary navigation'|Out-Null
 Keys '^{END}';Keys '+{HOME}';[Windows.Forms.Clipboard]::SetText('SENTINEL-NEW-TARGET');Keys '^c'
 $record.successful_navigation_keeps_new_target_focus=([Windows.Forms.Clipboard]::GetText()-ceq'中文 destination-token')
 Shot '03-destination-focus';Keys '^q';$record.clean_exit=$script:app.WaitForExit(5000)
 $record.passed=$record.cancel_keeps_quick_open-and$record.cancel_restores_focus_and_selection-and$record.cancel_retains_dirty_source-and$record.cancel_keeps_both_files-and$record.successful_navigation_keeps_new_target_focus-and$record.clean_exit
}catch{$record.error=$_.Exception.Message;$record.passed=$false}
finally{if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()};if($priorClipboard){[Windows.Forms.Clipboard]::SetDataObject($priorClipboard,$true)};$record|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8}
$record|ConvertTo-Json -Depth 6
