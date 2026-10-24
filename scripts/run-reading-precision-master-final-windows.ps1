# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
 [Parameter(Mandatory=$true)][string]$Exe,
 [Parameter(Mandatory=$true)][string]$Stage,
 [ValidateSet('All','Precision','Regression')][string]$Selection='All',
 [string]$VerifiedSecondary,
 [string]$VerifiedInApp
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if($Stage-notmatch'^[A-Za-z0-9_-]+$'){throw 'Use a simple fresh stage'}
$batch=Join-Path $root "artifacts/reading-precision/master/$Stage-batch"
$planPath=Join-Path $batch 'plan-before-execution.json'
if(Test-Path -LiteralPath $batch){throw 'Retain prior attempts; use a fresh stage'}
New-Item -ItemType Directory $batch|Out-Null
$candidateSha=(Get-FileHash -LiteralPath $Exe).Hash.ToLowerInvariant()
$prior=@()
foreach($entry in @(
 @{result=$VerifiedSecondary;driver='scripts/verify-reading-precision-master-secondary-windows.ps1';kind='secondary'},
 @{result=$VerifiedInApp;driver='scripts/verify-reading-precision-master-inapp-windows.ps1';kind='inapp'}
)){
 if(!$entry.result){continue}
 $report=Get-Content -LiteralPath $entry.result -Raw -Encoding utf8|ConvertFrom-Json
 $driverSha=(Get-FileHash -LiteralPath (Join-Path $root $entry.driver)).Hash.ToLowerInvariant()
 if(!$report.passed-or$report.binary_sha256.ToLowerInvariant()-cne$candidateSha-or$report.harness_sha256-cne$driverSha){throw 'Prior GUI evidence must pass on this exact binary with the current driver'}
 if($entry.kind-eq'secondary'-and$report.scenario-ne'All'){throw 'Prior secondary evidence must cover all four formats and both same-file buffers'}
 if($entry.kind-eq'inapp'-and$null-eq$report.original_regression_target_source_byte){throw 'Prior in-app evidence must cover the original failing heading'}
 $prior+=,@{kind=$entry.kind;result=$entry.result;result_sha256=(Get-FileHash -LiteralPath $entry.result).Hash.ToLowerInvariant();binary_sha256=$candidateSha;driver_sha256=$driverSha;started_utc=$report.started;finished_utc=$report.finished;assertions=@($report.cases).Count}
}
$jobs=[Collections.Generic.List[object]]::new()
function AddJob([string]$Name,[string]$Script,[hashtable]$Parameters,[string]$Result){
 $jobs.Add(@{name=$Name;script=$Script;parameters=$Parameters;result=$Result})
}
if($Selection-in@('All','Precision')){
 if(!$VerifiedSecondary){
  $prefix="$Stage-secondary"
  AddJob $prefix 'scripts/verify-reading-precision-master-secondary-windows.ps1' @{Exe=$Exe;Stage=$prefix} "evidence/reading-precision/master/$prefix-lifecycle-write/result.json"
 }
 foreach($mode in @('Read','Write')){
  foreach($scenario in @('reflow','lifecycle')){
   $prefix="$Stage-full";$name="$prefix-$scenario-$($mode.ToLower())"
   AddJob $name 'scripts/verify-reading-precision-master-windows.ps1' @{Exe=$Exe;Stage=$prefix;Scenario=$scenario;Mode=$mode} "evidence/reading-precision/master/$name/result.json"
  }
 }
 foreach($case in @(
  @{suffix='mega-middle';mode='Read';region='Middle';document='mega-paragraph.md'},
  @{suffix='mega-late';mode='Write';region='Late';document='mega-line.txt'},
  @{suffix='utf16';mode='Read';region='Start';document='utf16.txt'},
  @{suffix='gb18030';mode='Write';region='Start';document='gb18030.txt'}
 )){
  $prefix="$Stage-$($case.suffix)";$name="$prefix-reflow-$($case.mode.ToLower())"
  AddJob $name 'scripts/verify-reading-precision-master-windows.ps1' @{Exe=$Exe;Stage=$prefix;Scenario='reflow';Mode=$case.mode;TargetRegion=$case.region;Documents=@($case.document)} "evidence/reading-precision/master/$name/result.json"
 }
 foreach($document in @('mapping.md','mapping-large.md')){
  $prefix="$Stage-$($document.Replace('.md',''))";$name="$prefix-mapping-read"
  AddJob $name 'scripts/verify-reading-precision-master-windows.ps1' @{Exe=$Exe;Stage=$prefix;Scenario='mapping';Mode='Read';Documents=@($document)} "evidence/reading-precision/master/$name/result.json"
 }
 $prefix="$Stage-boundary";$name="$prefix-boundaries-read"
 AddJob $name 'scripts/verify-reading-precision-master-windows.ps1' @{Exe=$Exe;Stage=$prefix;Scenario='boundaries';Mode='Read'} "evidence/reading-precision/master/$name/result.json"
 foreach($case in @(
  @{suffix='md';mode='Read';document='precision.md'},
  @{suffix='txt';mode='Write';document='precision.txt'},
  @{suffix='large-md';mode='Write';document='large.md'},
  @{suffix='large-txt';mode='Read';document='large.txt'}
 )){
  $prefix="$Stage-external-$($case.suffix)";$name="$prefix-lifecycle-$($case.mode.ToLower())"
  AddJob $name 'scripts/verify-reading-precision-master-relocation-windows.ps1' @{Exe=$Exe;Stage=$prefix;Mode=$case.mode;BaseDocument=$case.document} "evidence/reading-precision/master/$name/result.json"
 }
 $prefix="$Stage-original-tail"
 AddJob $prefix 'scripts/verify-reading-precision-master-relocation-windows.ps1' @{Exe=$Exe;Stage=$prefix;Mode='Read';BaseDocument='precision.md';Kinds=@('split-merge');TargetSourceByte=40557} "evidence/reading-precision/master/$prefix-lifecycle-read/result.json"
 if(!$VerifiedInApp){
  $prefix="$Stage-inapp"
  AddJob $prefix 'scripts/verify-reading-precision-master-inapp-windows.ps1' @{Exe=$Exe;Stage=$prefix;TargetHeadingRegression=$true} "evidence/reading-precision/master/$prefix-lifecycle-read/result.json"
 }
}
if($Selection-in@('All','Regression')){
 $prefix="$Stage-state"
 AddJob $prefix 'scripts/verify-reading-precision-master-state-windows.ps1' @{Exe=$Exe;Stage=$prefix} "evidence/reading-state/master/$prefix/result.json"
 $prefix="$Stage-future-clear"
 AddJob $prefix 'scripts/verify-reading-precision-master-future-clear-windows.ps1' @{Exe=$Exe;Stage=$prefix} "evidence/reading-state/master/$prefix/result.json"
 $prefix="$Stage-worker"
 AddJob $prefix 'scripts/verify-reading-precision-master-worker-windows.ps1' @{Exe=$Exe;Stage=$prefix} "evidence/reading-state/master/$prefix/result.json"
 $prefix="$Stage-legacy"
 AddJob $prefix 'scripts/verify-reading-state-master-all-windows.ps1' @{Exe=$Exe;Stage=$prefix} "artifacts/reading-state/$prefix-batch.json"
 $prefix="$Stage-search"
 AddJob $prefix 'scripts/verify-search-master-windows.ps1' @{Exe=$Exe;ReviewStage=$prefix} "evidence/search/$prefix/windows/result.json"
}
$plan=@{stage=$Stage;created_utc=[DateTimeOffset]::UtcNow.ToString('o');selection=$Selection;
 binary=(Resolve-Path -LiteralPath $Exe).Path;binary_sha256=$candidateSha;reused_current_binary_gui_results=$prior;
 runner_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant();jobs=$jobs.ToArray()}
