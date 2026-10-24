# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe,[string]$Stage='final-ru')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-search-master-windows.ps1" -Exe $Exe -LoadFunctionsOnly
. "$PSScriptRoot\workbench-native-input.ps1"
$output=Join-Path $root "evidence\workbench-redesign-2026-10-22\dual-$Stage";$fixtures=Join-Path $output 'workspace';New-Item -ItemType Directory -Force $output,$fixtures|Out-Null
$settings=Join-Path $output 'settings.json';$report=Join-Path $output 'state.json';$uiReport=Join-Path $output 'ui.json';$a=Join-Path $fixtures 'main.md';$b=Join-Path $fixtures 'side-这是一个用于验证窄窗截断与完整路径提示的长文件名.md'
[IO.File]::WriteAllText($a,"# Main document`n`n中文主稿。",$utf8);[IO.File]::WriteAllText($b,"# Second document`n`n中文副稿。",$utf8);$firstHash=(Get-FileHash $a).Hash;$secondHash=(Get-FileHash $b).Hash
@{language='ru';appearance='Dark';reduce_motion=$true;gpu_acceleration=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$env:MDCARD_UI_REPORT=$uiReport;$savedClipboard=[Windows.Forms.Clipboard]::GetDataObject()
function UiState {Get-Content -LiteralPath $uiReport -Raw|ConvertFrom-Json}
function Capture([string]$Name){[void][MdcardUi]::SetCursorPos(5,5);Start-Sleep -Milliseconds 450;Shot $Name;Copy-Item $uiReport (Join-Path $output "$Name.json");Copy-Item $report (Join-Path $output "$Name-search.json")}
$result=@{verification_id=[Guid]::NewGuid().ToString();binary_sha256=(Get-FileHash $Exe).Hash;language='ru';appearance='Dark'}
try{
 StartReview;Keys '^p';Fill 'quick-query' 'side-';Await {param($s)!$s.query_pending-and$s.quick.Count-eq1} 'Dual long-name target'|Out-Null;Keys '+{ENTER}';Await {param($s)$null-ne$s.secondary} 'Secondary open'|Out-Null
 Control 'secondary-editor';Keys '^{END}';Paste ' secondary-dirty';Await {param($s)$s.secondary.dirty} 'Dirty secondary'|Out-Null
 Click 450 250;Keys '^{END}';Paste ' main-dirty';Await {param($s)$s.text.EndsWith('main-dirty')} 'Dirty main'|Out-Null
 Capture '01-dual-dirty-wide'
 $s=State;$r=@($s.controls|Where-Object name -eq 'secondary-editor')|Select-Object -Last 1;$divider=$r.rect[0]-16.0;$bounds=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0
 $target=(UiState).content_rect[2]-240.0
 [void][MdcardUi]::SetCursorPos($bounds.Left+[int]($divider*$scale),$bounds.Top+[int](300*$scale));Start-Sleep -Milliseconds 180;[MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
 foreach($step in 1..10){[void][MdcardUi]::SetCursorPos($bounds.Left+[int](($divider+($target-$divider)*$step/10)*$scale),$bounds.Top+[int](300*$scale));Start-Sleep -Milliseconds 40};[MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 500
 Capture '02-minimum-secondary-dirty';$before=(UiState).primary_content_width;Start-Sleep -Seconds 2;$after=(UiState).primary_content_width
 if([Math]::Abs($before-$after)-gt0.5-or$after-lt300){throw 'Native dual pane drifted or squeezed out main document'}
 [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,40,30,[int](800*$scale),[int](600*$scale),$true);Capture '03-compact-dual-dirty'
 if((Get-FileHash $a).Hash-ne$firstHash-or(Get-FileHash $b).Hash-ne$secondHash){throw 'Layout modified isolated source files'}
 $result.passed=$true;$result.source_files_unchanged=$true;$result.primary_width_before=$before;$result.primary_width_after=$after;$result.close_scope='Own isolated dirty test process terminated after evidence; normal close protection verified separately.'
}catch{$result.error=$_.Exception.Message;$result.passed=$false}
finally{if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()};if($savedClipboard){[Windows.Forms.Clipboard]::SetDataObject($savedClipboard,$true)};$result|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8}
if(!$result.passed){throw 'Dual-pane native layout verification failed'}
