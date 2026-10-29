# SPDX-License-Identifier: AGPL-3.0-only
param([string]$Installer,[string]$InstallRoot,[string]$OutputDirectory)
$ErrorActionPreference='Stop';New-Item -ItemType Directory -Force $OutputDirectory|Out-Null
Add-Type @'
using System;using System.Text;using System.Collections.Generic;using System.Runtime.InteropServices;
public class OwnedInstallerUi {
public delegate bool Callback(IntPtr h,IntPtr p);
[DllImport("user32.dll")]public static extern bool EnumWindows(Callback c,IntPtr p);
[DllImport("user32.dll")]public static extern bool EnumChildWindows(IntPtr h,Callback c,IntPtr p);
[DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetWindowText(IntPtr h,StringBuilder b,int n);
[DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,StringBuilder b,int n);
[DllImport("user32.dll")]public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
[DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
public static string Row(IntPtr h){var t=new StringBuilder(512);var c=new StringBuilder(128);GetWindowText(h,t,512);GetClassName(h,c,128);return h.ToInt64()+"|"+c+"|"+t;}
public static string[] Windows(uint pid,bool children){var r=new List<string>();EnumWindows((h,p)=>{uint owner;GetWindowThreadProcessId(h,out owner);if(owner==pid){r.Add(Row(h));if(children)EnumChildWindows(h,(w,q)=>{r.Add(Row(w));return true;},IntPtr.Zero);}return true;},IntPtr.Zero);return r.ToArray();}
}
'@
$exe=Join-Path $InstallRoot 'MDcard.exe';$before=if(Test-Path -LiteralPath $exe){(Get-FileHash -LiteralPath $exe).Hash}else{$null}
$setup=Start-Process -FilePath $Installer -ArgumentList @('/LANG=english',('/DIR="'+$InstallRoot+'"'),('/LOG="'+(Join-Path $OutputDirectory 'cancel.log')+'"')) -WindowStyle Hidden -PassThru
$deadline=[DateTime]::UtcNow.AddSeconds(20);$cancel=$null;$child=$null
do {
 Start-Sleep -Milliseconds 100
 $entry=Get-CimInstance Win32_Process -Filter ('ParentProcessId = '+$setup.Id)|Where-Object Name -like '*.tmp'|Select-Object -First 1
 if($entry){$child=Get-Process -Id $entry.ProcessId;$retainedHandle=$child.Handle;$rows=[OwnedInstallerUi]::Windows([uint32]$child.Id,$true);$cancel=$rows|Where-Object {$_-match'\|TNewButton\|Cancel$'}|Select-Object -First 1}
}while(!$cancel-and[DateTime]::UtcNow-lt$deadline)
if(!$cancel){throw 'No owned wizard Cancel button before timeout'}
$rows|Set-Content (Join-Path $OutputDirectory 'wizard-controls.txt') -Encoding utf8
[void][OwnedInstallerUi]::PostMessage([IntPtr]::new([long]($cancel.Split('|')[0])),0xF5,[IntPtr]::Zero,[IntPtr]::Zero)
$dialog=$null;$deadline=[DateTime]::UtcNow.AddSeconds(5)
do {Start-Sleep -Milliseconds 100;$dialog=[OwnedInstallerUi]::Windows([uint32]$child.Id,$false)|Where-Object {$_-match'\|#32770\|Exit Setup$'}|Select-Object -First 1}while(!$dialog-and[DateTime]::UtcNow-lt$deadline)
if(!$dialog){throw 'No owned Exit Setup confirmation'}
[void][OwnedInstallerUi]::PostMessage([IntPtr]::new([long]($dialog.Split('|')[0])),0x111,[IntPtr]::new(6),[IntPtr]::Zero)
if(!$setup.WaitForExit(10000)){throw 'Cancelled setup did not exit'}
$after=if(Test-Path -LiteralPath $exe){(Get-FileHash -LiteralPath $exe).Hash}else{$null}
$result=@{cancelViaActualWizardButton=$true;exitConfirmedViaOwnedDialog=$true;exitCode=$setup.ExitCode;previousExeSHA256=$before;afterExeSHA256=$after;previousVersionPreserved=($before-eq$after);installedExeVersion=$(if($after){(Get-Item $exe).VersionInfo.ProductVersion}else{$null})}
$result|ConvertTo-Json|Set-Content (Join-Path $OutputDirectory 'result.json') -Encoding utf8
if($setup.ExitCode-ne2-or$before-ne$after){throw 'Installer cancellation did not preserve previous installation'}
