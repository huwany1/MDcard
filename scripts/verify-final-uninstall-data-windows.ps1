# SPDX-License-Identifier: AGPL-3.0-only
$ErrorActionPreference='Stop';$repo=Split-Path $PSScriptRoot
if(Get-Process MDcard -ErrorAction SilentlyContinue){throw 'Save and close the active MDcard before QA cleanup'}
$roots=@((Join-Path $env:APPDATA 'MDcard'),(Join-Path $env:LOCALAPPDATA 'MDcard'),(Join-Path $env:TEMP 'MDcard-0.1.5-final-default'),(Join-Path $env:TEMP 'MDcard-0.1.5-final-autosave'),(Join-Path $env:TEMP 'MDcard-0.1.5-final-recovery'))
$before=@{};foreach($root in $roots){if(Test-Path -LiteralPath $root){Get-ChildItem -LiteralPath $root -File -Recurse|ForEach-Object{$before[$_.FullName]=(Get-FileHash -LiteralPath $_.FullName).Hash}}}
$appBefore=(Get-FileHash -LiteralPath 'D:\MDcard\App\MDcard.exe').Hash
$shell=New-Object -ComObject WScript.Shell;$desktop=Join-Path $shell.SpecialFolders('Desktop') 'MDcard.lnk';$desktopBefore=(Get-FileHash -LiteralPath $desktop).Hash
$choice='HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.md\UserChoice';$choiceBefore=(Get-ItemProperty -LiteralPath $choice|Select-Object ProgId,Hash)|ConvertTo-Json -Compress
$records=@()
foreach($root in @((Join-Path $env:TEMP 'MDcard-0.1.5-wizard-中文 路径'),(Join-Path $env:LOCALAPPDATA 'Programs\MDcard 0.1.5 中文 验收'))){
 $exe=Join-Path $root 'unins000.exe';if(Test-Path -LiteralPath $exe){$p=Start-Process -FilePath $exe -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART') -WindowStyle Hidden -Wait -PassThru;$records+=@{root=$root;exitCode=$p.ExitCode;ownedExecutableRemoved=!(Test-Path (Join-Path $root 'MDcard.exe'))};if($p.ExitCode){throw 'Final QA uninstall failed'}}
}
$errors=@();foreach($file in $before.Keys){if(!(Test-Path -LiteralPath $file)-or(Get-FileHash -LiteralPath $file).Hash-ne$before[$file]){$errors+=$file}}
$choiceAfter=(Get-ItemProperty -LiteralPath $choice|Select-Object ProgId,Hash)|ConvertTo-Json -Compress
$result=@{finalInstallerSHA256='AB961FB64AC6CFDD2F4C782D16B81405FC70D69D661F090863F0E31BE502199D';uninstalls=$records;preservedFileCount=$before.Count;preservedFileMismatches=$errors;dailyPortableUnchanged=((Get-FileHash -LiteralPath 'D:\MDcard\App\MDcard.exe').Hash-eq$appBefore);desktopUnchanged=((Get-FileHash -LiteralPath $desktop).Hash-eq$desktopBefore);originalDesktopMatchesBackup=((Get-FileHash -LiteralPath $desktop).Hash-eq(Get-FileHash -LiteralPath (Join-Path $repo 'evidence/release-015/windows-installer-lifecycle/desktop-before.lnk')).Hash);userChoiceUnchanged=($choiceBefore-eq$choiceAfter);testDataRoots=$roots}
$result|ConvertTo-Json -Depth 5|Set-Content (Join-Path $repo 'evidence/release-015/final-windows-uninstall-data.json') -Encoding utf8
if($errors.Count-or!$result.dailyPortableUnchanged-or!$result.desktopUnchanged-or!$result.userChoiceUnchanged){throw 'Final uninstall changed user data or original entry'}
