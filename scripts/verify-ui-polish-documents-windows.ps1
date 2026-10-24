# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe,[string]$Stage='package')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-search-master-windows.ps1" -Exe $Exe -LoadFunctionsOnly
$output=Join-Path $root "evidence\ui-polish-2026-10-22\master\documents-$Stage"
$fixtures=Join-Path $output 'workspace';New-Item -ItemType Directory -Force $output,$fixtures|Out-Null
$settings=Join-Path $output 'settings.json';$report=Join-Path $output 'state.json';$a=Join-Path $fixtures 'original.md';$newPath=Join-Path $fixtures 'new-document.md'
$env:MDCARD_UI_REPORT=Join-Path $output 'ui.json'
$original="# 文档保护`r`n`r`n原始中文稿件 🦀`r`n";$addition='Added 中文 🦀'
$bomUtf8=[Text.UTF8Encoding]::new($true);[IO.File]::WriteAllText($a,$original,$bomUtf8);$initialHash=(Get-FileHash $a).Hash
if(Test-Path -LiteralPath $newPath){throw 'Use a new Stage; target fixture already exists'}
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$priorClipboard=[Windows.Forms.Clipboard]::GetDataObject();$record=@{verification_id=[Guid]::NewGuid().ToString();binary_sha256=(Get-FileHash -LiteralPath $Exe).Hash;stage=$Stage}
function UiState {for($i=0;$i-lt30;$i++){try{return Get-Content -Raw -LiteralPath $env:MDCARD_UI_REPORT|ConvertFrom-Json}catch{Start-Sleep -Milliseconds 30}};throw 'UI report unavailable'}
function UiClick([string]$Name){$ui=UiState;$r=@($ui.controls|Where-Object {$_.name-ceq$Name-and$_.enabled})|Select-Object -Last 1;if(!$r){throw "No enabled control $Name"};Click (($r.rect[0]+$r.rect[2])/2) (($r.rect[1]+$r.rect[3])/2)}
try {
 StartReview;Click 450 250;Keys '^{END}';Paste $addition
 Await {param($s)$s.text-ceq($original+$addition)} 'Initial source edit'|Out-Null
 Keys '^z';Await {param($s)$s.text-ceq$original} 'Undo returns original source'|Out-Null
 $record.undo_returns_clean=(!(UiState).dirty);Shot '01-undo-returned-clean'
 Keys '^+z';Await {param($s)$s.text-ceq($original+$addition)} 'Redo returns exact source'|Out-Null
 Keys '^s';$saved=[IO.File]::ReadAllBytes($a)
 $record.save_exact=([IO.File]::ReadAllText($a)-ceq($original+$addition));$record.bom_preserved=($saved[0]-eq239-and$saved[1]-eq187-and$saved[2]-eq191)
 Keys '^z';Keys '^s';$record.undo_save_restores_original_bytes=((Get-FileHash $a).Hash-ceq$initialHash)
 Click 450 250;Keys '^{END}';Paste 'UNSAVED';Keys '^n';Shot '02-unsaved-new-protection';UiClick 'Cancel'
 $record.cancel_retains_edits=((State).text-ceq($original+'UNSAVED'))
 $record.cancel_keeps_disk=((Get-FileHash $a).Hash-ceq$initialHash)
 Keys '^n';UiClick 'Discard';Await {param($s)$null-eq$s.path-and$s.text-eq''} 'New unnamed draft'|Out-Null
 Click 450 250;Paste "# 新草稿`n`n中文 🦀";Keys '^s'
 Start-Sleep -Milliseconds 700;Keys '%n';Set-Clipboard -Value $newPath;Keys '^a';Keys '^v';Keys '{ENTER}'
 Await {param($s)$s.path-and$s.path.EndsWith('new-document.md')} 'Native Save As accepted path'|Out-Null
 $record.save_as_exact=([IO.File]::ReadAllText($newPath)-ceq"# 新草稿`n`n中文 🦀")
 $record.original_protected=((Get-FileHash $a).Hash-ceq$initialHash)
 Shot '03-saved-new-document'
 Keys '^o';Start-Sleep -Milliseconds 700;Keys '%n';Set-Clipboard -Value $a;Keys '^a';Keys '^v';Keys '{ENTER}'
 Await {param($s)$s.path-and$s.path.EndsWith('original.md')-and$s.text-ceq$original} 'Native Open restores original'|Out-Null
 $record.open_exact=$true;Shot '04-open-original'
 Keys '^q';$record.clean_exit=$script:app.WaitForExit(5000)
}catch{$record.error=$_.Exception.Message}
finally{
 if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()}
 if($priorClipboard){[Windows.Forms.Clipboard]::SetDataObject($priorClipboard,$true)}
 $record|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
}
$record|ConvertTo-Json -Depth 6
