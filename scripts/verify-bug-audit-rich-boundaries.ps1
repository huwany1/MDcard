# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
    [string]$Exe = "$PSScriptRoot\..\dist\windows\MDcard.exe",
    [string]$EvidenceName = 'baseline-rich-boundaries',
    [ValidateSet('Save', 'New')][string]$Action = 'Save',
    [switch]$RequireFixed
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
$output = Join-Path $root "evidence\bug-audit\master-review\$EvidenceName"
New-Item -ItemType Directory -Force $output | Out-Null
Remove-Item -LiteralPath (Join-Path $output 'result.json') -ErrorAction SilentlyContinue
$settings = Join-Path $output 'settings.json'
$fixture = Join-Path $output 'pending.md'
$richReport = Join-Path $output 'rich.json'
$searchReport = Join-Path $output 'state.json'
$source = "# Pending edit boundary`n`nBefore `$x^2`$ after.`n"
[IO.File]::WriteAllText($fixture, $source, [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText($settings, '{"language":"en","appearance":"Light","gpu_acceleration":true,"reduce_motion":true,"font_size":15}')
$priorRich = $env:MDCARD_RICH_REPORT
$priorSearch = $env:MDCARD_SEARCH_REPORT
$priorClipboard = [Windows.Forms.Clipboard]::GetDataObject()
$env:MDCARD_RICH_REPORT = $richReport
$env:MDCARD_SEARCH_REPORT = $searchReport
function RichState {
    for ($attempt = 0; $attempt -lt 30; $attempt++) {
        try { return Get-Content -LiteralPath $richReport -Raw | ConvertFrom-Json } catch { Start-Sleep -Milliseconds 20 }
    }
    throw 'No rich report'
}
function RichControl([string]$Name) {
    $until = [DateTime]::UtcNow.AddSeconds(12)
    do {
        $control = (RichState).controls | Where-Object name -eq $Name | Select-Object -Last 1
        if ($control) { return $control.rect }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $until)
    throw "Missing $Name"
}
function ClickRich([string]$Name) {
    $rect = RichControl $Name
    Click (($rect[0] + $rect[2]) / 2) (($rect[1] + $rect[3]) / 2)
}
function QuickClick([double]$X, [double]$Y) {
    $bounds = Bounds
    $scale = [MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle) / 96.0
    [void][MdcardUi]::SetCursorPos($bounds.Left + [int]($X * $scale), $bounds.Top + [int]($Y * $scale))
    [MdcardUi]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
    [MdcardUi]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
}
try {
    Launch 'renderer' '' $fixture
    ClickRich 'node-0'
    $focusVerified = $false
    for ($attempt = 0; $attempt -lt 5; $attempt++) {
        ClickRich 'floating-editor'
        Keys '^a'
        $lastSelect = @((RichState).inputEvents | Where-Object { $_.StartsWith('key:A:') }) | Select-Object -Last 1
        if ($lastSelect -match 'focus=floating$') { $focusVerified = $true; break }
    }
    if (!$focusVerified) { throw 'Floating source editor never received keyboard focus' }
    Set-Clipboard -Value 'z^3'
    $timer = [Diagnostics.Stopwatch]::StartNew()
    [Windows.Forms.SendKeys]::SendWait('^v')
    Start-Sleep -Milliseconds 40
    QuickClick $(if ($Action -eq 'Save') { 175 } else { 50 }) 82
    $clickMs = $timer.Elapsed.TotalMilliseconds
    if ($clickMs -ge 250) { throw 'Rapid command missed the 250ms debounce window' }
    Start-Sleep -Milliseconds 700
    $disk = [IO.File]::ReadAllText($fixture)
    Shot 'toolbar-save-after-pending-formula'
    $savedLatest = $disk.Contains('$z^3$')
    $state = Get-Content -LiteralPath $searchReport -Raw | ConvertFrom-Json
    $retainedLatest = $state.text.Contains('$z^3$') -and $state.path.EndsWith('pending.md')
    $pasteEvent = @((RichState).inputEvents | Where-Object { $_.StartsWith('paste:3 ') }) | Select-Object -Last 1
    if ($pasteEvent -notmatch 'focus=floating$') { throw 'Paste was not delivered to the floating source editor' }
    @{
        verification_id = [Guid]::NewGuid().ToString()
        exe = (Resolve-Path -LiteralPath $Exe).Path
        exe_sha256 = (Get-FileHash -LiteralPath $Exe).Hash
        paste_to_save_click_ms = $clickMs
        input_focus_verified = $focusVerified
        paste_event = $pasteEvent
        saved_latest_formula = $savedLatest
        action = $Action
        retained_latest_formula = $retainedLatest
        memory_after_click = $state.text
        path_after_click = $state.path
        original = $source
        disk_after_click = $disk
        title_after_click = $script:app.MainWindowTitle
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
    if ($RequireFixed -and $Action -eq 'Save' -and !$savedLatest) { throw 'Toolbar Save omitted pending formula edit' }
    if ($RequireFixed -and $Action -eq 'New' -and !$retainedLatest) { throw 'Toolbar New discarded pending formula edit' }
} finally {
    if ($script:app -and !$script:app.HasExited) {
        Keys '{ESC}'
        if ($Action -eq 'Save' -or $retainedLatest) { Keys '^s' }
        Keys '^q'
        if (!$script:app.WaitForExit(5000)) { $script:app.Kill(); $script:app.WaitForExit() }
    }
    if ($null -eq $priorRich) { Remove-Item Env:MDCARD_RICH_REPORT -ErrorAction SilentlyContinue } else { $env:MDCARD_RICH_REPORT = $priorRich }
    if ($null -eq $priorSearch) { Remove-Item Env:MDCARD_SEARCH_REPORT -ErrorAction SilentlyContinue } else { $env:MDCARD_SEARCH_REPORT = $priorSearch }
    if ($priorClipboard) { [Windows.Forms.Clipboard]::SetDataObject($priorClipboard, $true) }
}
Get-Content -LiteralPath (Join-Path $output 'result.json') -Raw
