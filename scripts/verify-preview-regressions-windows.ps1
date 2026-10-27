# SPDX-License-Identifier: AGPL-3.0-only
param([Parameter(Mandatory)][string]$Exe,[Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
. "$PSScriptRoot\recovery-json.ps1"
$output=[IO.Path]::GetFullPath($OutputDirectory)
if(Test-Path -LiteralPath $output){throw 'Use a new isolated regression directory'}
New-Item -ItemType Directory -Path $output|Out-Null
$settings=Join-Path $output 'settings.json'
$variables=@('MDCARD_UI_REPORT','MDCARD_SEARCH_REPORT','MDCARD_RICH_REPORT','MDCARD_READING_REPORT','MDCARD_READING_STATE_PATH','MDCARD_RECOVERY_DIR')
$previous=@{};foreach($name in $variables){$previous[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
$env:MDCARD_UI_REPORT=Join-Path $output 'ui.json'
$env:MDCARD_SEARCH_REPORT=Join-Path $output 'search.json'
$env:MDCARD_RICH_REPORT=Join-Path $output 'rich.json'
$env:MDCARD_READING_REPORT=Join-Path $output 'reading-report.json'
$env:MDCARD_READING_STATE_PATH=Join-Path $output 'reading-state.json'
$env:MDCARD_RECOVERY_DIR=Join-Path $output 'recovery'
$clipboard=[Windows.Forms.Clipboard]::GetDataObject()
$utf8=[Text.UTF8Encoding]::new($false)
function Report {ReadJsonStable $env:MDCARD_UI_REPORT @('controls','revision','recovery','autosave')}
function SearchReport {ReadJsonStable $env:MDCARD_SEARCH_REPORT @('controls','query','results')}
function Assert([bool]$Value,[string]$Message){if(!$Value){throw $Message}}
function WaitUntil([scriptblock]$Predicate,[string]$Message,[int]$Seconds=20){
 $end=[DateTime]::UtcNow.AddSeconds($Seconds)
 do{if(&$Predicate){return};Start-Sleep -Milliseconds 100}while([DateTime]::UtcNow-lt$end)
 throw $Message
}
function Control([string]$Name){
 $c=(Report).controls|Where-Object name -EQ $Name|Select-Object -First 1
 if(!$c){throw "Missing real UI control: $Name"}
 $l=[Math]::Max($c.rect[0],$c.clip[0]);$t=[Math]::Max($c.rect[1],$c.clip[1])
 $r=[Math]::Min($c.rect[2],$c.clip[2]);$b=[Math]::Min($c.rect[3],$c.clip[3])
 if($Name-eq'main-source-editor'){
  for($i=0;$i-lt4;$i++){Click ($l+40) ($t+15);if(((Report).controls|Where-Object name -EQ $Name|Select-Object -First 1).focus){return}}
  throw 'Source focus was not acquired'
 }
 Click (($l+$r)/2) (($t+$b)/2)
}
function Paste([string]$Value){Set-Clipboard -Value $Value;Keys '^v'}
function Checkpoint {
 WaitUntil {$r=Report;$r.recovery.main.current_version-and$r.recovery.main.committed_at-gt0-and!$r.recovery.main.working-and$null-eq$r.recovery.main.error} 'Recovery timer did not commit the current revision' 30
 return (Report).recovery.main
}
function RecoveryText([string]$Key){
 $dir=Join-Path $env:MDCARD_RECOVERY_DIR "records\$Key"
 $copies=foreach($name in @('0.json','1.json')){
  $p=Join-Path $dir $name
  if(Test-Path -LiteralPath $p){try{((Get-Content -Raw -Encoding utf8 -LiteralPath $p|ConvertFrom-Json).payload|ConvertFrom-Json)}catch{}}
 }
 return ($copies|Sort-Object sequence -Descending|Select-Object -First 1).text
}
try{
 @{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15;autosave=$true;recovery=$true}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
 $fixture=Join-Path $output 'regression.md'
 [IO.File]::WriteAllText($fixture,"# Preview regression`n`nOriginal paragraph.`n",$utf8)
 Launch 'first-launch' '' $fixture
 Assert ((Report).autosave.enabled-and(Report).recovery.enabled) 'Explicit autosave/recovery preferences were not applied'
 Control 'main-source-editor';Keys '^{END}';Paste "`nAUTO-TIMER 中文🙂."
 WaitUntil {([IO.File]::ReadAllText($fixture)).Contains('AUTO-TIMER')-and!(Report).dirty} 'Autosave stopped without the old global repaint timer'
 Shot '01-autosave-timer'
 Keys '^+p';Control 'Auto save';Keys '{ESC}'
 Assert (!(Report).autosave.enabled) 'Autosave could not be disabled explicitly'
 $diskBefore=(Get-FileHash -LiteralPath $fixture).Hash
 Control 'main-source-editor';Keys '^{END}';Paste "`nRECOVERY-ONLY 中文 retained."
 $checkpoint=Checkpoint
 Assert ((RecoveryText $checkpoint.key).Contains('RECOVERY-ONLY')) 'Recovery checkpoint omitted the latest edit'
 Assert ((Get-FileHash -LiteralPath $fixture).Hash-eq$diskBefore) 'Recovery changed original bytes while autosave was disabled'
 Shot '02-independent-recovery-timer'
 Keys '^s';WaitUntil {!(Report).dirty-and([IO.File]::ReadAllText($fixture)).Contains('RECOVERY-ONLY')} 'Manual save failed'
 Shot '03-manual-save'
 Keys '^f';Paste 'RECOVERY-ONLY'
 WaitUntil {(SearchReport).query-eq'RECOVERY-ONLY'-and(SearchReport).results-ge1-and!(SearchReport).query_pending} 'Find did not locate the latest saved text'
 $hit=(SearchReport).controls|Where-Object name -LIKE 'result-*'|Select-Object -First 1
 Assert ($null-ne$hit) 'Find result had no real UI control'
 Click (($hit.rect[0]+$hit.rect[2])/2) (($hit.rect[1]+$hit.rect[3])/2)
 Start-Sleep -Milliseconds 700;Keys '{ESC}'
 WaitUntil {$s=(Report).selection;$null-ne$s-and$s[0]-ne$s[1]} 'Find result did not select source text'
 Shot '04-search-source-navigation'
 Control 'main-source-editor';Keys '^{END}';Paste "`nCRASH-RECOVER-MARKER 中文🙂."
 $checkpoint=Checkpoint;$expected=RecoveryText $checkpoint.key
 Assert ($expected.Contains('CRASH-RECOVER-MARKER')) 'Latest crash checkpoint was not captured'
 $savedHash=(Get-FileHash -LiteralPath $fixture).Hash
 $script:app.Kill();$script:app.WaitForExit();Start-Sleep -Milliseconds 300
 Launch 'restart' '' $fixture
 WaitUntil {(Report).recovery.pending-ge1} 'Restart did not discover the committed recovery copy'
 $items=@((Report).recovery.items);$index=-1
 for($i=0;$i-lt$items.Count;$i++){if($items[$i].key-eq$checkpoint.key){$index=$i;break}}
 Assert ($index-ge0) 'Recovery identity changed across restart'
 if(!(Report).recovery.open){Control 'recovery-open'}
 Control "recovery-restore-$index"
 WaitUntil {(Report).recovery_origin-and(Report).dirty-and$null-eq(Report).document_path} 'Recovery did not open an independent dirty copy'
 Control 'Write';Control 'main-source-editor';Keys '^a';Keys '^c'
 $actual=[string](Get-Clipboard -Raw)
 Assert ($actual.Replace("`r`n","`n")-ceq$expected.Replace("`r`n","`n")) 'Visible recovered body differs from committed recovery text'
 Assert ((Get-FileHash -LiteralPath $fixture).Hash-eq$savedHash) 'Recovery overwrote the original file'
 Shot '05-exact-recovered-copy'
 @{date=[DateTimeOffset]::UtcNow.ToOffset([TimeSpan]::FromHours(9)).ToString('o');timezone='Asia/Tokyo';exe=(Resolve-Path -LiteralPath $Exe).Path;exe_sha256=(Get-FileHash -LiteralPath $Exe).Hash;real_mouse_keyboard=$true;autosave_timer=$true;recovery_timer_independent=$true;manual_save=$true;search_selects_source=$true;restart_discovers=$true;recovered_text_exact=$true;original_preserved=$true;recovery_key=$checkpoint.key;recovered_utf8_bytes=$utf8.GetByteCount($expected)}|ConvertTo-Json -Depth 4|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
}catch{
 if($script:app-and!$script:app.HasExited){Shot 'failure'}
 throw
}finally{
 if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()}
 if($null-ne$clipboard){[Windows.Forms.Clipboard]::SetDataObject($clipboard,$true)}
 foreach($name in $variables){[Environment]::SetEnvironmentVariable($name,$previous[$name],'Process')}
}
Write-Output $output
