# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
 [Parameter(Mandatory=$true)][string]$Exe,
 [string]$Stage='master-final-r8'
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$batch=Join-Path $root "artifacts/reading-state/$Stage-batch.json"
if(Test-Path -LiteralPath $batch){throw 'Use a fresh Stage to preserve previous evidence'}
$results=@()
foreach($scenario in @('same-file','priorities','capacity','core','routes','clear','rename','source','reflow','guards','secondary','formats','content','corrupt','failure','crash','atomic','semantics')){
 $caseStage=if($scenario-eq'core'){$Stage}else{"$Stage-$scenario"}
 $log=Join-Path $root "artifacts/reading-state/$caseStage-driver.log"
 if($scenario-eq'core'){
  & pwsh -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot/verify-reading-state-master-windows.ps1" -Exe $Exe -Stage $Stage *> $log
 }else{
  & pwsh -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot/verify-reading-state-master-matrix-windows.ps1" -Exe $Exe -Stage $Stage -Scenario $scenario *> $log
 }
 $exitCode=$LASTEXITCODE
 $result="evidence/reading-state/master/$caseStage/result.json"
 $report=if(Test-Path -LiteralPath (Join-Path $root $result)){Get-Content -LiteralPath (Join-Path $root $result) -Raw|ConvertFrom-Json}else{$null}
 $item=@{scenario=$scenario;passed=($exitCode-eq0-and$report.passed);exit=$exitCode;result=$result;error=$report.error;binary_sha256=$report.binary_sha256}
 $results+=,$item
 $results|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $batch -Encoding utf8
 $item|ConvertTo-Json -Compress|Write-Output
}
if(@($results|Where-Object {!$_.passed}).Count-gt0){exit 1}
