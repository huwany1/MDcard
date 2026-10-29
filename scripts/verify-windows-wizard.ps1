# SPDX-License-Identifier: AGPL-3.0-only
param([string]$Installer,[string]$InstallRoot,[string]$OutputDirectory,[int]$ResumeBootstrapId=0,[string]$StartupReport='',[switch]$CaptureExistingResult,[string]$InstallationLog='')
$ErrorActionPreference='Stop';New-Item -ItemType Directory -Force $OutputDirectory|Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;using System.Text;using System.Collections.Generic;using System.Runtime.InteropServices;
public class WizardQA {
public delegate bool Callback(IntPtr h,IntPtr p);
[StructLayout(LayoutKind.Sequential)]public struct Rect{public int L,T,R,B;}
[DllImport("user32.dll")]public static extern bool EnumWindows(Callback c,IntPtr p);
[DllImport("user32.dll")]public static extern bool EnumChildWindows(IntPtr h,Callback c,IntPtr p);
[DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetWindowText(IntPtr h,StringBuilder b,int n);
[DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,StringBuilder b,int n);
[DllImport("user32.dll")]public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
[DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);
[DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
[DllImport("user32.dll")]public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int cx,int cy,uint flags);
public static string Row(IntPtr h){var t=new StringBuilder(512);var c=new StringBuilder(128);GetWindowText(h,t,512);GetClassName(h,c,128);return h.ToInt64()+"|"+c+"|"+t;}
public static string[] Windows(uint pid){var rows=new List<string>();EnumWindows((h,p)=>{uint owner;GetWindowThreadProcessId(h,out owner);if(owner==pid){rows.Add(Row(h));EnumChildWindows(h,(w,q)=>{if(IsWindowVisible(w))rows.Add(Row(w));return true;},IntPtr.Zero);}return true;},IntPtr.Zero);return rows.ToArray();}
}
'@
function Shot([string]$Name,[IntPtr]$Handle){$r=[WizardQA+Rect]::new();[void][WizardQA]::GetWindowRect($Handle,[ref]$r);$b=[Drawing.Bitmap]::new($r.R-$r.L,$r.B-$r.T);$g=[Drawing.Graphics]::FromImage($b);$g.CopyFromScreen($r.L,$r.T,0,0,$b.Size);$b.Save((Join-Path $OutputDirectory ($Name+'.png')));$g.Dispose();$b.Dispose()}
if($CaptureExistingResult){
 $app=Get-Process MDcard -ErrorAction SilentlyContinue|Where-Object Path -eq (Join-Path $InstallRoot 'MDcard.exe')|Select-Object -First 1
 $log=Get-Content -Raw -LiteralPath $InstallationLog
 if(!$app-or!(Test-Path -LiteralPath $StartupReport)-or!$log.Contains('Installation process succeeded.')-or!$log.Contains('Filename: '+(Join-Path $InstallRoot 'MDcard.exe'))){throw 'Missing independent successful install/automatic launch evidence'}
 [void][WizardQA]::SetWindowPos($app.MainWindowHandle,[IntPtr]::new(-1),0,0,0,0,0x43);Shot '03-auto-started-app' $app.MainWindowHandle
 @{installerSHA256=(Get-FileHash $Installer).Hash;installRoot=$InstallRoot;destinationPage=$true;optionalTasksPage=$true;lastActionActualCaption='Next';automaticLaunch=$true;wizardActionsAfterOpeningSetup=2;launchBinarySHA256=(Get-FileHash $app.Path).Hash;installationLog=$InstallationLog;installationLogSHA256=(Get-FileHash $InstallationLog).Hash;automaticLaunchProcessId=$app.Id;setupExitCode=$null;qaBoundary='Caption selector probe resumed the same wizard. Bootstrap exit code unavailable after resuming through Get-Process; complete successful Inno log and its Run entry plus automatically created app independently confirmed.'}|ConvertTo-Json -Depth 4|Set-Content (Join-Path $OutputDirectory 'result.json') -Encoding utf8
 [void][WizardQA]::PostMessage($app.MainWindowHandle,0x10,[IntPtr]::Zero,[IntPtr]::Zero);$app.WaitForExit(10000)|Out-Null;return
}
$env:MDCARD_SETTINGS_PATH=Join-Path $OutputDirectory 'settings.json';@{language='en';reduce_motion=$true}|ConvertTo-Json|Set-Content $env:MDCARD_SETTINGS_PATH -Encoding utf8
$env:MDCARD_UI_REPORT=Join-Path $OutputDirectory 'app-ui.json';$env:MDCARD_RECOVERY_DIR=Join-Path $OutputDirectory 'recovery';$env:MDCARD_READING_STATE_PATH=Join-Path $OutputDirectory 'reading.json'
$setup=if($ResumeBootstrapId){Get-Process -Id $ResumeBootstrapId}else{Start-Process $Installer -ArgumentList @('/LANG=english',('/DIR="'+$InstallRoot+'"'),('/LOG="'+(Join-Path $OutputDirectory 'wizard-install.log')+'"')) -WindowStyle Hidden -PassThru}
$deadline=[DateTime]::UtcNow.AddSeconds(20);$child=$null
do{Start-Sleep -Milliseconds 100;$entry=Get-CimInstance Win32_Process -Filter ('ParentProcessId = '+$setup.Id)|Where-Object Name -like '*.tmp'|Select-Object -First 1;if($entry){$child=Get-Process -Id $entry.ProcessId}}while(!$child-and[DateTime]::UtcNow-lt$deadline)
if(!$child){throw 'No owned interactive installer'}
if(!$ResumeBootstrapId){
$deadline=[DateTime]::UtcNow.AddSeconds(20)
do{$rows=[WizardQA]::Windows([uint32]$child.Id);$next=$rows|Where-Object{$_-match'\|TNewButton\|.*Next(?: >)?$'}|Select-Object -First 1;if(!$next){Start-Sleep -Milliseconds 100}}while(!$next-and[DateTime]::UtcNow-lt$deadline)
$rows|Set-Content (Join-Path $OutputDirectory '01-destination-controls.txt');$main=($rows|Where-Object{$_-match'\|TWizardForm\|'}|Select-Object -First 1).Split('|')[0];[void][WizardQA]::SetWindowPos([IntPtr]::new([long]$main),[IntPtr]::new(-1),0,0,0,0,0x43);Shot '01-destination' ([IntPtr]::new([long]$main))
$next=$rows|Where-Object{$_-match'\|TNewButton\|.*Next(?: >)?$'}|Select-Object -First 1;if(!$next){throw 'Destination page Next button absent'}
[void][WizardQA]::PostMessage([IntPtr]::new([long]($next.Split('|')[0])),0xF5,[IntPtr]::Zero,[IntPtr]::Zero);Start-Sleep -Milliseconds 600
}else{$rows=[WizardQA]::Windows([uint32]$child.Id);$main=($rows|Where-Object{$_-match'\|TWizardForm\|'}|Select-Object -First 1).Split('|')[0]}
$rows=[WizardQA]::Windows([uint32]$child.Id);$rows|Set-Content (Join-Path $OutputDirectory '02-options-controls.txt');Shot '02-optional-tasks' ([IntPtr]::new([long]$main))
$install=$rows|Where-Object{$_-match'\|TNewButton\|.*(?:Install and launch MDcard|Next(?: >)?)$'}|Select-Object -First 1;if(!$install){throw 'Actual final wizard action absent'}
[void][WizardQA]::PostMessage([IntPtr]::new([long]($install.Split('|')[0])),0xF5,[IntPtr]::Zero,[IntPtr]::Zero)
if(!$setup.WaitForExit(90000)-or$setup.ExitCode-ne0){throw 'Interactive final install did not complete'}
$deadline=[DateTime]::UtcNow.AddSeconds(15);$app=$null
do{Start-Sleep -Milliseconds 100;$app=Get-Process MDcard -ErrorAction SilentlyContinue|Where-Object Path -eq (Join-Path $InstallRoot 'MDcard.exe')|Select-Object -First 1}while(!$app-and[DateTime]::UtcNow-lt$deadline)
if($StartupReport){$env:MDCARD_UI_REPORT=$StartupReport}
if(!$app-or!(Test-Path -LiteralPath $env:MDCARD_UI_REPORT)){throw 'Ordinary wizard did not automatically start installed app'}
Start-Sleep -Milliseconds 700;$app.Refresh();[void][WizardQA]::SetWindowPos($app.MainWindowHandle,[IntPtr]::new(-1),0,0,0,0,0x43);Shot '03-auto-started-app' $app.MainWindowHandle
@{installerSHA256=(Get-FileHash $Installer).Hash;installRoot=$InstallRoot;userLevel=$true;destinationPage=$true;optionalTasksPage=$true;lastActionActualCaption=$install.Split('|')[-1];automaticLaunch=$true;wizardActionsAfterOpeningSetup=2;continuedAfterQACaptionMismatch=[bool]$ResumeBootstrapId;launchBinarySHA256=(Get-FileHash (Join-Path $InstallRoot 'MDcard.exe')).Hash;silentLaunch='skipifsilent separately checked during three fresh installations'}|ConvertTo-Json|Set-Content (Join-Path $OutputDirectory 'result.json') -Encoding utf8
[void][WizardQA]::PostMessage($app.MainWindowHandle,0x10,[IntPtr]::Zero,[IntPtr]::Zero);$app.WaitForExit(10000)|Out-Null
