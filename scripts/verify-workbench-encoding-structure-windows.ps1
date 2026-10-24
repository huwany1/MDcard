# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe,[string]$Stage='package')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\workbench-clipboard.ps1"
. "$PSScriptRoot\verify-search-master-windows.ps1" -Exe $Exe -LoadFunctionsOnly
. "$PSScriptRoot\workbench-native-input.ps1"
$output=Join-Path $root "evidence\workbench-redesign-2026-10-22\master\encoding-structure-$Stage"
$fixtures=Join-Path $output 'workspace';New-Item -ItemType Directory -Force $output,$fixtures|Out-Null
$settings=Join-Path $output 'settings.json';$report=Join-Path $output 'state.json';$a=Join-Path $fixtures 'source.md'
$env:MDCARD_UI_REPORT=Join-Path $output 'ui.json'
$original="# 中文编码与结构`r`n`r`n中文 🦀`r`n`r`n| Name | Value |`r`n| --- | --- |`r`n| Apples | 10 |`r`n"
[IO.File]::WriteAllText($a,$original,$utf8)
$initialHash=(Get-FileHash $a).Hash
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$priorClipboard=[WorkbenchClipboard]::GetDataObject();$record=@{verification_id=[Guid]::NewGuid().ToString();binary_sha256=(Get-FileHash -LiteralPath $Exe).Hash;stage=$Stage}
function UiState {for($i=0;$i-lt30;$i++){try{return Get-Content -Raw -LiteralPath $env:MDCARD_UI_REPORT|ConvertFrom-Json}catch{Start-Sleep -Milliseconds 30}};throw 'UI report unavailable'}
function UiClick([string]$Name){$r=@((UiState).controls|Where-Object {$_.name-ceq$Name-and$_.enabled})|Select-Object -Last 1;if(!$r){throw "No enabled control $Name"};Click (($r.rect[0]+$r.rect[2])/2) (($r.rect[1]+$r.rect[3])/2)}
try {
 StartReview;Click 450 250;Keys '^+e';Shot '01-encoding-open'
 Keys '{TAB 2}';Keys ' ';Shot '02-encoding-selected';UiClick 'Use for saving'
 $record.encoding_pending_dirty=(UiState).dirty
 $record.encoding_pending_leaves_disk=((Get-FileHash $a).Hash-ceq$initialHash)
 if(!$record.encoding_pending_dirty){throw 'Encoding conversion was not pending dirty'}
 Keys '^n';UiClick 'Cancel';$record.encoding_unsaved_cancel_retains_source=((State).text-ceq$original-and(UiState).dirty)
 Keys '^s';$bytes=[IO.File]::ReadAllBytes($a);$expected=[Text.UTF8Encoding]::new($true).GetPreamble()+$utf8.GetBytes($original)
 $record.utf8_bom_crlf_conversion_exact=([Convert]::ToHexString($bytes)-ceq[Convert]::ToHexString($expected))
 if(!$record.utf8_bom_crlf_conversion_exact){throw 'Native encoding choice did not produce exact UTF-8 BOM reference'}
 $convertedHash=(Get-FileHash $a).Hash;Shot '03-converted-saved'
 Click 450 250;Keys '^{END}';Keys '{UP}';Keys '{HOME}';Keys '^+t';$record.structure_open_leaves_source=((State).text-ceq$original);Keys '^%{ENTER}'
 Await {param($s)$s.text-cne$original} 'Table row inserted into source'|Out-Null
 $record.structure_adds_one_row=([regex]::Matches((State).text,'(?m)^\|').Count-eq4)
 Shot '04-row-added';Keys '^z';Await {param($s)$s.text-ceq$original} 'Structure operation undoes atomically'|Out-Null
 Keys '{ESC}';Keys '^s';$record.structure_undo_exact_bytes=((Get-FileHash $a).Hash-ceq$convertedHash)
 $record.source_and_encoding_clean=(!(UiState).dirty-and(State).text-ceq$original)
 Keys '^q';$record.clean_exit=$script:app.WaitForExit(5000)
 $record.passed=$record.encoding_pending_dirty-and$record.encoding_pending_leaves_disk-and$record.encoding_unsaved_cancel_retains_source-and$record.utf8_bom_crlf_conversion_exact-and$record.structure_open_leaves_source-and$record.structure_adds_one_row-and$record.structure_undo_exact_bytes-and$record.source_and_encoding_clean-and$record.clean_exit
}catch{$record.error=$_.Exception.Message;$record.passed=$false}
finally{if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()};if($priorClipboard){[WorkbenchClipboard]::SetDataObject($priorClipboard,$true)};$record|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8}
$record|ConvertTo-Json -Depth 6

if(!$record.passed){throw "Required native verification checks failed; inspect result.json"}
