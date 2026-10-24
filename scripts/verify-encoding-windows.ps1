# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$Exe = "$PSScriptRoot\..\dist\windows\MDcard.exe")
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
$output = Join-Path $root 'evidence\encoding\windows-final'
$fixture = Join-Path $root 'artifacts\encoding'
New-Item -ItemType Directory -Force $output,$fixture | Out-Null
$settings = Join-Path $output 'settings.json'
$verificationId = [Guid]::NewGuid().ToString()
function WriteSettings([string]$Language) {
    @{language=$Language;appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15} |
        ConvertTo-Json | Set-Content -LiteralPath $settings -Encoding utf8
}
function CloseClean {
    Keys '^q'
    if (!$script:app.WaitForExit(5000)) { throw 'Test window did not exit cleanly' }
}
function AssertDirty([bool]$Expected) {
    $script:app.Refresh()
    if (($script:app.MainWindowTitle -match '●') -ne $Expected) { throw "Unexpected dirty title: $($script:app.MainWindowTitle)" }
}
function SameBytes([string]$A,[string]$B) {
    if ([Convert]::ToHexString([IO.File]::ReadAllBytes($A)) -ne [Convert]::ToHexString([IO.File]::ReadAllBytes($B))) { throw "Byte mismatch: $A / $B" }
}
python -c "from pathlib import Path; p=Path(r'$fixture'); text=('这是简体中文编码测试文档。支持本地编码，保存和读取文件。𠀀🦀\r\n'*4); (p/'w.txt').write_bytes(text.encode('gb18030')); (p/'w-utf8.txt').write_bytes(text.encode('utf-8')); (p/'n.txt').write_bytes('# 无 BOM 文件\r\n中文与保存\r\n'.encode('utf-16-le'))"
if ($LASTEXITCODE -ne 0) { throw 'Independent Python fixture generation failed' }
WriteSettings 'en'
try {
    Launch 'gb18030-renderer' '' (Join-Path $fixture 'w.txt')
    Shot 'gb18030-guessed'
    Click 1160 804; Shot 'encoding-grid'
    # Stable English modal at 1240 x 820 logical pixels, Windows 125% DPI.
    Click 440 398
    Shot 'utf8-selected'
    Click 700 608
    AssertDirty $true
    Shot 'conversion-pending'
    Keys '^n'; Shot 'conversion-unsaved-guard'; Keys '{ESC}'
    AssertDirty $true
    Keys '^s'; AssertDirty $false
    SameBytes (Join-Path $fixture 'w.txt') (Join-Path $fixture 'w-utf8.txt')
    Shot 'converted-utf8'
    CloseClean

    Launch 'utf16-renderer' '' (Join-Path $fixture 'n.txt')
    Shot 'utf16-opening-error'
    Click 480 440
    Shot 'manual-open-mode'
    # UTF-16 LE is the top-right option in the visible grid.
    Click 685 376
    Click 550 586
    $script:app.Refresh()
    if ($script:app.MainWindowTitle -notmatch 'n.txt') { throw 'Manual UTF-16 reopening failed' }
    Shot 'utf16-manual-opened'
    $before = [IO.File]::ReadAllBytes((Join-Path $fixture 'n.txt'))
    Keys '^s'
    if ([Convert]::ToHexString($before) -ne [Convert]::ToHexString([IO.File]::ReadAllBytes((Join-Path $fixture 'n.txt')))) { throw 'Unchanged UTF-16 bytes changed' }
    Click 400 230; Keys '^{END}'; Keys 'UNSAVED'
    Click 1160 804; Shot 'dirty-encoding-grid'
    # With a UTF-16 BOM checkbox the modal has one more row.
    Click 550 605
    Shot 'dirty-reopen-guard'
    Keys '{ESC}'; AssertDirty $true
    Shot 'dirty-cancel-retained'
    Keys '^s'; AssertDirty $false
    python -c "from pathlib import Path; p=Path(r'$fixture/n.txt'); assert p.read_bytes() == ('# 无 BOM 文件\r\n中文与保存\r\nUNSAVED').encode('utf-16-le')"
    if ($LASTEXITCODE -ne 0) { throw 'UTF-16 edit/save did not preserve independent reference bytes' }
    CloseClean

    foreach ($language in @('zh-CN','zh-TW','en','ko','ja','ru')) {
        WriteSettings $language
        Launch "locale-$language-renderer" '' (Join-Path $fixture 'w-utf8.txt')
        Click 1160 804; Shot "locale-$language"
        # Verify the entire encoding modal remains within the compact native window.
        [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,80,80,850,625,$true)
        Start-Sleep -Milliseconds 600
        Shot "compact-$language"
        Keys '{ESC}'
        # Menu entry is independently visible in the compact window.
        Click 230 82; Shot "compact-menu-$language"; Keys '{ESC}'
        CloseClean
    }
    @{
        verification_id=$verificationId;date='2026-10-19';timezone='Asia/Shanghai'
        exe=(Resolve-Path -LiteralPath $Exe).Path;exe_sha256=(Get-FileHash -LiteralPath $Exe).Hash
        gb18030_auto_detection=$true;manual_utf16_no_bom=$true;conversion_pending_dirty=$true
        cancel_retains_dirty=$true;python_reference_bytes_exact=$true
        locales=@('zh-CN','zh-TW','en','ko','ja','ru');compact_logical='680x500'
        real_keyboard_mouse=$true;all_test_windows_closed=$true;settings_path=$settings
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
    Write-Output $output
} finally {
    if ($script:app -and !$script:app.HasExited) { $script:app.Kill();$script:app.WaitForExit() }
}