$plan|ConvertTo-Json -Depth 15|Set-Content -LiteralPath $planPath -Encoding utf8
$results=@()
for($index=0;$index-lt$jobs.Count;$index++){
 $job=$jobs[$index];$started=[DateTimeOffset]::UtcNow.ToString('o')
 # Each child receives the frozen structured parameters through its own file,
 # preserving arrays and literal Unicode without shell-built command strings.
 $child=Join-Path $batch "job-$index.ps1"
 @'
param([string]$Plan,[int]$Index,[string]$Root)
$ErrorActionPreference='Stop'
$p=Get-Content -LiteralPath $Plan -Raw -Encoding utf8|ConvertFrom-Json -AsHashtable
if((Get-FileHash -LiteralPath $p.binary).Hash.ToLowerInvariant()-cne$p.binary_sha256){throw 'Candidate changed after plan freeze'}
$job=$p.jobs[$Index];$parameters=$job.parameters
& (Join-Path $Root $job.script) @parameters
exit $LASTEXITCODE
'@|Set-Content -LiteralPath $child -Encoding utf8
 & pwsh -NoProfile -ExecutionPolicy Bypass -File $child -Plan $planPath -Index $index -Root $root *> (Join-Path $batch "$($job.name).log")
 $code=$LASTEXITCODE;$path=Join-Path $root $job.result;$report=$null
 if(Test-Path -LiteralPath $path){$report=Get-Content -LiteralPath $path -Raw -Encoding utf8|ConvertFrom-Json}
 $item=@{name=$job.name;started_utc=$started;finished_utc=[DateTimeOffset]::UtcNow.ToString('o');exit_code=$code;result=$job.result;result_sha256=if(Test-Path -LiteralPath $path){(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()}else{$null}}
 $item.passed=($code-eq0-and$null-ne$report-and($report.passed-or($job.script-like'*master-all-windows.ps1'-and@($report|Where-Object {!$_.passed}).Count-eq0)))
 $results+=,$item;$results|ConvertTo-Json -Depth 10|Set-Content -LiteralPath (Join-Path $batch 'execution-status.json') -Encoding utf8
 $item|ConvertTo-Json -Compress|Write-Output
}
if(@($results|Where-Object {!$_.passed}).Count-gt0){exit 1}
