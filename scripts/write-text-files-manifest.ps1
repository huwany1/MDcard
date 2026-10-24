# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
function Record([string]$Relative) {
    $path = Join-Path $root $Relative
    $file = Get-Item -LiteralPath $path
    @{path=$Relative.Replace('\','/'); bytes=$file.Length; sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash}
}
$outputs = @('dist\windows\MDcard.exe','dist\linux\mdcard','dist\MDcard-0.1.0-windows-x64.zip','dist\MDcard-0.1.0-linux-x64.tar.gz') | ForEach-Object { Record $_ }
$linuxHash = ((Get-Content -LiteralPath (Join-Path $root 'evidence\text-files\linux\executable.sha256') -Raw).Trim() -split '\s+')[0]
if ($outputs[1].sha256 -ne $linuxHash) { throw 'Linux UI evidence does not match the current executable' }
$windowsProof = Get-Content -LiteralPath (Join-Path $root 'evidence\text-files\master-review\result.json') -Raw | ConvertFrom-Json
if ($windowsProof.status -ne 'passed' -or $outputs[0].sha256 -ne $windowsProof.tested_exe_sha256) { throw 'Windows independent review does not match the current executable' }
$packageProof = Get-Content -LiteralPath (Join-Path $root 'evidence\text-files\package-check.json') -Raw | ConvertFrom-Json
if (!$packageProof.passed) { throw 'Current package verification did not pass' }
foreach ($package in $packageProof.archives) {
    $record = $outputs | Where-Object { $_.path -eq $package.archive.Replace('\','/') }
    if (!$record -or $record.sha256 -ne $package.archive_sha256) { throw "Package changed after verification: $($package.archive)" }
}
$sourcePaths = @('Cargo.toml','Cargo.lock','Dockerfile','README.md','docs\ACCEPTANCE.md')
$sourcePaths += Get-ChildItem -LiteralPath (Join-Path $root 'src'),(Join-Path $root 'locales'),(Join-Path $root 'scripts') -File -Recurse | ForEach-Object { [IO.Path]::GetRelativePath($root,$_.FullName) }
$source = $sourcePaths | ForEach-Object { Record $_ }
$proof = Get-ChildItem -LiteralPath (Join-Path $root 'evidence\text-files') -File -Recurse | Where-Object { $_.Name -ne 'manifest.json' -and $_.Extension -ne '.exe' } | Sort-Object FullName | ForEach-Object { Record ([IO.Path]::GetRelativePath($root,$_.FullName)) }
@{
    verification_id=[Guid]::NewGuid().ToString()
    date='2026-10-19'
    timezone='Asia/Shanghai'
    feature='Relative descendant text-file search, display and editing'
    linux_tests=20
    windows_native_tests=19
    tests_failed=0
    tests_ignored=0
    final_independent_review='evidence/text-files/master-review/result.json'
    final_ui_linux='evidence/text-files/linux/result.json'
    final_package_check='evidence/text-files/package-check.json'
    outputs=$outputs
    source=$source
    evidence=$proof
    historical_localization_manifest='evidence/localization/manifest-before-text-files.json'
    historical_evidence_note='The preserved localization manifest and localization/ui-gpu evidence describe earlier binaries. Intermediate master-review records are identified by their executable hashes; use final confirmation for the current Windows binary.'
} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $root 'evidence\text-files\manifest.json') -Encoding UTF8
