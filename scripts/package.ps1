# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
foreach ($platform in @('windows','linux')) {
    $folder = Join-Path $root "dist\$platform"
    if (!(Test-Path -LiteralPath $folder)) { throw "Missing build output: $folder" }
    Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination $folder -Force
    Copy-Item -LiteralPath (Join-Path $root 'THIRD_PARTY_NOTICES.txt') -Destination $folder -Force
    Copy-Item -LiteralPath (Join-Path $root 'assets\OFL.txt') -Destination (Join-Path $folder 'NotoSansSC-OFL.txt') -Force
    Copy-Item -LiteralPath (Join-Path $root 'assets\NotoEmoji-OFL.txt') -Destination $folder -Force
    foreach ($font in @('JP','KR','TC')) {
        Copy-Item -LiteralPath (Join-Path $root "assets\NotoSans$font-OFL.txt") -Destination $folder -Force
    }
    Copy-Item -LiteralPath (Join-Path $root 'assets\font-sources.json') -Destination $folder -Force
    Copy-Item -LiteralPath (Join-Path $root 'locales') -Destination $folder -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $root 'examples') -Destination $folder -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $root 'README.md') -Destination $folder -Force
    Copy-Item -LiteralPath (Join-Path $root 'docs') -Destination $folder -Recurse -Force
    $preview = Join-Path $folder 'evidence\linux'
    New-Item -ItemType Directory -Force $preview | Out-Null
    Copy-Item -LiteralPath (Join-Path $root 'evidence\linux\images.png') -Destination $preview -Force
}
$windowsFiles = Get-ChildItem -LiteralPath (Join-Path $root 'dist\windows') | Where-Object { $_.Extension -ne '.exe' -or $_.Name -eq 'MDcard.exe' }
Compress-Archive -LiteralPath $windowsFiles.FullName -DestinationPath (Join-Path $root 'dist\MDcard-0.1.0-windows-x64.zip') -Force
Write-Output 'Windows package ready. For normalized Linux archive permissions, run scripts/package-linux.sh inside the build container.'
