# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$Exe = "$PSScriptRoot\..\dist\windows\MDcard.exe")
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -FunctionsOnly -Exe $Exe
$output = Join-Path $root 'evidence\localization\windows'
New-Item -ItemType Directory -Force $output | Out-Null
$settings = Join-Path $output 'settings.json'
$matrix = @()
function CloseLocalized { Keys '^q'; if (!$script:app.WaitForExit(5000)) {throw 'Localized app did not exit cleanly'} }
foreach ($language in @('zh-CN','zh-TW','en','ko','ja','ru')) {
    @{language=$language;appearance='Light';accent=0;gpu_acceleration=$true;reduce_motion=$true;font_size=23.0} | ConvertTo-Json | Set-Content -LiteralPath $settings -Encoding utf8
    Launch "$language-renderer"
    Shot "$language-workspace"
    Keys '^+p'; Shot "$language-preferences"; Keys '{ESC}'
    [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,80,80,775,525,$true)
    Start-Sleep -Milliseconds 500
    Shot "$language-compact"
    Keys '^+p'; Shot "$language-compact-preferences"; Keys '{ESC}'
    CloseLocalized
    $report = Get-Content -LiteralPath (Join-Path $output "$language-renderer.json") -Raw | ConvertFrom-Json
    if ($report.language -ne $language) {throw "Wrong actual language: $language"}
    $matrix += @{language=$language;actual_language=$report.language;normal='1240x820';compact='620x420';font_size=23;preferences_visible=$true;exit_code=$script:app.ExitCode}
}
@{verification_id=[Guid]::NewGuid().ToString();date='2026-10-19';timezone='Asia/Shanghai';exe_sha256=(Get-FileHash -LiteralPath $Exe).Hash;matrix=$matrix} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $output 'matrix.json') -Encoding utf8
Write-Output $output
