# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe,[string]$Stage='package')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-search-master-windows.ps1" -Exe $Exe -LoadFunctionsOnly
Add-Type @'
using System;using System.Runtime.InteropServices;
public class MdcardWindowReview {
 [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr h);
 [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int cmd);
}
'@
$output=Join-Path $root "evidence\ui-polish-2026-10-22\master\window-controls-$Stage"
$fixtures=Join-Path $output 'workspace';New-Item -ItemType Directory -Force $output,$fixtures|Out-Null
$settings=Join-Path $output 'settings.json';$report=Join-Path $output 'state.json';$a=Join-Path $fixtures 'protected.md'
$env:MDCARD_UI_REPORT=Join-Path $output 'ui.json'
$original="# Window controls`n`n中文原稿";[IO.File]::WriteAllText($a,$original,$utf8)
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$priorClipboard=[Windows.Forms.Clipboard]::GetDataObject();$record=@{verification_id=[Guid]::NewGuid().ToString();binary_sha256=(Get-FileHash -LiteralPath $Exe).Hash;stage=$Stage}
function UiClick([string]$Name){$s=Get-Content -Raw -LiteralPath $env:MDCARD_UI_REPORT|ConvertFrom-Json;$r=@($s.controls|Where-Object {$_.name-ceq$Name-and$_.enabled})|Select-Object -Last 1;if(!$r){throw "No enabled control $Name"};Click (($r.rect[0]+$r.rect[2])/2) (($r.rect[1]+$r.rect[3])/2)}
try {
 StartReview;$handle=$script:app.MainWindowHandle
 Click 92 24;Start-Sleep -Milliseconds 400;$record.green_maximizes=[MdcardWindowReview]::IsZoomed($handle);Shot '01-maximized'
 Click 92 24;Start-Sleep -Milliseconds 400;$record.green_restores=![MdcardWindowReview]::IsZoomed($handle);Shot '02-restored'
 Click 60 24;Start-Sleep -Milliseconds 400;$record.yellow_minimizes=[MdcardWindowReview]::IsIconic($handle)
 [void][MdcardWindowReview]::ShowWindow($handle,9);[void][MdcardUi]::SetForegroundWindow($handle);Start-Sleep -Milliseconds 400
 $record.restores_from_minimized=![MdcardWindowReview]::IsIconic($handle)
 $before=Bounds;$scale=[MdcardUi]::GetDpiForWindow($handle)/96.;$x=$before.Left+[int](500*$scale);$y=$before.Top+[int](24*$scale)
 [void][MdcardUi]::SetCursorPos($x,$y);Start-Sleep -Milliseconds 100;[MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 100
 for($i=1;$i-le10;$i++){[void][MdcardUi]::SetCursorPos($x+5*$i,$y+3*$i);Start-Sleep -Milliseconds 30}
 [MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 300;$after=Bounds
 $record.title_drag_moves_window=($after.Left-ne$before.Left-or$after.Top-ne$before.Top);Shot '03-title-drag'
 Click 450 250;Keys '^{END}';Paste '-UNSAVED';Click 28 24;UiClick 'Cancel'
 $record.red_cancel_preserves_dirty=((State).text-ceq($original+'-UNSAVED'))
 Shot '04-close-cancel';Click 28 24;UiClick 'Discard';$record.red_discard_exits=$script:app.WaitForExit(5000)
 $record.disk_protected=([IO.File]::ReadAllText($a)-ceq$original)
 $record.passed=$record.green_maximizes-and$record.green_restores-and$record.yellow_minimizes-and$record.restores_from_minimized-and$record.title_drag_moves_window-and$record.red_cancel_preserves_dirty-and$record.red_discard_exits-and$record.disk_protected
}catch{$record.error=$_.Exception.Message;$record.passed=$false}
finally{if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()};if($priorClipboard){[Windows.Forms.Clipboard]::SetDataObject($priorClipboard,$true)};$record|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8}
$record|ConvertTo-Json -Depth 6
