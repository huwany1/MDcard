# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe,[string]$Language='zh-CN',[string]$Appearance='Light')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
$output=Join-Path $root "evidence\workbench-redesign-2026-10-22\empty-final\$Language-$Appearance"
$empty=Join-Path $output 'empty-workspace';New-Item -ItemType Directory -Force $output,$empty|Out-Null
$settings=Join-Path $output 'settings.json';$uiReport=Join-Path $output 'ui.json';$stateReport=Join-Path $output 'state.json'
@{language=$Language;appearance=$Appearance;reduce_motion=$true;gpu_acceleration=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
function UiState {Get-Content -Raw -LiteralPath $uiReport|ConvertFrom-Json}
function Named([string]$Name){$r=@((UiState).controls|Where-Object name -eq $Name)|Select-Object -Last 1;if(!$r){throw "No native control $Name"};$b=Bounds;$s=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0;[void][MdcardUi]::SetCursorPos($b.Left+[int](($r.rect[0]+$r.rect[2])*.5*$s),$b.Top+[int](($r.rect[1]+$r.rect[3])*.5*$s));Start-Sleep -Milliseconds 180;[MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 60;[MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 300}
function Capture([string]$Name){[void][MdcardUi]::SetCursorPos(5,5);Start-Sleep -Milliseconds 400;Shot $Name;Copy-Item -LiteralPath $uiReport -Destination (Join-Path $output "$Name.json")}
$result=@{verification_id=[Guid]::NewGuid().ToString();binary_sha256=(Get-FileHash $Exe).Hash;language=$Language;appearance=$Appearance}
try{
 $info=[Diagnostics.ProcessStartInfo]::new((Resolve-Path $Exe).Path);$info.UseShellExecute=$false;$info.WorkingDirectory=$empty
 $info.Environment['MDCARD_SETTINGS_PATH']=$settings;$info.Environment['MDCARD_UI_REPORT']=$uiReport;$info.Environment['MDCARD_SEARCH_REPORT']=$stateReport
 $script:app=[Diagnostics.Process]::Start($info);Start-Sleep -Seconds 3;$script:app.Refresh();if($script:app.MainWindowHandle-eq0){throw 'No empty-workspace native window'}
 $dpi=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle);$scale=$dpi/96.0
 [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,40,30,[int](1280*$scale),[int](800*$scale),$true);[void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-1),0,0,0,0,0x43);[void][MdcardUi]::SetForegroundWindow($script:app.MainWindowHandle);Start-Sleep -Milliseconds 400
 Capture '01-empty-workspace';Keys '^n';Capture '02-untitled'
 Named 'navigation:Outline';Capture '03-empty-outline';Named 'navigation:Recent';Capture '04-empty-recent';Named 'navigation:Files'
 [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,40,30,[int](800*$scale),[int](600*$scale),$true);Capture '05-compact-untitled'
 $state=Get-Content -LiteralPath $stateReport -Raw|ConvertFrom-Json;$ui=UiState
 if($state.text-ne''-or$null-ne$state.path-or$ui.dirty-or$ui.workspace_files-ne0){throw 'Empty view changed the unnamed draft or index'}
 Keys '^q';$result.clean_exit=$script:app.WaitForExit(5000);$result.passed=$result.clean_exit;$result.dpi=$dpi;$result.empty_source_unchanged=$true
}catch{$result.error=$_.Exception.Message;$result.passed=$false}
finally{if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()};$result|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8}
if(!$result.passed){throw 'Empty workspace/unnamed draft verification failed'}
