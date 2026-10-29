# SPDX-License-Identifier: AGPL-3.0-only
param([string]$Installer,[string]$InstallRoot,[string]$OutputDirectory)
$ErrorActionPreference='Stop';New-Item -ItemType Directory -Force $OutputDirectory|Out-Null
$uninstall=Join-Path $InstallRoot 'unins000.exe'
if(Test-Path -LiteralPath $uninstall){$p=Start-Process -FilePath $uninstall -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART') -WindowStyle Hidden -Wait -PassThru;if($p.ExitCode-ne0){throw 'Previous QA installation did not uninstall'}}
$samples=@()
foreach($trial in 1..3){
 $clock=[Diagnostics.Stopwatch]::StartNew()
 $p=Start-Process -FilePath $Installer -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/TASKS=""',('/DIR="'+$InstallRoot+'"'),('/LOG="'+(Join-Path $OutputDirectory ('install-'+$trial+'.log'))+'"')) -WindowStyle Hidden -Wait -PassThru
 $clock.Stop();if($p.ExitCode-ne0){throw "Installer trial $trial failed: $($p.ExitCode)"}
 $samples+=@{trial=$trial;seconds=$clock.Elapsed.TotalSeconds;exitCode=$p.ExitCode}
 @{installer=$Installer;installerSHA256=(Get-FileHash -LiteralPath $Installer).Hash;installerBytes=(Get-Item -LiteralPath $Installer).Length;installRoot=$InstallRoot;trials=$samples;cache='Warm OS page cache; fresh app destination each trial; no concurrent build/compression/VM';measurement='Silent machine wait, separate from interactive wizard actions'}|ConvertTo-Json -Depth 6|Set-Content (Join-Path $OutputDirectory 'measurements.json') -Encoding utf8
 Write-Output "Final installer trial $trial complete: $($clock.Elapsed.TotalSeconds) seconds"
 if($trial-lt3){$p=Start-Process -FilePath $uninstall -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART') -WindowStyle Hidden -Wait -PassThru;if($p.ExitCode-ne0){throw 'Between-trial uninstall failed'}}
}
$exe=Join-Path $InstallRoot 'MDcard.exe';$manifest=Get-Content -Raw (Join-Path $InstallRoot 'manifest.json')|ConvertFrom-Json;$errors=@()
foreach($entry in $manifest.files){$f=Join-Path $InstallRoot $entry.path;if(!(Test-Path -LiteralPath $f)-or(Get-FileHash -LiteralPath $f).Hash.ToLowerInvariant()-ne$entry.sha256){$errors+=$entry.path}}
$logical=(@(Get-ChildItem -LiteralPath $InstallRoot -Recurse -File)|Measure-Object Length -Sum).Sum
@{binarySHA256=(Get-FileHash -LiteralPath $exe).Hash;productVersion=(Get-Item -LiteralPath $exe).VersionInfo.ProductVersion;sourceCommit=$manifest.productSourceCommit;manifestSHA256=(Get-FileHash -LiteralPath (Join-Path $InstallRoot 'manifest.json')).Hash;payloadFiles=$manifest.files.Count;integrityErrors=$errors;logicalInstalledBytes=$logical;includesInstallerUninstallFiles=$true}|ConvertTo-Json -Depth 5|Set-Content (Join-Path $OutputDirectory 'installed-integrity.json') -Encoding utf8
if($errors.Count){throw 'Final installed payload checksum mismatch'}
