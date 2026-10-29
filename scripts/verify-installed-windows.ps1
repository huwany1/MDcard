# SPDX-License-Identifier: AGPL-3.0-only
# Actual installed/native GUI path plus a byte-identical portable baseline.
param([string]$InstalledRoot,[string]$BaselineRoot,[string]$OutputDirectory,[string[]]$Kinds=@('baseline','installed'),[int]$FirstTrial=1,[int]$LastTrial=3)
$ErrorActionPreference='Stop'
$Exe=Join-Path $InstalledRoot 'MDcard.exe'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
. "$PSScriptRoot\recovery-json.ps1"
$resultRoot=[IO.Path]::GetFullPath($OutputDirectory);New-Item -ItemType Directory -Force $resultRoot|Out-Null
$env:MDCARD_QA_CLEAN_ENV='1'
function Report {ReadJsonStable $env:MDCARD_UI_REPORT @('controls','document_path')}
function Control([string]$Name) {
 $c=(Report).controls|Where-Object name -eq $Name|Select-Object -First 1
 if(!$c){throw "No control $Name"}
 $l=[Math]::Max($c.rect[0],$c.clip[0]);$t=[Math]::Max($c.rect[1],$c.clip[1]);$r=[Math]::Min($c.rect[2],$c.clip[2]);$b=[Math]::Min($c.rect[3],$c.clip[3])
 if($Name-eq'main-source-editor'){
  for($attempt=0;$attempt-lt4;$attempt++){Click ($l+40) ($t+14);$focused=(Report).controls|Where-Object name -eq $Name|Select-Object -First 1;if($focused.focus){return};Start-Sleep -Milliseconds 200};throw 'Source did not receive focus'
 }else{Click (($l+$r)/2) (($t+$b)/2)}
}
function WaitFor([scriptblock]$Condition,[string]$Reason,[int]$Seconds=40){$end=[DateTime]::UtcNow.AddSeconds($Seconds);while([DateTime]::UtcNow-lt$end){if(&$Condition){return};Start-Sleep -Milliseconds 50};throw $Reason}
function Paste([string]$Text){Set-Clipboard -Value $Text;Start-Sleep -Milliseconds 100;Keys '^v'}
$clipboard=[Windows.Forms.Clipboard]::GetDataObject();$records=@()
try {
 foreach($kind in $Kinds) {foreach($trial in $FirstTrial..$LastTrial){
  $Exe=Join-Path $(if($kind-eq'baseline'){$BaselineRoot}else{$InstalledRoot}) 'MDcard.exe'
  $output=Join-Path $resultRoot "$kind-$trial";New-Item -ItemType Directory -Force $output|Out-Null
  $settings=Join-Path $output 'settings.json';@{language='en';appearance='Light';reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content $settings -Encoding utf8
  foreach($name in @('UI','SEARCH','RICH','EXPORT')){[Environment]::SetEnvironmentVariable("MDCARD_${name}_REPORT",(Join-Path $output ($name.ToLower()+'.json')),'Process')}
  $env:MDCARD_READING_STATE_PATH=Join-Path $output 'reading.json';$env:MDCARD_RECOVERY_DIR=Join-Path $output 'recovery'
  $env:MDCARD_QA_STARTUP_TIMING_PATH=Join-Path $output 'startup.json'
  $document=Join-Path $output '中文 场景A.md';[IO.File]::WriteAllText($document,"# First use`n`nInitial.`n",[Text.UTF8Encoding]::new($false))
  $clock=[Diagnostics.Stopwatch]::StartNew();Launch 'renderer' '' $document;Start-Sleep -Milliseconds 500
  Control 'Write';Control 'main-source-editor';Keys '^{END}';Paste "`nEdited 中文 and English.";Keys '^s'
  Shot 'debug-after-paste-save'
  WaitFor {([IO.File]::ReadAllText($document)).Contains('Edited 中文 and English.')-and!(Report).dirty} 'Scene A did not save'
  $aSeconds=$clock.Elapsed.TotalSeconds;Shot '01-edited-saved'
  Keys '^f';Paste 'Edited';WaitFor {(ReadJsonStable $env:MDCARD_SEARCH_REPORT @('results')).results-gt0} 'Search did not find edited text';Keys '{ESC}'
  Control 'Write';Control 'main-source-editor';Keys '^a'
  $full="# Full offline use`n`n`$x^2+1`$`n`n``````mermaid`ngraph LR`n A --> B`n```````n`n``````plantuml`nAlice -> Bob: 中文`n```````n`n``````flow`nst=>start: Start`ne=>end: End`nst->e`n```````n"
  $fullStart=$clock.Elapsed.TotalSeconds;Paste $full;Keys '^s';Control 'Split'
  WaitFor {(ReadJsonStable $env:MDCARD_RICH_REPORT @('cachedResults')).cachedResults-ge4} 'Full preview did not render all 4 local engine kinds' 60
  $previewReady=$clock.Elapsed.TotalSeconds-$fullStart;Shot '02-local-preview'
  Keys '^%e';WaitFor {(ReadJsonStable $env:MDCARD_EXPORT_REPORT @('open')).open} 'Native export dialog did not open'
  $r=(ReadJsonStable $env:MDCARD_EXPORT_REPORT @('controls')).controls.export;Click ($r.x+$r.w/2) ($r.y+$r.h/2)
  Start-Sleep -Milliseconds 600;Keys '%n';$pdf=Join-Path $output '中文 完整导出.pdf';Paste $pdf;Keys '{ENTER}'
  WaitFor {(ReadJsonStable $env:MDCARD_EXPORT_REPORT @('last')).last.type-eq'complete'} 'Native GUI PDF export did not complete' 60
  if(!([IO.File]::ReadAllBytes($pdf)[0..4]-join',').StartsWith('37,80,68,70,45')){throw 'GUI export did not create a PDF'}
  $bAdditional=$clock.Elapsed.TotalSeconds-$fullStart;Shot '03-native-pdf-export'
  $startup=Get-Content -Raw $env:MDCARD_QA_STARTUP_TIMING_PATH|ConvertFrom-Json
  $records+=@{kind=$kind;trial=$trial;binary=(Resolve-Path $Exe).Path;sha256=(Get-FileHash $Exe).Hash;startupSeconds=$startup.process_to_native_window_and_first_report_seconds;scenarioAProcessToSavedSeconds=$aSeconds;scenarioBAdditionalSeconds=$bAdditional;previewReadySeconds=$previewReady;guiPdf=$pdf;searchPassed=$true;chineseInput='Unicode clipboard paste; real IME separately required';environment='Only System32 PATH; installed runtime resolution; isolated test profile; host OS has developer tools'}
  Keys '{ESC}';Keys '^q';if(!$script:app.WaitForExit(5000)){throw 'GUI did not close cleanly'}
  $records|ConvertTo-Json -Depth 7|Set-Content (Join-Path $resultRoot 'measurements.json') -Encoding utf8
 }}
} finally {if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()};if($null-ne$clipboard){[Windows.Forms.Clipboard]::SetDataObject($clipboard,$true)}}
