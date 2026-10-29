# SPDX-License-Identifier: AGPL-3.0-only
"""Prepare read-only external QA media; never part of the application package."""
import json,re,shutil
from pathlib import Path
root=Path(__file__).resolve().parent.parent
media=root/'.local/windows-clean-qa/media';media.mkdir(parents=True,exist_ok=True)
final=root/'artifacts/release-015/final-packages/MDcard-0.1.5-windows-x64-setup.exe'
shutil.copy2(final,media/final.name)
names=['verify-ui-gpu-windows.ps1','recovery-json.ps1','verify-default-windows.ps1','verify-installed-ime-windows.ps1']
for name in names:
    text=(root/'scripts'/name).read_text(encoding='utf-8-sig')
    text=text.replace('$launch.Environment','$launch.EnvironmentVariables')
    text=text.replace("$root=Split-Path $PSScriptRoot","$root=Join-Path $env:USERPROFILE 'QA-helper'")
    text=re.sub(r'\$launch\.ArgumentList\.Add\(([^;\n]+)\)',r'$launch.Arguments += (\'"\'+[string](\1)+\'" \')',text)
    text=text.replace('AddSeconds(30)','AddSeconds(300)').replace('-lt30)','-lt300)')
    text=text.replace('Start-Sleep -Seconds 3','Start-Sleep -Seconds 20').replace('Start-Sleep -Milliseconds 500','Start-Sleep -Milliseconds 1500')
    (media/name).write_text(text,encoding='utf-8-sig')
text=(root/'scripts/verify-installed-engines.mjs').read_text(encoding='utf-8').replace('180000','1800000')
(media/'verify-installed-engines.mjs').write_text(text,encoding='utf-8')
(media/'guest-run.ps1').write_text((root/'scripts/windows-clean-guest.ps1').read_text(encoding='utf-8'),encoding='utf-8-sig')
answer='''<?xml version="1.0" encoding="utf-8"?>
<unattend xmlns="urn:schemas-microsoft-com:unattend" xmlns:wcm="http://schemas.microsoft.com/WMIConfig/2002/State">
 <settings pass="windowsPE">
  <component name="Microsoft-Windows-International-Core-WinPE" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS"><SetupUILanguage><UILanguage>zh-CN</UILanguage></SetupUILanguage><InputLocale>0804:00000804</InputLocale><SystemLocale>zh-CN</SystemLocale><UILanguage>zh-CN</UILanguage><UserLocale>zh-CN</UserLocale></component>
  <component name="Microsoft-Windows-Setup" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
   <RunSynchronous>
    <RunSynchronousCommand wcm:action="add"><Order>1</Order><Path>reg add HKLM\\SYSTEM\\Setup\\LabConfig /v BypassTPMCheck /t REG_DWORD /d 1 /f</Path></RunSynchronousCommand>
    <RunSynchronousCommand wcm:action="add"><Order>2</Order><Path>reg add HKLM\\SYSTEM\\Setup\\LabConfig /v BypassSecureBootCheck /t REG_DWORD /d 1 /f</Path></RunSynchronousCommand>
    <RunSynchronousCommand wcm:action="add"><Order>3</Order><Path>reg add HKLM\\SYSTEM\\Setup\\LabConfig /v BypassCPUCheck /t REG_DWORD /d 1 /f</Path></RunSynchronousCommand>
   </RunSynchronous>
   <DiskConfiguration><Disk wcm:action="add"><DiskID>0</DiskID><WillWipeDisk>true</WillWipeDisk><CreatePartitions><CreatePartition wcm:action="add"><Order>1</Order><Type>EFI</Type><Size>260</Size></CreatePartition><CreatePartition wcm:action="add"><Order>2</Order><Type>MSR</Type><Size>16</Size></CreatePartition><CreatePartition wcm:action="add"><Order>3</Order><Type>Primary</Type><Extend>true</Extend></CreatePartition></CreatePartitions><ModifyPartitions><ModifyPartition wcm:action="add"><Order>1</Order><PartitionID>1</PartitionID><Format>FAT32</Format><Label>System</Label></ModifyPartition><ModifyPartition wcm:action="add"><Order>2</Order><PartitionID>3</PartitionID><Format>NTFS</Format><Label>WindowsQA</Label><Letter>C</Letter></ModifyPartition></ModifyPartitions></Disk><WillShowUI>OnError</WillShowUI></DiskConfiguration>
   <ImageInstall><OSImage><InstallFrom><MetaData wcm:action="add"><Key>/IMAGE/INDEX</Key><Value>1</Value></MetaData></InstallFrom><InstallTo><DiskID>0</DiskID><PartitionID>3</PartitionID></InstallTo><WillShowUI>OnError</WillShowUI></OSImage></ImageInstall><UserData><AcceptEula>true</AcceptEula><FullName>MDcard isolated QA</FullName><Organization>Local release validation</Organization></UserData>
  </component>
 </settings>
 <settings pass="specialize"><component name="Microsoft-Windows-Shell-Setup" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS"><ComputerName>MDCARD-QA</ComputerName><TimeZone>Tokyo Standard Time</TimeZone></component></settings>
 <settings pass="oobeSystem">
  <component name="Microsoft-Windows-International-Core" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS"><InputLocale>0804:00000804</InputLocale><SystemLocale>zh-CN</SystemLocale><UILanguage>zh-CN</UILanguage><UserLocale>zh-CN</UserLocale></component>
  <component name="Microsoft-Windows-Shell-Setup" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS"><OOBE><HideEULAPage>true</HideEULAPage><HideOnlineAccountScreens>true</HideOnlineAccountScreens><HideWirelessSetupInOOBE>true</HideWirelessSetupInOOBE><ProtectYourPC>3</ProtectYourPC></OOBE><UserAccounts><LocalAccounts><LocalAccount wcm:action="add"><Name>MDcardQA</Name><DisplayName>Disposable MDcard QA</DisplayName><Group>Administrators</Group><Password><Value>Local-Disposable-QA-015!</Value><PlainText>true</PlainText></Password></LocalAccount></LocalAccounts></UserAccounts><AutoLogon><Enabled>true</Enabled><LogonCount>2</LogonCount><Username>MDcardQA</Username><Password><Value>Local-Disposable-QA-015!</Value><PlainText>true</PlainText></Password></AutoLogon><FirstLogonCommands><SynchronousCommand wcm:action="add"><Order>1</Order><Description>Offline local MDcard QA</Description><CommandLine>cmd /c for %d in (D E F G H I J) do @if exist %d:\\guest-run.ps1 powershell.exe -NoProfile -STA -ExecutionPolicy Bypass -File %d:\\guest-run.ps1</CommandLine><RequiresUserInput>false</RequiresUserInput></SynchronousCommand></FirstLogonCommands></component>
 </settings>
</unattend>'''
(media/'autounattend.xml').write_text(answer,encoding='utf-8')
print(json.dumps({'media':str(media),'installer':str(final),'purpose':'QA only; offline virtual guest; no project source or development tools'}))
