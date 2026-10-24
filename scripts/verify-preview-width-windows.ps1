# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
    [string]$Exe = "$PSScriptRoot\..\artifacts\preview-width\windows\MDcard.exe",
    [string]$EvidenceName = 'windows',
    [string]$Document = '',
    [ValidateSet('Light', 'Dark')][string]$Appearance = 'Light',
    [switch]$ShellOpen
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
$output = Join-Path $root "evidence\preview-width\$EvidenceName"
$scratch = Join-Path $root "artifacts\preview-width\$EvidenceName"
New-Item -ItemType Directory -Force -Path $output,$scratch | Out-Null
$settings = Join-Path $scratch 'settings.json'
$fixture = Join-Path $scratch 'responsive.md'
$utf8 = [Text.UTF8Encoding]::new($false)
$paragraph = '阅读区域变窄时，这段正文应自动换行；区域变宽时，每行应显示更多内容。 Responsive body follows the available width. '
$table = "| $('W' * 160) | Other |`n| --- | --- |`n| value | value |`n`n"
$text = "# 阅读宽度回归验证`n`n$table## 表格后的正文`n`n$($paragraph * 8)`n`n## 下一段`n`n$($paragraph * 4)"
if ($Document) {
    $fixture = (Resolve-Path -LiteralPath $Document).Path
} else {
    [IO.File]::WriteAllText($fixture, $text, $utf8)
}
@{language='en'; appearance=$Appearance; reduce_motion=$true; font_size=15} |
    ConvertTo-Json | Set-Content -LiteralPath $settings -Encoding utf8
$hashBefore = (Get-FileHash -LiteralPath $fixture).Hash
$captures = @()
function ScrollPreview([int]$Steps) {
    $bounds = Bounds
    [void][MdcardUi]::SetCursorPos(
        $bounds.Left + [int](($bounds.Right - $bounds.Left) * 0.8),
        $bounds.Top + [int](($bounds.Bottom - $bounds.Top) * 0.65))
    for ($index = 0; $index -lt $Steps; $index++) {
        [MdcardUi]::mouse_event(0x800, 0, 0, ([uint32]::MaxValue - 119), [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 120
    }
}
try {
    if ($ShellOpen) {
        $existingIds = @(Get-Process -Name MDcard -ErrorAction SilentlyContinue | ForEach-Object Id)
        Start-Process -FilePath $fixture | Out-Null
        $deadline = [DateTime]::UtcNow.AddSeconds(12)
        do {
            $script:app = Get-Process -Name MDcard -ErrorAction SilentlyContinue |
                Where-Object { $_.Id -notin $existingIds -and $_.MainWindowHandle -ne 0 } |
                Select-Object -First 1
            if ($script:app) { break }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $deadline)
        if (!$script:app) { throw 'File association did not open a new MDcard window' }
        if ($script:app.Path -ne (Resolve-Path -LiteralPath $Exe).Path) {
            throw "File association launched a different program: $($script:app.Path)"
        }
        Start-Sleep -Seconds 2
        $script:app.Refresh()
        [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle, 50, 50, 1550, 1025, $true)
        for ($attempt = 0; $attempt -lt 8; $attempt++) {
            $script:app.Refresh()
            [void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle, [IntPtr]::new(-1), 0, 0, 0, 0, 0x43)
            [void][MdcardUi]::SetForegroundWindow($script:app.MainWindowHandle)
            Start-Sleep -Milliseconds 200
            Click 500 27
            if ([MdcardUi]::GetForegroundWindow() -eq $script:app.MainWindowHandle) { break }
        }
        [void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle, [IntPtr]::new(-2), 0, 0, 0, 0, 0x43)
        if ([MdcardUi]::GetForegroundWindow() -ne $script:app.MainWindowHandle) {
            throw 'File association test window did not receive focus'
        }
    } else {
        Launch 'renderer' '' $fixture
    }
    $openedProcess = @{
        id = $script:app.Id
        path = $script:app.Path
        title = $script:app.MainWindowTitle
        method = $(if ($ShellOpen) { 'Windows ShellExecute file association' } else { 'Direct executable' })
    }
    Keys '^e'
    Shot 'read-wide'
    $captures += 'read-wide'
    if ($Document) {
        ScrollPreview 3
        Shot 'read-table-wide'
        $captures += 'read-table-wide'
    }
    [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle, 80, 80, 775, 700, $true)
    Shot 'read-narrow'
    $captures += 'read-narrow'
    if ($Document) {
        ScrollPreview 5
        Shot 'read-table-narrow'
        $captures += 'read-table-narrow'
    }
    [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle, 50, 50, 1550, 1025, $true)
    Keys '^b'
    Shot 'read-sidebar-hidden'
    $captures += 'read-sidebar-hidden'
    Keys '^b'
    Keys '^e'
    Keys '^e'
    Shot 'split'
    $captures += 'split'
    if ($Document) {
        ScrollPreview 7
        Shot 'split-table'
        $captures += 'split-table'
    }
    $dpi = [MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)
    Keys '^q'
    if (!$script:app.WaitForExit(5000)) { throw 'App did not close cleanly' }
    if ((Get-FileHash -LiteralPath $fixture).Hash -ne $hashBefore) { throw 'Read/resize modified source' }
    @{
        verification_id = [Guid]::NewGuid().ToString()
        binary_sha256 = (Get-FileHash -LiteralPath $Exe).Hash
        process = $openedProcess
        document = $fixture
        document_sha256 = $hashBefore
        source_unchanged = $true
        captures = $captures
        platform = 'Windows native'
        dpi = $dpi
        note = 'Native screenshots require visual review; automated wrapping assertions are in preview_layout_tests.'
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
} finally {
    if ($script:app -and !$script:app.HasExited) { $script:app.Kill(); $script:app.WaitForExit() }
}
Write-Output $output
