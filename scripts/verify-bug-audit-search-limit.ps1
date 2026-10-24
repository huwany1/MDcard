# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\verify-search-master-windows.ps1" -Exe $Exe -LoadFunctionsOnly
$output = Join-Path $root 'evidence\bug-audit\master-review\search-limit'
$fixtures = Join-Path $output 'workspace'
New-Item -ItemType Directory -Force $output,$fixtures | Out-Null
Remove-Item -LiteralPath (Join-Path $output 'result.json') -ErrorAction SilentlyContinue
$settings = Join-Path $output 'settings.json'
$report = Join-Path $output 'state.json'
$a = Join-Path $fixtures 'overflow.md'
$original = "# Overflow`n`n" + ('needle ' * 10001)
[IO.File]::WriteAllText($a,$original,$utf8)
foreach ($name in @('TARGET','.GIT','Node_Modules','.MDCARD-HISTORY')) {
    $folder = Join-Path $fixtures $name
    New-Item -ItemType Directory -Force $folder | Out-Null
    [IO.File]::WriteAllText((Join-Path $folder 'hidden.md'),'hidden-only-query',$utf8)
}
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15} | ConvertTo-Json | Set-Content -LiteralPath $settings -Encoding utf8
$priorClipboard = [Windows.Forms.Clipboard]::GetDataObject()
try {
    StartReview
    Keys '^f'
    Fill 'find-query' 'needle'
    $found = Await {param($s) $s.results -eq 10000 -and $s.results_limited -and !$s.query_pending} 'Visible truncation status for 10001 matches'
    Shot '01-search-truncation-warning'
    Keys '{ESC}'
    Keys '^+f'
    Fill 'find-query' 'hidden-only-query'
    $hidden = Await {param($s) $s.query -eq 'hidden-only-query' -and !$s.query_pending -and $s.results -eq 0 -and $s.index.ready} 'Ignored directories are excluded'
    if ($hidden.index.files -ne 1) { throw "Ignored directories were indexed: $($hidden.index.files) files" }
    Shot '02-uppercase-directories-excluded'
    Keys '{ESC}'
    StopReview
    if ([IO.File]::ReadAllText($a) -cne $original) { throw 'Search modified source' }
    @{
        verification_id = [Guid]::NewGuid().ToString()
        passed = $true
        exe_sha256 = (Get-FileHash -LiteralPath $Exe).Hash
        matches = $found.results
        results_limited = $found.results_limited
        uppercase_ignored_directories = 4
        indexed_files = $hidden.index.files
        ignored_query_results = $hidden.results
        source_unchanged = $true
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
} finally {
    if ($script:app -and !$script:app.HasExited) { $script:app.Kill(); $script:app.WaitForExit() }
    if ($priorClipboard) { [Windows.Forms.Clipboard]::SetDataObject($priorClipboard, $true) }
}
Get-Content -LiteralPath (Join-Path $output 'result.json') -Raw
