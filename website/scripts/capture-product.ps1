# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
$ErrorActionPreference='Stop'
$site=Split-Path $PSScriptRoot
$repo=Split-Path $site
$Exe=Join-Path $repo 'dist/windows/MDcard.exe'
. (Join-Path $repo 'scripts/verify-ui-gpu-windows.ps1') -Exe $Exe -FunctionsOnly
$output=Join-Path $site 'evidence/product'
New-Item -ItemType Directory -Force $output|Out-Null
$settings=Join-Path $output 'settings.json'
$source=Join-Path $site 'samples/从原稿到作品.md'
$workspace=Join-Path $output '一份文档的旅程'
New-Item -ItemType Directory -Force $workspace|Out-Null
$fixture=Join-Path $workspace '从原稿到作品.md'
$final=[IO.File]::ReadAllText($source)
[IO.File]::WriteAllText($fixture,$final.Replace('交付状态：已校对。','交付状态：待校对。'),[Text.UTF8Encoding]::new($false))
$before=(Get-FileHash $fixture).Hash.ToLower()
@{language='zh-CN';appearance='Light';accent=0;reduce_motion=$true;font_size=17;gpu_acceleration=$true}|ConvertTo-Json|Set-Content $settings -Encoding utf8
$env:MDCARD_UI_REPORT=Join-Path $output 'ui.json'
function Capture($name){[void][MdcardUi]::SetCursorPos(5,5); Shot $name; Copy-Item $env:MDCARD_UI_REPORT (Join-Path $output "$name.json")}
try {
 Launch 'renderer' '' $fixture
 $scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0
 [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,20,20,[int](1360*$scale),[int](860*$scale),$true)
 Start-Sleep -Seconds 2
 Keys '^e'; Keys '^e'
 Click 430 220; Keys '^a'
 $clip=[Windows.Forms.Clipboard]::GetDataObject()
 try {[Windows.Forms.Clipboard]::SetText($final); Keys '^v'} finally {if($clip){[Windows.Forms.Clipboard]::SetDataObject($clip,$true)}}
 Keys '^s'; Keys '^{HOME}'; Capture 'write'
 if([IO.File]::ReadAllText($fixture)-cne$final){throw 'Real editor save did not match final manuscript'}
 Keys '^e'; Start-Sleep -Seconds 4; Capture 'split'
 Keys '^e'; Capture 'read'; Keys '^e'; Keys '^e'
 Keys '^+t'; Capture 'structure'; Keys '^+t'
 Keys '^f'; Capture 'search'; Keys '{ESC}'
 Keys '^%e'; Capture 'export'; Keys '{ESC}'
 Keys '^q'; if(!$script:app.WaitForExit(6000)){throw 'No clean app exit'}
 @{recorded_at=[DateTimeOffset]::UtcNow.ToString('o');build_id='icon-suite-20261024-r3';binary_sha256=(Get-FileHash $Exe).Hash.ToLower();binary_source_commit='9fe090ea938a35b9f5d14ea647d7377b65fd5ab4';manuscript='samples/从原稿到作品.md';before_sha256=$before;after_sha256=(Get-FileHash $fixture).Hash.ToLower();edited_via_actual_gui=$true;saved_content_matches_sample=$true;real_windows_gui=$true;dpi=($scale*96);window_logical=@(1360,860);captures=@('write','split','read','structure','search','export')} |ConvertTo-Json -Depth 5|Set-Content (Join-Path $output 'provenance.json') -Encoding utf8
} finally {if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()}}
Write-Output $output
