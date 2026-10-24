# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
    [string]$Exe = "$PSScriptRoot\..\dist\windows\MDcard.exe",
    [string]$EvidenceName = 'baseline-active-pane',
    [switch]$RequireFixed
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\verify-search-master-windows.ps1" -Exe $Exe -LoadFunctionsOnly
$output = Join-Path $root "evidence\bug-audit\master-review\$EvidenceName"
$fixtures = Join-Path $output 'workspace'
New-Item -ItemType Directory -Force $output,$fixtures | Out-Null
Remove-Item -LiteralPath (Join-Path $output 'result.json') -ErrorAction SilentlyContinue
$settings = Join-Path $output 'settings.json'
$report = Join-Path $output 'state.json'
$a = Join-Path $fixtures 'main.md'
$b = Join-Path $fixtures 'second.md'
$original = "# Main`ncat-main`n"
$other = "# Second`ncat-secondary`n"
[IO.File]::WriteAllText($a,$original,$utf8)
[IO.File]::WriteAllText($b,$other,$utf8)
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15} | ConvertTo-Json | Set-Content -LiteralPath $settings -Encoding utf8
$priorClipboard = [Windows.Forms.Clipboard]::GetDataObject()
try {
    StartReview
    Keys '^p'
    Fill 'quick-query' 'second.md'
    Await {param($s) @($s.quick).Count -eq 1} 'Second document appears in switcher' | Out-Null
    Keys '+{ENTER}'
    Await {param($s) [bool]$s.secondary} 'Second document is open' | Out-Null
    Control 'secondary-editor'
    Keys '^h'
    Fill 'find-query' 'cat'
    Fill 'replace-query' 'dog'
    $found = Await {param($s) $s.results -eq 1 -and !$s.error} 'Single document query completed'
    Shot '01-current-document-search'
    $foundSecondary = $found.groups[0].path.EndsWith('second.md')
    Control 'preview'
    Await {param($s) $s.preview_files -eq 1} 'Single diff is ready' | Out-Null
    Control 'apply'
    $after = Await {param($s) !$s.open -and !$s.busy} 'Single replace completed'
    Shot '02-current-document-replaced'
    $replacedSecondary = $after.secondary.text -ceq $other.Replace('cat','dog') -and $after.text -ceq $original
    $undoPassed = $null
    $redoPassed = $null
    if ($RequireFixed -and $foundSecondary -and $replacedSecondary) {
        Keys '^z'
        $undone = Await {param($s) $s.secondary.text -ceq $other} 'Undo restores secondary document'
        $undoPassed = $undone.text -ceq $original
        Keys '^+z'
        $redone = Await {param($s) $s.secondary.text -ceq $other.Replace('cat','dog')} 'Redo restores secondary replacement'
        $redoPassed = $redone.text -ceq $original
        Keys '^s'
        Await {param($s) !$s.busy -and !$s.secondary.dirty} 'Secondary replacement saves' | Out-Null
        Same ([IO.File]::ReadAllText($b)) ($other.Replace('cat','dog')) 'Secondary disk result'
        Same ([IO.File]::ReadAllText($a)) $original 'Main disk unchanged'
    }
    @{
        verification_id = [Guid]::NewGuid().ToString()
        exe_sha256 = (Get-FileHash -LiteralPath $Exe).Hash
        searched_secondary = $foundSecondary
        replaced_secondary_only = $replacedSecondary
        groups = $found.groups
        main_after = $after.text
        secondary_after = $after.secondary.text
        undo_passed = $undoPassed
        redo_passed = $redoPassed
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
    if ($RequireFixed -and (!$foundSecondary -or !$replacedSecondary)) { throw 'Current document search/replace targeted the main pane while secondary was active' }
} finally {
    if ($script:app -and !$script:app.HasExited) { $script:app.Kill(); $script:app.WaitForExit() }
    if ($priorClipboard) { [Windows.Forms.Clipboard]::SetDataObject($priorClipboard, $true) }
}
Get-Content -LiteralPath (Join-Path $output 'result.json') -Raw
