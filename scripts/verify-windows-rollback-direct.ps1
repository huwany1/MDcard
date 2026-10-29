# SPDX-License-Identifier: AGPL-3.0-only
param([string]$InstallRoot,[string]$NewManifest,[string]$OutputDirectory)
$ErrorActionPreference='Stop';New-Item -ItemType Directory -Force $OutputDirectory|Out-Null
$backup=$InstallRoot+'.mdcard-rollback';$scriptPath=Join-Path $PSScriptRoot '../packaging/windows/rollback.ps1';$before=@{}
Get-ChildItem -LiteralPath $InstallRoot -File -Recurse|ForEach-Object{$before[$_.FullName]=(Get-FileHash -LiteralPath $_.FullName).Hash}
$key='HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{6F259284-88A3-44E2-A5A8-7F110F5F7045}_is1';$registration=Get-ItemProperty -LiteralPath $key
$shell=New-Object -ComObject WScript.Shell;$desktop=Join-Path $shell.SpecialFolders('Desktop') 'MDcard.lnk';$desktopBefore=if(Test-Path $desktop){(Get-FileHash -LiteralPath $desktop).Hash}else{$null}
$clock=[Diagnostics.Stopwatch]::StartNew()
try{
 & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $scriptPath -Operation Prepare -InstallRoot $InstallRoot -BackupRoot $backup -NewManifest $NewManifest
 if($LASTEXITCODE){throw 'Complete previous payload backup failed'};$prepare=$clock.Elapsed.TotalSeconds
 $space=(@(Get-ChildItem -LiteralPath $backup -File -Recurse)|Measure-Object Length -Sum).Sum
 [IO.File]::WriteAllText((Join-Path $InstallRoot 'README.txt'),'Different incomplete upgrade bytes')
 [IO.File]::WriteAllText((Join-Path $InstallRoot 'unknown-user-after.md'),'KEEP newly created user document 中文')
 Set-ItemProperty -LiteralPath $key -Name DisplayVersion -Value '0.1.5-QA-incomplete'
 if($desktopBefore){$link=$shell.CreateShortcut($desktop);$link.TargetPath=Join-Path $InstallRoot 'QA-incomplete.exe';$link.Save()}
 $restoreClock=[Diagnostics.Stopwatch]::StartNew();& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $scriptPath -Operation Restore -InstallRoot $InstallRoot -BackupRoot $backup
 if($LASTEXITCODE){throw 'Complete old payload restoration failed'};$restoreSeconds=$restoreClock.Elapsed.TotalSeconds
 $errors=@();foreach($file in $before.Keys){if(!(Test-Path -LiteralPath $file)-or(Get-FileHash -LiteralPath $file).Hash-ne$before[$file]){$errors+=$file}}
 $after=Get-ItemProperty -LiteralPath $key;$desktopAfter=if(Test-Path $desktop){(Get-FileHash -LiteralPath $desktop).Hash}else{$null}
 $result=@{oldPayloadFiles=$before.Count;oldPayloadMismatches=$errors;registrationRestored=($after.DisplayVersion-eq$registration.DisplayVersion-and$after.InstallLocation-eq$registration.InstallLocation);desktopRestored=($desktopBefore-eq$desktopAfter);unknownUserDocumentRetained=(Test-Path (Join-Path $InstallRoot 'unknown-user-after.md'));backupCleaned=!(Test-Path $backup);prepareSeconds=$prepare;restoreSeconds=$restoreSeconds;temporaryBackupLogicalBytes=$space}
 $result|ConvertTo-Json -Depth 4|Set-Content (Join-Path $OutputDirectory 'result.json') -Encoding utf8
 if($errors.Count-or!$result.registrationRestored-or!$result.desktopRestored-or!$result.unknownUserDocumentRetained){throw 'Direct full payload rollback check failed'}
}finally{if(Test-Path -LiteralPath (Join-Path $backup 'complete')){Write-Output 'Complete backup retained; see recovery instructions'}elseif(Test-Path -LiteralPath (Join-Path $backup 'state.json')){$s=Get-Content -Raw (Join-Path $backup 'state.json')|ConvertFrom-Json;if($s.complete){& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $scriptPath -Operation Restore -InstallRoot $InstallRoot -BackupRoot $backup}}}
