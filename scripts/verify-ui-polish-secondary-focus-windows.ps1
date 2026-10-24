# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe,[string]$Stage='final')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-search-master-windows.ps1" -Exe $Exe -LoadFunctionsOnly
$output=Join-Path $root "evidence\ui-polish-2026-10-22\master\secondary-focus-$Stage"
$fixtures=Join-Path $output 'workspace';New-Item -ItemType Directory -Force $output,$fixtures|Out-Null
$settings=Join-Path $output 'settings.json';$report=Join-Path $output 'state.json';$a=Join-Path $fixtures 'main.md';$b=Join-Path $fixtures 'second.md'
$env:MDCARD_UI_REPORT=Join-Path $output 'ui.json'
$original="# Main`nPRIMARY-PROTECTED";$other="# Second`n中文 secondary-selection"
[IO.File]::WriteAllText($a,$original,$utf8);[IO.File]::WriteAllText($b,$other,$utf8)
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$priorClipboard=[Windows.Forms.Clipboard]::GetDataObject();$record=@{verification_id=[Guid]::NewGuid().ToString();binary_sha256=(Get-FileHash -LiteralPath $Exe).Hash;stage=$Stage;cases=@()}
try {
 StartReview;Keys '^p';Fill 'quick-query' 'second.md';Await {param($s)@($s.quick).Count-eq1} 'Second document indexed'|Out-Null
 Keys '+{ENTER}';Await {param($s)[bool]$s.secondary} 'Secondary pane opened'|Out-Null
 foreach($case in @('preferences','encoding','quick-switcher','find-replace','export')) {
  Control 'secondary-editor';Keys '^{END}';Keys '+{HOME}';[Windows.Forms.Clipboard]::SetText('SENTINEL-SETUP');Keys '^c';$expected=[Windows.Forms.Clipboard]::GetText()
  if($expected-cne'中文 secondary-selection'){throw 'Failed to establish secondary selection'}
  switch($case){'preferences'{Keys '^+p';Keys '{ESC}'}'encoding'{Keys '^+e';Keys '{ESC}'}'quick-switcher'{Keys '^p';Keys '{ESC}'}'find-replace'{Keys '^f';Keys '{ESC}'}'export'{Keys '^%e';Keys '{ESC}'}}
  [Windows.Forms.Clipboard]::SetText('SENTINEL-NO-COPY');Keys '^c';$actual=[Windows.Forms.Clipboard]::GetText();$s=State
  $record.cases+=@{name=$case;selected_text=$actual;returned_to_secondary=($actual-ceq$expected);both_sources_unchanged=($s.text-ceq$original-and$s.secondary.text-ceq$other)}
  Shot $case
 }
 Keys '^q';$record.clean_exit=$script:app.WaitForExit(5000)
}catch{$record.error=$_.Exception.Message}
finally{if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()};if($priorClipboard){[Windows.Forms.Clipboard]::SetDataObject($priorClipboard,$true)};$record|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8}
$record|ConvertTo-Json -Depth 6
