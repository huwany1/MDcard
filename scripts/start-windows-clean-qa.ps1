# SPDX-License-Identifier: AGPL-3.0-only
# Isolated, networkless Windows evaluation guest. No physical disks or source mount.
param([string]$QaRoot="$PSScriptRoot\..\.local\windows-clean-qa")
$ErrorActionPreference='Stop';$qa=[IO.Path]::GetFullPath($QaRoot)
$qemu=Join-Path $qa 'qemu';$disk=Join-Path $qa 'guest.qcow2';$vars=Join-Path $qa 'guest-vars.fd'
if(Test-Path -LiteralPath $disk){throw 'Existing QA guest disk: inspect/resume it explicitly instead of replacing it'}
& (Join-Path $qemu 'qemu-img.exe') create -f qcow2 $disk 80G;if($LASTEXITCODE){throw 'Cannot create isolated virtual disk'}
Copy-Item -LiteralPath (Join-Path $qemu 'share\edk2-i386-vars.fd') -Destination $vars
$arguments=@('-machine','q35,accel=tcg','-cpu','max','-smp','2','-m','4096','-drive',('if=pflash,format=raw,readonly=on,file="'+(Join-Path $qemu 'share\edk2-x86_64-code.fd')+'"'),'-drive',('if=pflash,format=raw,file="'+$vars+'"'),'-drive',('file="'+$disk+'",format=qcow2,if=ide'),'-drive',('file="'+(Join-Path $qa 'Windows11-LTSC-2024-zh-CN-eval.iso')+'",media=cdrom,readonly=on'),'-drive',('file="'+(Join-Path $qa 'mdcard-clean-qa.iso')+'",media=cdrom,readonly=on'),'-boot','once=d,menu=off','-vga','none','-device','VGA,xres=1600,yres=1200','-display','none','-vnc','127.0.0.1:59','-monitor','tcp:127.0.0.1:4559,server=on,wait=off','-serial',('file:"'+(Join-Path $qa 'serial.log')+'"'),'-nic','none','-rtc','base=localtime','-device','qemu-xhci','-device','usb-tablet')
$process=Start-Process -FilePath (Join-Path $qemu 'qemu-system-x86_64.exe') -ArgumentList $arguments -WindowStyle Hidden -RedirectStandardOutput (Join-Path $qa 'qemu.stdout.log') -RedirectStandardError (Join-Path $qa 'qemu.stderr.log') -PassThru
@{pid=$process.Id;startedUTC=[DateTime]::UtcNow.ToString('o');arguments=$arguments;disk=$disk;qaOnly=$true;developmentToolsInstalled=$false;network='No virtual NIC';hardwareLimit='Software TCG, 2 vCPU, 4096 MiB; QA-only LabConfig bypass for TPM/secure boot/CPU. Not representative performance.'}|ConvertTo-Json -Depth 5|Set-Content (Join-Path $qa 'guest-process.json') -Encoding utf8
Write-Output ('Started isolated QA guest PID '+$process.Id)
