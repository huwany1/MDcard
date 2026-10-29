# SPDX-License-Identifier: AGPL-3.0-only
$ErrorActionPreference='Stop';$repo=Split-Path $PSScriptRoot;$pwsh=(Get-Command pwsh).Source
$setup=Join-Path $repo 'artifacts/release-015/final-packages/MDcard-0.1.5-windows-x64-setup.exe'
$root=Join-Path $env:TEMP 'MDcard-0.1.5-stale-backup-no-exe';$backup=$root+'.mdcard-rollback'
if((Test-Path $root)-or(Test-Path $backup)){throw 'Inspect existing stale-backup QA fixture'}
New-Item -ItemType Directory -Path $backup|Out-Null
[IO.File]::WriteAllText((Join-Path $backup 'RECOVERY.txt'),'KEEP QA prior interrupted installation backup')
$log=Join-Path $repo 'evidence/release-015/windows-stale-backup-no-exe.log'
$p=Start-Process -FilePath $setup -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART',('/DIR="'+$root+'"'),('/LOG="'+$log+'"')) -WindowStyle Hidden -Wait -PassThru
$blocked=(Get-Content -Raw $log).Contains('A previous interrupted upgrade backup exists.')
@{exitCode=$p.ExitCode;blockedSpecificallyForBackup=$blocked;oldExecutableAbsent=$true;newExecutableNotWritten=!(Test-Path (Join-Path $root 'MDcard.exe'));backupMarkerPreserved=([IO.File]::ReadAllText((Join-Path $backup 'RECOVERY.txt'))-eq'KEEP QA prior interrupted installation backup');qaFixtureRetained=$backup}|ConvertTo-Json|Set-Content (Join-Path $repo 'evidence/release-015/windows-stale-backup-no-exe.json') -Encoding utf8
if($p.ExitCode-eq0-or!$blocked){throw 'Stale backup was not independently protected'}
$exe=Join-Path $env:TEMP 'MDcard-0.1.5-wizard-中文 路径/MDcard.exe'
& $pwsh -NoProfile -File (Join-Path $PSScriptRoot 'verify-default-windows.ps1') -Exe $exe -Installer $setup -OutputDirectory "$env:TEMP\MDcard-0.1.5-final-default" *> (Join-Path $repo 'evidence/release-015/final-windows-default.log')
$code=$LASTEXITCODE;Copy-Item -LiteralPath "$env:TEMP\MDcard-0.1.5-final-default" -Destination (Join-Path $repo 'evidence/release-015/final-windows-default') -Recurse -Force;if($code){exit $code}
& $pwsh -NoProfile -File (Join-Path $PSScriptRoot 'verify-installed-ime-windows.ps1') -Exe $exe -OutputDirectory "$env:TEMP\MDcard-0.1.5-final-ime" *> (Join-Path $repo 'evidence/release-015/final-windows-ime.log')
$code=$LASTEXITCODE;Copy-Item -LiteralPath "$env:TEMP\MDcard-0.1.5-final-ime" -Destination (Join-Path $repo 'evidence/release-015/final-windows-ime') -Recurse -Force;exit $code
