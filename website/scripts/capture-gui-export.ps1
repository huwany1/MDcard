# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
$ErrorActionPreference='Stop'
$site=Split-Path $PSScriptRoot;$repo=Split-Path $site;$Exe=Join-Path $repo 'dist/windows/MDcard.exe'
. (Join-Path $repo 'scripts/verify-ui-gpu-windows.ps1') -Exe $Exe -FunctionsOnly
$output=Join-Path $site 'evidence/product/gui-export';New-Item -ItemType Directory -Force $output|Out-Null
$settings=Join-Path $output 'settings.json';$report=Join-Path $output 'export-report.json'
$source=Join-Path $site 'samples/从原稿到作品.md';$destination=Join-Path $output '从原稿到作品-GUI.html'
@{language='zh-CN';appearance='Light';reduce_motion=$true;font_size=17;gpu_acceleration=$true}|ConvertTo-Json|Set-Content $settings -Encoding utf8
'{"format":0}'|Set-Content (Join-Path $output 'export-settings.json') -Encoding utf8
$oldReport=$env:MDCARD_EXPORT_REPORT;$env:MDCARD_EXPORT_REPORT=$report
Add-Type @'
using System; using System.Runtime.InteropServices;using System.Text;
public class ReleaseDialog {
 delegate bool EnumProc(IntPtr h,IntPtr extra);
 [DllImport("user32.dll")]static extern bool EnumWindows(EnumProc p,IntPtr e);
 [DllImport("user32.dll")]static extern uint GetWindowThreadProcessId(IntPtr h,out uint pid);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]static extern int GetWindowText(IntPtr h,StringBuilder b,int c);
 [DllImport("user32.dll")]public static extern void keybd_event(byte k,byte s,uint f,UIntPtr e);
 public static IntPtr Find(uint pid){IntPtr found=IntPtr.Zero;EnumWindows((h,e)=>{uint p;GetWindowThreadProcessId(h,out p);var b=new StringBuilder(512);GetWindowText(h,b,512);if(p==pid&&(b.ToString()=="另存为"||b.ToString()=="Save As")){found=h;return false;}return true;},IntPtr.Zero);return found;}
}
'@
function Chord([byte[]]$codes){foreach($c in $codes){[ReleaseDialog]::keybd_event($c,0,0,[UIntPtr]::Zero)};for($i=$codes.Count-1;$i-ge0;$i--){[ReleaseDialog]::keybd_event($codes[$i],0,2,[UIntPtr]::Zero)};Start-Sleep -Milliseconds 250}
$before=(Get-FileHash $source).Hash.ToLower();$clipboard=[Windows.Forms.Clipboard]::GetDataObject()
try{
 Launch 'renderer' '' $source
 Keys '^%e';Start-Sleep -Milliseconds 600
 $s=Get-Content $report -Raw|ConvertFrom-Json;$r=$s.controls.export;if(!$r){throw 'No actual export control'}
 Shot 'before-export';Click ($r.x+$r.w/2) ($r.y+$r.h/2)
 $limit=[DateTime]::UtcNow.AddSeconds(10)
 do{$dialog=[ReleaseDialog]::Find($script:app.Id);Start-Sleep -Milliseconds 100}until($dialog-ne[IntPtr]::Zero-or[DateTime]::UtcNow-gt$limit)
 if($dialog-eq[IntPtr]::Zero){throw 'No native Save dialog'}
 [void][MdcardUi]::SetForegroundWindow($dialog);Start-Sleep -Milliseconds 200
 Chord @(18,78);Chord @(17,65);[Windows.Forms.Clipboard]::SetText($destination);Chord @(17,86);Chord @(13)
 $limit=[DateTime]::UtcNow.AddSeconds(60)
 do{Start-Sleep -Milliseconds 150;$s=Get-Content $report -Raw|ConvertFrom-Json}until((!$s.running-and$s.last.type-eq'complete')-or[DateTime]::UtcNow-gt$limit)
 if($s.last.type-ne'complete'){throw ('GUI export did not complete: '+($s|ConvertTo-Json -Depth 8 -Compress))}
 Shot 'complete';if(!(Test-Path $destination)){throw 'No real output file'}
 if((Get-FileHash $source).Hash.ToLower()-ne$before){throw 'Source mutated during export'}
 @{recorded_at=[DateTimeOffset]::UtcNow.ToString('o');build_id='icon-suite-20261024-r3';binary_sha256=(Get-FileHash $Exe).Hash.ToLower();source_sha256=$before;output_sha256=(Get-FileHash $destination).Hash.ToLower();output_bytes=(Get-Item $destination).Length;native_dialog_used=$true;real_gui_export=$true;output=$destination;final_state=$s}|ConvertTo-Json -Depth 8|Set-Content (Join-Path $output 'provenance.json') -Encoding utf8
 Keys '^q';if(!$script:app.WaitForExit(6000)){throw 'No clean app exit'}
}finally{if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()};if($clipboard){[Windows.Forms.Clipboard]::SetDataObject($clipboard,$true)};$env:MDCARD_EXPORT_REPORT=$oldReport}
Write-Output $output
