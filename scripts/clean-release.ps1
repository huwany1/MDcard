# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
    [string]$Version = '0.1.1',
    [switch]$DryRun
)
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path -LiteralPath (Split-Path $PSScriptRoot)).Path.TrimEnd('\')
$releaseFile = Join-Path $repoRoot "dist\MDcard-$Version-release.json"
if (!(Test-Path -LiteralPath $releaseFile)) { throw 'Verify the release and write its release index before cleanup.' }
$release = Get-Content -Raw -LiteralPath $releaseFile | ConvertFrom-Json
if (!$release.passed -or $release.version -ne $Version) { throw 'Release verification has not passed.' }
$retained = @('windows', 'linux', "MDcard-$Version-windows-x64.zip", "MDcard-$Version-linux-x64.tar.gz", "MDcard-$Version-release.json")
foreach ($name in $retained) {
    if (!(Test-Path -LiteralPath (Join-Path $repoRoot "dist\$name"))) { throw "Missing retained release: $name" }
}
foreach ($package in $release.packages) {
    $archivePath = Join-Path $repoRoot $package.archive
    if ((Get-FileHash -LiteralPath $archivePath).Hash -ne $package.archive_sha256) { throw "Archive changed: $archivePath" }
}
function Assert-WorkspacePath([string]$Path) {
    $absolute = [IO.Path]::GetFullPath($Path)
    if (!$absolute.StartsWith($repoRoot + '\', [StringComparison]::OrdinalIgnoreCase) -or
        $absolute.StartsWith($repoRoot + '\.git', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing cleanup outside generated workspace files: $absolute"
    }
    return $absolute
}
$targets = @(
    'artifacts', 'evidence', 'target', 'scripts\__pycache__',
    'renderer\runtime', 'renderer\node_modules', 'renderer\vendor',
    'tests\rich-acceptance\target', 'tests\performance-acceptance\target', 'tests\search-acceptance\target'
) | ForEach-Object { Assert-WorkspacePath (Join-Path $repoRoot $_) }
$targets += Get-ChildItem -LiteralPath (Join-Path $repoRoot 'dist') -Force |
    Where-Object { $_.Name -notin $retained } |
    ForEach-Object { Assert-WorkspacePath $_.FullName }
$targets = @($targets | Where-Object { Test-Path -LiteralPath $_ })
foreach ($target in $targets) { Write-Output "Remove: $target" }
if ($DryRun) { return }
# Cargo's mounted target directory belongs to this repository; do not touch
# Docker's shared Cargo download cache or any other container/volume.
$mounts = docker inspect mdcard-dev --format '{{json .Mounts}}' 2>$null | ConvertFrom-Json
if ($LASTEXITCODE -eq 0 -and @($mounts | Where-Object { $_.Destination -eq '/work/target' -and $_.Name -eq 'mdcard-target' }).Count -eq 1) {
    # The volume mount itself cannot be removed by cargo clean. Remove only
    # children of this explicitly verified target mount.
    $cacheCleanup = @'
from pathlib import Path
import shutil
root = Path('/work/target')
assert root.resolve() == Path('/work/target')
for entry in root.iterdir():
    if entry.is_symlink() or not entry.is_dir():
        entry.unlink()
    else:
        shutil.rmtree(entry)
print('Cleared repository target volume contents; preserved the mount and Cargo download cache.')
'@
    docker exec mdcard-dev python3 -c $cacheCleanup
    if ($LASTEXITCODE -ne 0) { throw 'Repository build-cache cleanup failed.' }
}
foreach ($target in $targets) {
    $entry = Get-Item -LiteralPath $target -Force
    if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        $entry.Delete()
        continue
    }
    if ($entry.PSIsContainer) {
        # Remove links themselves first; an artifact junction may point at
        # live renderer sources which must survive this cleanup.
        foreach ($link in Get-ChildItem -LiteralPath $target -Recurse -Force -Attributes ReparsePoint) {
            [void](Assert-WorkspacePath $link.FullName)
            $link.Delete()
        }
        Remove-Item -LiteralPath $target -Recurse -Force
    } else {
        Remove-Item -LiteralPath $target -Force
    }
}
Write-Output 'Kept latest complete packages, installed release folders, source repository, and full Git history.'
