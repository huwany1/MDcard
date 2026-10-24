# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe,[string]$Stage='r4')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-search-master-windows.ps1" -Exe $Exe -LoadFunctionsOnly
$output=Join-Path $root "evidence\ui-polish-2026-10-22\master\modes-$Stage"
$fixtures=Join-Path $output 'workspace';New-Item -ItemType Directory -Force $output,$fixtures|Out-Null
$settings=Join-Path $output 'settings.json';$report=Join-Path $output 'state.json';$a=Join-Path $fixtures 'modes.md'
$env:MDCARD_UI_REPORT=Join-Path $output 'ui.json'
$original="# 阅读正文必须可见`n`n中文 English 2026 混排随可用区域换行。`n`n## 表格后的正文`n`n| heading | value |`n| --- | --- |`n| $('W'*180) | cell |`n`n这段正文不能受宽表格影响。`n`n$('阅读区域变窄时自动换行。 Wider view uses comfortable lines. '*20)`n"
[IO.File]::WriteAllText($a,$original,$utf8)
@{language='zh-CN';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$record=@{verification_id=[Guid]::NewGuid().ToString();stage=$Stage;binary_sha256=(Get-FileHash -LiteralPath $Exe).Hash;states=@()}
function CaptureMode([string]$Name){Shot $Name;$s=Get-Content -Raw -LiteralPath $env:MDCARD_UI_REPORT|ConvertFrom-Json;$record.states+=@{name=$Name;mode=$s.mode;content_rect=$s.content_rect;can_split=$s.can_split;dirty=$s.dirty};Copy-Item -LiteralPath $env:MDCARD_UI_REPORT -Destination (Join-Path $output "$Name.json")}
try {
 StartReview;CaptureMode '01-split-wide';Keys '^e';CaptureMode '02-read-wide'
 [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,50,50,1000,750,$true);CaptureMode '03-read-narrow'
 Keys '^e';CaptureMode '04-write-narrow';Keys '^e';CaptureMode '05-requested-split-narrow'
 [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,50,50,1550,1025,$true);CaptureMode '06-restored-split-wide'
 $record.memory_unchanged=((State).text-ceq$original);$record.disk_unchanged=([IO.File]::ReadAllText($a)-ceq$original)
 Keys '^q';$record.clean_exit=$script:app.WaitForExit(5000)
} catch {$record.error=$_.Exception.Message}
finally {if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()};$record|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8}
$record|ConvertTo-Json -Depth 6
