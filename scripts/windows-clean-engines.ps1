# SPDX-License-Identifier: AGPL-3.0-only
# Independent runtime acceptance; no GUI selector can block these checks.
$ErrorActionPreference='Stop';$out=Join-Path $env:USERPROFILE 'MDcard-clean-QA-r2';New-Item -ItemType Directory -Force $out|Out-Null
$serial=$null;try{$serial=New-Object IO.Ports.SerialPort COM1,115200,None,8,one;$serial.Open()}catch{}
function Progress([string]$Message){$line=[DateTime]::UtcNow.ToString('o')+' '+$Message;$line|Add-Content (Join-Path $out 'engine-progress.log');Write-Output $line;if($serial-and$serial.IsOpen){$serial.WriteLine($line)}}
$result=@{runtimePassed=$false;network='QEMU has no virtual NIC';noProjectDirectory=$true;psVersion=$PSVersionTable.PSVersion.ToString();osVersion=[Environment]::OSVersion.Version.ToString();user=[Security.Principal.WindowsIdentity]::GetCurrent().Name;isAdministrator=([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)}
try{
 foreach($name in 'result.json','progress.log','transcript.log'){if(Test-Path (Join-Path $env:USERPROFILE ('MDcard-clean-QA\'+$name))){Copy-Item (Join-Path $env:USERPROFILE ('MDcard-clean-QA\'+$name)) (Join-Path $out ('prior-'+$name))}}
 $tools=@{};foreach($name in 'node','npm','java','pandoc','rustc','cargo','gcc','git'){$tools[$name]=[bool](Get-Command $name -ErrorAction SilentlyContinue)};$result.systemDeveloperTools=$tools;if(@($tools.Values|Where-Object{$_}).Count){throw 'Unexpected system developer runtime'}
 $stage=(Get-ChildItem -LiteralPath (Join-Path $env:LOCALAPPDATA 'Programs') -Directory|Where-Object Name -like 'MDcard*'|Select-Object -First 1).FullName
 $result.installedRoot=$stage;$result.installerSHA256=(Get-FileHash (Join-Path $PSScriptRoot 'MDcard-0.1.5-windows-x64-setup.exe')).Hash;$result.nativeSHA256=(Get-FileHash (Join-Path $stage 'MDcard.exe')).Hash
 if($result.nativeSHA256-ne'933F1591A0FD47806E9E2EB2EB29BA2489493236E836CA8A66F83A7240B4C127'-or$result.installerSHA256-ne'AB961FB64AC6CFDD2F4C782D16B81405FC70D69D661F090863F0E31BE502199D'){throw 'Final installed/package identity mismatch'}
 Progress 'Starting independent actual-installed four engines and nine exports'
 $env:MDCARD_QA_NETWORK='QEMU -nic none; no virtual NIC';Set-Location $out
 & (Join-Path $stage 'renderer\runtime\node.exe') (Join-Path $PSScriptRoot 'verify-installed-engines.mjs') $stage (Join-Path $out 'engines')
 if($LASTEXITCODE){throw 'Installed engine/export verification failed'}
 $result.engines=Get-Content -Raw -Encoding UTF8 (Join-Path $out 'engines\result.json')|ConvertFrom-Json;$result.runtimePassed=$true;Progress 'Four installed engines and all nine exports passed offline'
}catch{$result.error=$_.Exception.ToString();Progress ('FAILED '+$result.error)}
$json=$result|ConvertTo-Json -Depth 12;$json|Set-Content (Join-Path $out 'engines-only-result.json') -Encoding UTF8
if($serial-and$serial.IsOpen){$serial.WriteLine('MDCARD_ENGINE_RESULT '+[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($json)));$serial.Close()}
Write-Output $json
