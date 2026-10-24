# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
 [Parameter(Mandatory=$true)][string]$Candidate,
 [Parameter(Mandatory=$true)][string]$Stage,
 [string]$Baseline="$PSScriptRoot/../artifacts/reading-precision/entry-baseline-264d42d/MDcard.exe"
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if($Stage-notmatch'^[A-Za-z0-9_-]+$'){throw 'Use a simple fresh stage'}
$out=Join-Path $root "artifacts/reading-precision/$Stage"
if(Test-Path -LiteralPath $out){throw 'Retain previous measurements; use a fresh stage'}
New-Item -ItemType Directory $out|Out-Null
$driver=Join-Path $PSScriptRoot 'measure-reading-precision-master-windows.ps1'
$scorer=Join-Path $PSScriptRoot 'summarize-reading-precision-master-performance.py'
$executables=@{baseline=(Resolve-Path -LiteralPath $Baseline).Path;candidate=(Resolve-Path -LiteralPath $Candidate).Path}
$plan=@{stage=$Stage;created_utc=[DateTimeOffset]::UtcNow.ToString('o');
 budget='Frozen complete App::update all/wheel invocation P95 increase <=max(20% original P95,2ms); Off and On separate. No other GUI/build/benchmark workload during sampling.';
 runner_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant();driver_sha256=(Get-FileHash -LiteralPath $driver).Hash.ToLowerInvariant();scorer_sha256=(Get-FileHash -LiteralPath $scorer).Hash.ToLowerInvariant();
 binaries=@{};inputs=@{};order=@()}
foreach($role in @('baseline','candidate')){$plan.binaries[$role]=@{path=$executables[$role];sha256=(Get-FileHash -LiteralPath $executables[$role]).Hash.ToLowerInvariant()}}
foreach($file in Get-ChildItem -LiteralPath (Join-Path $root 'artifacts/reading-precision/performance-inputs') -File){$plan.inputs[$file.Name]=(Get-FileHash -LiteralPath $file.FullName).Hash.ToLowerInvariant()}
foreach($diag in @('Off','On')){foreach($size in @('ordinary','one-mib','ten-mib')){foreach($role in @('baseline','candidate')){$plan.order+=,"$($diag.ToLower())-$role-$size"}}}
$plan|ConvertTo-Json -Depth 12|Set-Content -LiteralPath (Join-Path $out 'plan-before-first-measurement.json') -Encoding utf8
$status=@();$comparisons=@()
foreach($diag in @('Off','On')){
 foreach($size in @('ordinary','one-mib','ten-mib')){
  foreach($role in @('baseline','candidate')){
   $run="$Stage-$($diag.ToLower())-$role-$size"
   $started=[DateTimeOffset]::UtcNow.ToString('o')
   & pwsh -NoProfile -ExecutionPolicy Bypass -File $driver -Exe $executables[$role] -Stage $run -Size $size -Diagnostics $diag *> (Join-Path $out "$run.log")
   $item=@{run=$run;started_utc=$started;finished_utc=[DateTimeOffset]::UtcNow.ToString('o');exit_code=$LASTEXITCODE}
   $status+=,$item;$status|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $out 'execution-status.json') -Encoding utf8
   $item|ConvertTo-Json -Compress|Write-Output
  }
  $comparison=Join-Path $out "$($diag.ToLower())-$size-comparison.json"
  & python $scorer --baseline (Join-Path $root "evidence/reading-precision/master/$Stage-$($diag.ToLower())-baseline-$size") --candidate (Join-Path $root "evidence/reading-precision/master/$Stage-$($diag.ToLower())-candidate-$size") --output $comparison *> (Join-Path $out "$($diag.ToLower())-$size-comparison.log")
  $comparisons+=,@{diagnostics=$diag;size=$size;exit_code=$LASTEXITCODE;path=$comparison}
  $comparisons|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $out 'comparisons-status.json') -Encoding utf8
 }
}
if(@($status|Where-Object {$_.exit_code-ne0}).Count-or@($comparisons|Where-Object {$_.exit_code-ne0}).Count){exit 1}
