# SPDX-License-Identifier: AGPL-3.0-only
# Built-in Windows PowerShell 5.1. Per-user immutable app backup, never user profiles.
param([ValidateSet('Prepare','Restore','Commit')][string]$Operation,[string]$InstallRoot,[string]$BackupRoot,[string]$NewManifest)
$ErrorActionPreference='Stop'
$app=[IO.Path]::GetFullPath($InstallRoot).TrimEnd('\');$backup=[IO.Path]::GetFullPath($BackupRoot).TrimEnd('\')
if($backup-ne($app+'.mdcard-rollback')){throw 'Unexpected rollback path'}
$statePath=Join-Path $backup 'state.json';$filesRoot=Join-Path $backup 'files'
function SafeChild([string]$Root,[string]$Relative){$p=[IO.Path]::GetFullPath((Join-Path $Root $Relative));if(!$p.StartsWith($Root.TrimEnd('\')+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Unsafe backup member'};return $p}
function SaveState($State){$State|ConvertTo-Json -Depth 12|Set-Content -LiteralPath $statePath -Encoding UTF8}
function RegTool([string[]]$Arguments){& "$env:SystemRoot\System32\reg.exe" @Arguments|Out-Null;if($LASTEXITCODE-ne0){throw 'Cannot preserve or restore product registration'}}
function RestoreCopy([string]$From,[string]$To){New-Item -ItemType Directory -Force (Split-Path $To)|Out-Null;if(Test-Path -LiteralPath $To){[IO.File]::SetAttributes($To,[IO.FileAttributes]::Normal)};Copy-Item -LiteralPath $From -Destination $To -Force}
if($Operation-eq'Prepare'){
 if(Test-Path -LiteralPath $backup){throw ('Previous interrupted installation backup exists. Follow RECOVERY.txt in '+$backup+' before retrying.')}
 if(!(Test-Path -LiteralPath (Join-Path $app 'MDcard.exe'))){throw 'No previous application to back up'}
 $entries=@(Get-ChildItem -LiteralPath $app -Recurse -Force)
 if(((Get-Item -LiteralPath $app).Attributes-band[IO.FileAttributes]::ReparsePoint)-or@($entries|Where-Object{$_.Attributes-band[IO.FileAttributes]::ReparsePoint}).Count){throw 'Application directory contains a reparse point; move it to an ordinary directory before upgrading'}
 New-Item -ItemType Directory -Path $filesRoot -Force|Out-Null
 $state=@{schema=1;product='MDcard';target=$app;complete=$false;files=@();registry=@();openWith=@();shortcuts=@();newFiles=@()}
 SaveState $state
 Copy-Item -LiteralPath $PSCommandPath -Destination (Join-Path $backup 'Restore-MDcard.ps1')
 @("MDcard interrupted-upgrade recovery. Save and close MDcard first.","Verified previous application files are under: $filesRoot",('Run: powershell.exe -NoProfile -ExecutionPolicy Bypass -File "'+$backup+'\Restore-MDcard.ps1" -Operation Restore -InstallRoot "'+$app+'" -BackupRoot "'+$backup+'"'),"Only backups with state.json complete=true may be restored. An incomplete backup means no application payload was overwritten; remove that incomplete backup after inspecting the unchanged old app.","User documents and profile settings/recovery outside the application directory are never deleted.")|Set-Content (Join-Path $backup 'RECOVERY.txt') -Encoding UTF8
 foreach($entry in $entries|Where-Object{!$_.PSIsContainer}){
  $relative=$entry.FullName.Substring($app.Length+1);$destination=SafeChild $filesRoot $relative
  New-Item -ItemType Directory -Force (Split-Path $destination)|Out-Null
  $hash=(Get-FileHash -LiteralPath $entry.FullName).Hash;Copy-Item -LiteralPath $entry.FullName -Destination $destination
  if((Get-FileHash -LiteralPath $destination).Hash-ne$hash){throw ('Backup verification failed: '+$relative)}
  $state.files+=@{path=$relative;sha256=$hash}
 }
 $keys=@('Software\Microsoft\Windows\CurrentVersion\Uninstall\{6F259284-88A3-44E2-A5A8-7F110F5F7045}_is1','Software\Classes\MDcard.Markdown')
 foreach($key in $keys){$exists=Test-Path -LiteralPath ('Registry::HKEY_CURRENT_USER\'+$key);$name='registry-'+$state.registry.Count+'.reg';if($exists){RegTool @('export',('HKCU\'+$key),(Join-Path $backup $name),'/y')};$state.registry+=@{key=$key;exists=$exists;file=$name}}
 foreach($extension in '.md','.markdown'){
  $key='Software\Classes\'+$extension+'\OpenWithProgids';$handle=[Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($key)
  $exists=$handle-and($handle.GetValueNames()-contains'MDcard.Markdown');$value=$null;$kind=$null
  if($exists){$value=$handle.GetValue('MDcard.Markdown');$kind=[string]$handle.GetValueKind('MDcard.Markdown')};if($handle){$handle.Close()}
  $state.openWith+=@{key=$key;exists=[bool]$exists;value=$value;kind=$kind}
 }
 $shell=New-Object -ComObject WScript.Shell
 foreach($shortcut in @((Join-Path $shell.SpecialFolders('Programs') 'MDcard.lnk'),(Join-Path $shell.SpecialFolders('Desktop') 'MDcard.lnk'))){$exists=Test-Path -LiteralPath $shortcut;$name='shortcut-'+$state.shortcuts.Count+'.lnk';if($exists){Copy-Item -LiteralPath $shortcut -Destination (Join-Path $backup $name)};$state.shortcuts+=@{path=$shortcut;exists=$exists;file=$name}}
 $manifest=Get-Content -Raw -LiteralPath $NewManifest|ConvertFrom-Json
 $state.newFiles=@($manifest.files|ForEach-Object{$_.path})+@('manifest.json')
 $state.complete=$true;SaveState $state
 Write-Output ('Verified complete upgrade backup: '+$backup);exit 0
}
if(!(Test-Path -LiteralPath $statePath)){throw 'Missing owned rollback state'}
$state=Get-Content -Raw -LiteralPath $statePath|ConvertFrom-Json
if($state.schema-ne1-or$state.product-ne'MDcard'-or$state.target-ne$app-or!$state.complete){throw 'Incomplete or mismatched rollback backup; no application files changed'}
if($Operation-eq'Restore'){
 if(Get-Process MDcard,mdcard -ErrorAction SilentlyContinue){throw 'Save and close MDcard before restoring the previous version'}
 foreach($entry in $state.files){$file=SafeChild $filesRoot $entry.path;if(!(Test-Path -LiteralPath $file)-or(Get-FileHash -LiteralPath $file).Hash-ne$entry.sha256){throw ('Corrupt previous-version backup; preserved for manual recovery: '+$file)}}
 $old=@{};foreach($entry in $state.files){$old[$entry.path.Replace('/','\')]=$true}
 foreach($relative in $state.newFiles){if(!$old.ContainsKey($relative.Replace('/','\'))){$file=SafeChild $app $relative;if(Test-Path -LiteralPath $file){Remove-Item -LiteralPath $file -Force}}}
 foreach($entry in $state.files){RestoreCopy (SafeChild $filesRoot $entry.path) (SafeChild $app $entry.path)}
 foreach($entry in $state.registry){$key='Registry::HKEY_CURRENT_USER\'+$entry.key;if(Test-Path -LiteralPath $key){Remove-Item -LiteralPath $key -Recurse -Force};if($entry.exists){RegTool @('import',(Join-Path $backup $entry.file))}}
 foreach($entry in $state.openWith){$key=[Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($entry.key);if($entry.exists){$kind=[Microsoft.Win32.RegistryValueKind]::$($entry.kind);$value=if($kind-eq[Microsoft.Win32.RegistryValueKind]::None-or$kind-eq[Microsoft.Win32.RegistryValueKind]::Binary){[byte[]]$entry.value}else{$entry.value};$key.SetValue('MDcard.Markdown',$value,$kind)}else{$key.DeleteValue('MDcard.Markdown',$false)};$key.Close()}
 foreach($entry in $state.shortcuts){if($entry.exists){RestoreCopy (Join-Path $backup $entry.file) $entry.path}elseif(Test-Path -LiteralPath $entry.path){Remove-Item -LiteralPath $entry.path -Force}}
 foreach($entry in $state.files){if((Get-FileHash -LiteralPath (SafeChild $app $entry.path)).Hash-ne$entry.sha256){throw 'Restored previous files did not match verified backup'}}
 Write-Output 'Previous application, registration and shortcuts restored; user profile untouched.'
}
# Only this positively identified, same-target backup is removed. Original user
# documents are untouched; this deletes redundant copied backup files only.
Remove-Item -LiteralPath $backup -Recurse -Force
