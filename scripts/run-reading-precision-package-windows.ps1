# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
 [Parameter(Mandatory=$true)][string]$PackageDirectory,
 [Parameter(Mandatory=$true)][string]$ExpectedBinarySha256,
 [Parameter(Mandatory=$true)][string]$Plan,
 [Parameter(Mandatory=$true)][string]$Stage
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if($Stage-notmatch'^[A-Za-z0-9_-]+$'){throw 'Use a fresh simple stage'}
$pkg=(Resolve-Path -LiteralPath $PackageDirectory).Path
if(!$pkg.StartsWith($root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Use a task-owned extracted package inside the workspace'}
$exe=Join-Path $pkg 'MDcard.exe'
$renderer=Join-Path $pkg 'renderer'
$required=@($exe,(Join-Path $pkg 'manifest.json'),(Join-Path $renderer 'worker.mjs'),(Join-Path $renderer 'runtime/node.exe'))
foreach($path in $required){if(!(Test-Path -LiteralPath $path -PathType Leaf)){throw "Incomplete extracted package: $path"}}
if(!(Test-Path -LiteralPath (Join-Path $renderer 'vendor') -PathType Container)){throw 'Extracted package vendor is missing'}
if((Get-FileHash -LiteralPath $exe).Hash.ToLowerInvariant()-cne$ExpectedBinarySha256.ToLowerInvariant()){throw 'Package executable differs from the accepted candidate'}
$out=Join-Path $root "artifacts/reading-precision/master/$Stage"
if(Test-Path -LiteralPath $out){throw 'Retain previous package GUI evidence; choose a fresh stage'}
New-Item -ItemType Directory $out|Out-Null
$inputPlan=Get-Content -LiteralPath $Plan -Raw -Encoding utf8|ConvertFrom-Json -AsHashtable
$jobs=@()
foreach($job in $inputPlan.parent_windows_known_regression_rechecks){
 $parameters=$job.arguments.Clone()
 if($parameters.Exe-cne'${WINPKG}/MDcard.exe'){throw 'Only the frozen package-executable placeholder is accepted'}
 $parameters.Exe=$exe
 $mode=if($parameters.ContainsKey('Mode')){$parameters.Mode.ToLowerInvariant()}else{'read'}
 $result=if($job.script-like'*secondary-windows.ps1'){
  "evidence/reading-precision/master/$($parameters.Stage)-lifecycle-write/result.json"
 }elseif($job.script-like'*master-windows.ps1'){
  "evidence/reading-precision/master/$($parameters.Stage)-$($parameters.Scenario)-$mode/result.json"
 }else{"evidence/reading-precision/master/$($parameters.Stage)-lifecycle-$mode/result.json"}
 if(Test-Path -LiteralPath (Join-Path $root $result)){throw "A frozen package scenario already has evidence: $result"}
 $jobs+=,@{name=$parameters.Stage;script=$job.script;parameters=$parameters;result=$result}
}
if($jobs.Count-ne8){throw 'The reviewed Windows package matrix must retain all eight groups'}
$driverCatalog=Get-Content -LiteralPath (Join-Path $root 'artifacts/reading-precision/master/r25-windows-driver-snapshot/manifest.json') -Raw -Encoding utf8|ConvertFrom-Json
$drivers=@()
foreach($entry in $driverCatalog.scripts){
 $source=Join-Path $root $entry.path
 $snapshot=Join-Path $out ("drivers/"+$entry.path)
 New-Item -ItemType Directory -Force (Split-Path $snapshot -Parent)|Out-Null
 Copy-Item -LiteralPath $source -Destination $snapshot
 $drivers+=,@{path=$entry.path;sha256=(Get-FileHash -LiteralPath $source).Hash.ToLowerInvariant();snapshot=$snapshot}
}
$before=@{}
foreach($path in $required){$before[$path]=(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()}
$planPath=Join-Path $out 'plan-before-execution.json'
@{created_utc=[DateTimeOffset]::UtcNow.ToString('o');package=$pkg;binary=$exe;binary_sha256=$ExpectedBinarySha256.ToLowerInvariant();renderer=$renderer;
 source_plan=(Resolve-Path -LiteralPath $Plan).Path;source_plan_sha256=(Get-FileHash -LiteralPath $Plan).Hash.ToLowerInvariant();
 runner_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant();required_before=$before;drivers=$drivers;jobs=$jobs}|ConvertTo-Json -Depth 20|Set-Content -LiteralPath $planPath -Encoding utf8
$status=@()
for($index=0;$index-lt$jobs.Count;$index++){
 $job=$jobs[$index];$child=Join-Path $out "job-$index.ps1"
 @'
param([string]$Plan,[int]$Index,[string]$Root)
$ErrorActionPreference='Stop'
$p=Get-Content -LiteralPath $Plan -Raw -Encoding utf8|ConvertFrom-Json -AsHashtable
foreach($entry in $p.drivers){if((Get-FileHash -LiteralPath (Join-Path $Root $entry.path)).Hash.ToLowerInvariant()-cne$entry.sha256){throw 'A package driver changed after freeze'}}
$job=$p.jobs[$Index];$parameters=$job.parameters
& (Join-Path $Root $job.script) @parameters
exit $LASTEXITCODE
'@|Set-Content -LiteralPath $child -Encoding utf8
 $started=[DateTimeOffset]::UtcNow.ToString('o')
 & pwsh -NoProfile -ExecutionPolicy Bypass -File $child -Plan $planPath -Index $index -Root $root *> (Join-Path $out "$($job.name).log")
 $code=$LASTEXITCODE;$path=Join-Path $root $job.result
 $report=if(Test-Path -LiteralPath $path){Get-Content -LiteralPath $path -Raw -Encoding utf8|ConvertFrom-Json}else{$null}
 $valid=$code-eq0-and$report.passed-and$report.binary_sha256.ToLowerInvariant()-ceq$ExpectedBinarySha256.ToLowerInvariant()
 if($valid){
  $launches=@($report.observation_launches)
  $valid=$launches.Count-gt0
  foreach($launch in $launches){
   if(![string]::Equals($launch.renderer_dir,$renderer,[StringComparison]::OrdinalIgnoreCase)){$valid=$false}
  }
 }
 $item=@{name=$job.name;started_utc=$started;finished_utc=[DateTimeOffset]::UtcNow.ToString('o');exit_code=$code;passed=[bool]$valid;
 result=$job.result;result_sha256=if(Test-Path -LiteralPath $path){(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()}else{$null}}
 $status+=,$item;$status|ConvertTo-Json -Depth 10|Set-Content -LiteralPath (Join-Path $out 'execution-status.json') -Encoding utf8
 $item|ConvertTo-Json -Compress|Write-Output
}
foreach($path in $required){if((Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()-cne$before[$path]){throw 'A required package resource changed during GUI verification'}}
if(@($status|Where-Object {!$_.passed}).Count){exit 1}
