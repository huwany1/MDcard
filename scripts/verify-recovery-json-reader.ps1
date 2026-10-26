# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$OutputDirectory="$PSScriptRoot\..\evidence\recovery\2026-10-26\json-reader")
$ErrorActionPreference='Stop'
. "$PSScriptRoot\recovery-json.ps1"
New-Item -ItemType Directory -Force $OutputDirectory|Out-Null
$writer=Join-Path $OutputDirectory 'writer.py';$json=Join-Path $OutputDirectory 'racing.json'
@'
import json,sys,time
payload=json.dumps({'controls':[],'recovery':{'items':[]},'revision':1,'padding':'x'*16384})
for _ in range(800):
    with open(sys.argv[1],'w',encoding='utf-8') as f:
        time.sleep(.001)
        f.write(payload)
        f.flush()
    time.sleep(.001)
'@|Set-Content -LiteralPath $writer -Encoding utf8
$process=Start-Process -FilePath (Get-Command python).Source -ArgumentList @($writer,$json) -WindowStyle Hidden -PassThru
$nullReads=0;$invalid=0;$valid=0
try {
 for($i=0;$i-lt400;$i++){
  try{$v=Get-Content -LiteralPath $json -Raw -ErrorAction Stop|ConvertFrom-Json -ErrorAction Stop;if($null-eq$v){$nullReads++}else{$valid++}}catch{$invalid++}
  Start-Sleep -Milliseconds 1
 }
 for($i=0;$i-lt200;$i++){$v=ReadJsonStable $json @('controls','recovery','revision');if($v.revision-ne1){throw 'Stable reader returned an invalid state'}}
 $process.WaitForExit();if($process.ExitCode-ne0){throw 'JSON writer failed'}
 if($nullReads-eq0-and$invalid-eq0){throw 'The controlled truncate/write race was not reproduced'}
 @{date=[DateTimeOffset]::Now.ToString('o');naive_null_reads=$nullReads;naive_invalid_reads=$invalid;naive_valid_reads=$valid;stable_validated_reads=200;mechanism='Controlled truncate-before-write interval, matching std::fs::write diagnostics';source='src/diagnostic_sink.rs non-Linux write branch'}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $OutputDirectory 'result.json') -Encoding utf8
}finally{if(!$process.HasExited){$process.Kill();$process.WaitForExit()}}
