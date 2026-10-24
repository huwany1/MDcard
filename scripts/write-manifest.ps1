# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
if (Test-Path -LiteralPath (Join-Path $root 'evidence\folder-tree\master-review\result.json')) {
    $oldManifest = Join-Path $root 'evidence\manifest.json'
    $historical = Join-Path $root 'evidence\folder-tree\manifest-before-folder-tree.json'
    if ((Test-Path -LiteralPath $oldManifest) -and !(Test-Path -LiteralPath $historical)) {
        Copy-Item -LiteralPath $oldManifest -Destination $historical
    }
    & (Join-Path $PSScriptRoot 'write-folder-tree-manifest.ps1')
    Copy-Item -LiteralPath (Join-Path $root 'evidence\folder-tree\manifest.json') -Destination $oldManifest -Force
    return
}
if (Test-Path -LiteralPath (Join-Path $root 'evidence\window-corners\windows\result.json')) {
    $oldManifest = Join-Path $root 'evidence\manifest.json'
    $historical = Join-Path $root 'evidence\window-corners\manifest-before-corners.json'
    if ((Test-Path -LiteralPath $oldManifest) -and !(Test-Path -LiteralPath $historical)) {
        Copy-Item -LiteralPath $oldManifest -Destination $historical
    }
    & (Join-Path $PSScriptRoot 'write-window-corners-manifest.ps1')
    Copy-Item -LiteralPath (Join-Path $root 'evidence\window-corners\manifest.json') -Destination $oldManifest -Force
    return
}
if (Test-Path -LiteralPath (Join-Path $root 'evidence\text-files\master-review\result.json')) {
    # Keep the previous localization manifest intact as historical evidence.
    $oldManifest = Join-Path $root 'evidence\manifest.json'
    $historical = Join-Path $root 'evidence\localization\manifest-before-text-files.json'
    if ((Test-Path -LiteralPath $oldManifest) -and !(Test-Path -LiteralPath $historical)) {
        $previous = Get-Content -LiteralPath $oldManifest -Raw | ConvertFrom-Json
        if ($previous.feature -ne 'Relative descendant text-file search, display and editing') {
            Copy-Item -LiteralPath $oldManifest -Destination $historical
        }
    }
    & (Join-Path $PSScriptRoot 'write-text-files-manifest.ps1')
    Copy-Item -LiteralPath (Join-Path $root 'evidence\text-files\manifest.json') -Destination $oldManifest -Force
    return
}
function Record([string]$Relative) {
    $path = Join-Path $root $Relative
    $file = Get-Item -LiteralPath $path
    @{path=$Relative.Replace('\','/'); bytes=$file.Length; sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash}
}
$outputs = @('dist\windows\MDcard.exe','dist\linux\mdcard','dist\MDcard-0.1.0-windows-x64.zip','dist\MDcard-0.1.0-linux-x64.tar.gz') | ForEach-Object { Record $_ }
$uiProof = Get-Content -LiteralPath (Join-Path $root 'evidence\localization\windows\matrix.json') -Raw | ConvertFrom-Json
if ($outputs[0].sha256 -ne $uiProof.exe_sha256) { throw 'Windows executable changed after final localization verification' }
$masterProof = Get-Content -LiteralPath (Join-Path $root 'evidence\localization\master-review\result.json') -Raw | ConvertFrom-Json
if ($outputs[0].sha256 -ne $masterProof.exe_sha256) { throw 'Windows executable changed after independent localization verification' }
$undoProof = Get-Content -LiteralPath (Join-Path $root 'evidence\localization\master-review\undo-result.json') -Raw | ConvertFrom-Json
if ($outputs[0].sha256 -ne $undoProof.exe_sha256 -or !$undoProof.undo_restored_exact_original -or !$undoProof.redo_restored_exact_edit) { throw 'Undo/redo proof does not match final executable' }
$linuxHash = ((Get-Content -LiteralPath (Join-Path $root 'evidence\localization\linux\executable.sha256') -Raw).Trim() -split '\s+')[0]
if ($outputs[1].sha256 -ne $linuxHash) { throw 'Linux executable changed after final localization verification' }
$sourcePaths = @('Cargo.toml','Cargo.lock','Dockerfile','README.md','docs\ACCEPTANCE.md','src\main.rs','src\document.rs','src\preferences.rs','src\theme.rs','src\rendering.rs','src\localization.rs','scripts\verify-ui-gpu-windows.ps1','scripts\verify-ui-gpu-linux.sh','scripts\verify-localization-windows.ps1','scripts\verify-localization-linux.sh','scripts\verify-localization-packages.py','scripts\package.ps1','scripts\write-manifest.ps1')
$sourcePaths += Get-ChildItem -LiteralPath (Join-Path $root 'assets'),(Join-Path $root 'locales') -File | ForEach-Object { [IO.Path]::GetRelativePath($root,$_.FullName) }
$source = $sourcePaths | ForEach-Object { Record $_ }
$proof = Get-ChildItem -LiteralPath (Join-Path $root 'evidence') -File -Recurse | Where-Object { $_.Name -ne 'manifest.json' -and $_.Extension -ne '.exe' } | Sort-Object FullName | ForEach-Object { Record ([IO.Path]::GetRelativePath($root,$_.FullName)) }
@{
    verification_id=[Guid]::NewGuid().ToString()
    date='2026-10-19'
    timezone='Asia/Shanghai'
    rust=(Get-Content -LiteralPath (Join-Path $root 'evidence\rust-version.txt') -Raw).Trim()
    windows='Windows 11 26100 x64 / 125% DPI / GNU target'
    linux='Debian 12 x64 / glibc 2.36 / X11 Xvfb / Openbox / Mesa software / GTK portal'
    historical_windows_run=(Get-ChildItem -Path (Join-Path $root 'evidence\windows\*\result.json') -File | Where-Object {$_.Directory.Name -match '^\d{8}-\d{6}$'} | Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName.Substring($root.Length+1).Replace('\','/')
    final_ui_windows='evidence/localization/windows/matrix.json'
    final_ui_linux='evidence/localization/linux/matrix.json'
    final_independent_review='evidence/localization/master-review/result.json'
    historical_ui_windows='evidence/ui-gpu/windows/result.json'
    historical_ui_linux='evidence/ui-gpu/linux/result.json'
    historical_evidence_note='Older ui-gpu, controls and dialog records describe earlier binaries; final six-language screenshots, reviews and executable hashes are under evidence/localization.'
    historical_linux_dialogs='evidence/linux/dialog-result.json'
    final_undo_redo='evidence/localization/master-review/undo-result.json'
    final_package_check='evidence/localization/package-check.json'
    outputs=$outputs
    source=$source
    evidence=$proof
} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $root 'evidence\manifest.json') -Encoding UTF8
