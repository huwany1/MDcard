# SPDX-License-Identifier: AGPL-3.0-only
# Read-only QA CD entry point; Windows stock PowerShell 5.1, no developer tools.
param([switch]$SkipInstall,[string]$EvidenceTag='MDcard-clean-QA-r2')
$ErrorActionPreference='Stop'
$out=Join-Path $env:USERPROFILE $EvidenceTag;New-Item -ItemType Directory -Force $out|Out-Null
$serial=$null
try{$serial=New-Object IO.Ports.SerialPort COM1,115200,None,8,one;$serial.Open()}catch{}
function Progress([string]$Message){$line=([DateTime]::UtcNow.ToString('o')+' '+$Message);Add-Content (Join-Path $out 'progress.log') $line;if($serial-and$serial.IsOpen){$serial.WriteLine($line)}}
Start-Transcript -Path (Join-Path $out 'transcript.log') -Force
$result=@{passed=$false;network='QEMU -nic none; no virtual network adapter';media=$PSScriptRoot;sourceDirectoryPresent=$false}
try {
 Progress 'Guest reached native desktop; starting installed-package acceptance'
 $os=Get-CimInstance Win32_OperatingSystem;$result.os=@{caption=$os.Caption;version=$os.Version;architecture=$os.OSArchitecture}
 $tools=@{};foreach($name in @('node','npm','java','pandoc','rustc','cargo','gcc','git')){$tools[$name]=[bool](Get-Command $name -ErrorAction SilentlyContinue)};$result.systemDeveloperTools=$tools
 if(@($tools.Values|Where-Object{$_}).Count-gt0){throw 'Unexpected system developer tool in clean guest'}
 $setup=Join-Path $PSScriptRoot 'MDcard-0.1.5-windows-x64-setup.exe';$result.installerSHA256=(Get-FileHash $setup).Hash
 $appRoot=Join-Path $env:LOCALAPPDATA 'Programs\MDcard 中文 路径';$log=Join-Path $out 'install.log'
 if(!$SkipInstall){$proc=Start-Process $setup -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART',('/DIR="'+$appRoot+'"'),('/LOG="'+$log+'"')) -WindowStyle Hidden -Wait -PassThru;$result.installExitCode=$proc.ExitCode;if($proc.ExitCode-ne0){throw 'Clean guest install failed'}}else{$result.installEvidence='Reuse actual final package installed in first clean-OS attempt; installer/native SHA revalidated. No product rebuild or developer tools added.'}
 $exe=Join-Path $appRoot 'MDcard.exe';$result.binarySHA256=(Get-FileHash $exe).Hash;Progress 'Installed final EXE package, starting native default flow'
 & "$PSHOME\powershell.exe" -NoProfile -STA -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'verify-default-windows.ps1') -Exe $exe -Installer $setup -OutputDirectory (Join-Path $out 'native-default')
 if($LASTEXITCODE){throw 'Native default guest acceptance failed'}
 Progress 'Native New/Open/Save As and dirty-app install protection passed'
 & "$PSHOME\powershell.exe" -NoProfile -STA -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'verify-installed-ime-windows.ps1') -Exe $exe -OutputDirectory (Join-Path $out 'real-ime')
 if($LASTEXITCODE){throw 'Native Microsoft Pinyin guest acceptance failed'}
 Progress 'Microsoft Pinyin preedit and commit passed; starting offline bundled engines and nine exports'
 $env:MDCARD_QA_NETWORK='QEMU -nic none';$env:PATH=Join-Path $env:SystemRoot 'System32'
 & (Join-Path $appRoot 'renderer\runtime\node.exe') (Join-Path $PSScriptRoot 'verify-installed-engines.mjs') $appRoot (Join-Path $out 'engines')
 if($LASTEXITCODE-ne0){throw 'Installed bundled engines or nine exports failed'}
 $result.passed=$true;Progress 'All clean guest acceptance passed'
}catch{$result.error=$_.Exception.ToString();Progress ('FAILED: '+$result.error)}
$result|ConvertTo-Json -Depth 8|Set-Content (Join-Path $out 'result.json') -Encoding UTF8
Stop-Transcript
try{
 $archive=Join-Path $env:TEMP 'mdcard-clean-QA.zip';Compress-Archive -Path (Join-Path $out '*') -DestinationPath $archive -Force
 if($serial-and$serial.IsOpen){$serial.WriteLine('MDCARD_QA_ZIP_BEGIN');$encoded=[Convert]::ToBase64String([IO.File]::ReadAllBytes($archive));for($i=0;$i-lt$encoded.Length;$i+=12000){$serial.WriteLine($encoded.Substring($i,[Math]::Min(12000,$encoded.Length-$i)))};$serial.WriteLine('MDCARD_QA_ZIP_END');$serial.Close()}
}catch{}
& shutdown.exe /s /t 10
