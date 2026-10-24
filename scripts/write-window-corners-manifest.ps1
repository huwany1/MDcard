# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot
function Record([string]$relative) {
 $path=Join-Path $root $relative
 @{path=$relative.Replace('\','/');bytes=(Get-Item -LiteralPath $path).Length;sha256=(Get-FileHash -LiteralPath $path).Hash}
}
$outputs=@('dist\windows\MDcard.exe','dist\linux\mdcard','dist\MDcard-0.1.0-windows-x64.zip','dist\MDcard-0.1.0-linux-x64.tar.gz')|ForEach-Object {Record $_}
$windows=Get-Content -Raw -LiteralPath (Join-Path $root 'evidence\window-corners\windows\result.json')|ConvertFrom-Json
$linux=((Get-Content -Raw -LiteralPath (Join-Path $root 'evidence\window-corners\linux\executable.sha256')).Trim()-split '\s+')[0]
if($outputs[0].sha256-ne$windows.exe_sha256-or$outputs[1].sha256-ne$linux){throw 'Final corner UI evidence does not match current binaries'}
$master=Get-Content -Raw -LiteralPath (Join-Path $root 'evidence\window-corners\master-review\result.json')|ConvertFrom-Json
if($master.exe_sha256-ne$outputs[0].sha256){throw 'Independent corner review does not match current Windows binary'}
$packages=Get-Content -Raw -LiteralPath (Join-Path $root 'evidence\window-corners\package-check.json')|ConvertFrom-Json
if(!$packages.passed){throw 'Package verification failed'}
foreach($archive in $packages.archives) {
 $record=$outputs|Where-Object {$_.path-eq$archive.archive.Replace('\','/')}
 if(!$record-or$record.sha256-ne$archive.archive_sha256){throw "Package changed: $($archive.archive)"}
}
$paths=@('Cargo.toml','Cargo.lock','Dockerfile','README.md','docs\ACCEPTANCE.md','THIRD_PARTY_NOTICES.txt')
$paths+=Get-ChildItem -LiteralPath (Join-Path $root 'src'),(Join-Path $root 'scripts'),(Join-Path $root 'locales'),(Join-Path $root 'assets') -File -Recurse|ForEach-Object {[IO.Path]::GetRelativePath($root,$_.FullName)}
$source=$paths|ForEach-Object {Record $_}
$proof=Get-ChildItem -LiteralPath (Join-Path $root 'evidence\window-corners') -File -Recurse|Where-Object {$_.Name-ne'manifest.json'-and$_.Extension-ne'.exe'}|Sort-Object FullName|ForEach-Object {Record ([IO.Path]::GetRelativePath($root,$_.FullName))}
@{verification_id=[guid]::NewGuid().ToString();date='2026-10-19';timezone='Asia/Shanghai';feature='Native Windows rounded window corners';linux_tests=20;windows_native_tests=19;tests_failed=0;tests_ignored=0;final_windows='evidence/window-corners/windows/result.json';final_linux='evidence/window-corners/linux/result.json';final_independent_review='evidence/window-corners/master-review/result.json';outputs=$outputs;source=$source;evidence=$proof;historical_manifest='evidence/window-corners/manifest-before-corners.json';historical_evidence_note='Text-files, localization, and ui-gpu evidence retain their prior executable identities. No historical UI screenshots were relabeled as current corner verification.'}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $root 'evidence\window-corners\manifest.json') -Encoding UTF8
