# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot
$output=Join-Path $root 'evidence/rich/master-review/lifecycle'
New-Item -ItemType Directory -Force $output | Out-Null
$binary=(Resolve-Path -LiteralPath $Exe).Path
$realWorker=Join-Path $root 'renderer/worker.mjs'
$records=@()
function RunCase([string]$Name,[int]$QuitSeconds,[string]$Worker,[string]$Source) {
 $folder=Join-Path $output $Name
 New-Item -ItemType Directory -Force $folder | Out-Null
 [IO.File]::WriteAllText((Join-Path $folder 'worker.mjs'),$Worker,[Text.UTF8Encoding]::new($false))
 [IO.File]::WriteAllText((Join-Path $folder 'fixture.md'),$Source,[Text.UTF8Encoding]::new($false))
 [IO.File]::WriteAllText((Join-Path $folder 'settings.json'),'{"language":"en","appearance":"Light","gpu_acceleration":true,"reduce_motion":true,"font_size":15}',[Text.UTF8Encoding]::new($false))
 $start=[Diagnostics.ProcessStartInfo]::new($binary)
 $start.UseShellExecute=$false
 $start.ArgumentList.Add((Join-Path $folder 'fixture.md'))
 $start.Environment['MDCARD_SETTINGS_PATH']=Join-Path $folder 'settings.json'
 $start.Environment['MDCARD_RENDERER_DIR']=$folder
 $start.Environment['MDCARD_RICH_REPORT']=Join-Path $folder 'counters.json'
 $start.Environment['MDCARD_AUTO_QUIT_SECONDS']=[string]$QuitSeconds
 $timer=[Diagnostics.Stopwatch]::StartNew()
 $app=[Diagnostics.Process]::Start($start)
 try {
  if(!$app.WaitForExit(($QuitSeconds+8)*1000)){throw "Application exit hung during $Name"}
  $timer.Stop()
  if($app.ExitCode -ne 0){throw "Application failed during $Name with code $($app.ExitCode)"}
  $report=Get-Content -Raw -LiteralPath (Join-Path $folder 'counters.json') | ConvertFrom-Json
  $counters=$report.counters
  if($counters.submissions -lt 1){throw "The fake renderer was never exercised during $Name"}
  return @{name=$Name;exit_code=$app.ExitCode;elapsed_ms=$timer.Elapsed.TotalMilliseconds;counters=$counters}
 }finally{
  if(!$app.HasExited){& taskkill.exe /PID $app.Id /T /F | Out-Null}
  $app.Dispose()
 }
}
$large=('# Fault isolation' + "`n`n``````math`n" + ('x'*64000) + "`n```````n`n``````math`ny+1`n```````n")
$noRead='process.stdin.pause(); setInterval(()=>{},1000);'
$records+=RunCase 'blocked-stdin-close' 3 $noRead $large
$first=Join-Path $output 'restart-after-timeout/first-spawn.marker'
if(Test-Path -LiteralPath $first){Remove-Item -LiteralPath $first}
$workerJson=ConvertTo-Json -Compress $realWorker
$markerJson=ConvertTo-Json -Compress $first
$recover=@"
import fs from 'node:fs';
import {pathToFileURL} from 'node:url';
const marker=$markerJson;
if(!fs.existsSync(marker)){fs.writeFileSync(marker,'stalled');process.stdin.pause();setInterval(()=>{},1000);}
else{process.argv[1]=$workerJson;await import(pathToFileURL(process.argv[1]).href);}
"@
$restarted=RunCase 'restart-after-timeout' 27 $recover $large
if($restarted.counters.errors -ne 1){throw 'Expected one isolated timeout error and a successful next node'}
if($restarted.counters.completed -lt 2){throw 'The worker failed to process the next node after restart'}
$records+=$restarted
@{verification_id=[Guid]::NewGuid().ToString();date=[DateTimeOffset]::Now.ToString('o');exe_sha256=(Get-FileHash -LiteralPath $binary).Hash;passed=$true;cases=$records} | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
Write-Output 'Passed blocked-stdin shutdown and watchdog restart with subsequent valid rendering.'
