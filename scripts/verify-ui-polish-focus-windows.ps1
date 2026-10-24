# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe,[string]$Stage='baseline')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-search-master-windows.ps1" -Exe $Exe -LoadFunctionsOnly
$output=Join-Path $root "evidence\ui-polish-2026-10-22\master\focus-$Stage"
$fixtures=Join-Path $output 'workspace';New-Item -ItemType Directory -Force $output,$fixtures|Out-Null
$settings=Join-Path $output 'settings.json';$report=Join-Path $output 'state.json';$a=Join-Path $fixtures 'focus.md'
$env:MDCARD_UI_REPORT=Join-Path $output 'ui.json'
function UiState {for($try=0;$try-lt20;$try++){try{return Get-Content -Raw -LiteralPath $env:MDCARD_UI_REPORT|ConvertFrom-Json}catch{Start-Sleep -Milliseconds 30}};return $null}
$original="# Focus verification`n`n中文选区 selection-token"
[IO.File]::WriteAllText($a,$original,$utf8)
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$priorClipboard=[Windows.Forms.Clipboard]::GetDataObject();$record=@{verification_id=[Guid]::NewGuid().ToString();stage=$Stage;binary_sha256=(Get-FileHash -LiteralPath $Exe).Hash;cases=@()}
try {
 StartReview;Click 450 250;Keys '^{END}';Keys '+{HOME}';Keys '^c'
 $expected=[Windows.Forms.Clipboard]::GetText();$record.expected_selection=$expected
 foreach($case in @('preferences','encoding','view-cycle','sidebar','resize')) {
  Click 450 250;Keys '^{END}';Keys '+{HOME}';Keys '^c'
  if([Windows.Forms.Clipboard]::GetText()-cne$expected){throw "Cannot establish initial selection for $case"}
  $beforeUi=UiState
  switch($case){
   'preferences' {Keys '^+p';Keys '{ESC}'}
   'encoding' {Keys '^+e';Keys '{ESC}'}
   'view-cycle' {Keys '^e';Keys '^e';Keys '^e'}
   'sidebar' {Keys '^b';Keys '^b'}
   'resize' {[void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,80,80,1000,750,$true);Start-Sleep -Milliseconds 300;[void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,50,50,1550,1025,$true);Start-Sleep -Milliseconds 300}
  }
  [Windows.Forms.Clipboard]::SetText('SENTINEL-NO-COPY');Keys '^c';$actual=[Windows.Forms.Clipboard]::GetText()
  $afterUi=UiState
  $record.cases+=@{name=$case;selected_text=$actual;selection_and_focus_preserved=($actual-ceq$expected);source_unchanged=((State).text-ceq$original);before_focus=$beforeUi.focused;after_focus=$afterUi.focused;before_selection=$beforeUi.selection;after_selection=$afterUi.selection}
  Shot $case
 }
 Keys '^q';$record.clean_exit=$script:app.WaitForExit(5000)
} catch {$record.error=$_.Exception.Message}
finally {
 if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()}
 if($priorClipboard){[Windows.Forms.Clipboard]::SetDataObject($priorClipboard,$true)}
 $record|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
}
$record|ConvertTo-Json -Depth 6
