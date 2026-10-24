# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$Evidence='evidence/release-r29-2026-10-24')
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
function Inside([string]$Path){
 $full=[IO.Path]::GetFullPath($Path)
 if(!$full.StartsWith($root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw "Path escapes workspace: $full"}
 if(Test-Path -LiteralPath $full){
  $item=Get-Item -LiteralPath $full -Force
  if($item.Attributes-band[IO.FileAttributes]::ReparsePoint){throw "Reparse target rejected: $full"}
 }
 return $full
}
$out=Inside (Join-Path $root $Evidence)
$dist=Inside (Join-Path $root 'dist')
$source=Inside (Join-Path $root 'artifacts/reading-precision/packages/precision-b-20261024-r29')
$backup=Inside (Join-Path $dist '.before-r29-promotion-20261024')
if(Test-Path -LiteralPath $backup){throw 'Preserve prior promotion; backup path already exists'}
if(Test-Path -LiteralPath (Join-Path $out 'staged.json')){throw 'Preserve prior promotion evidence'}
foreach($platform in @('windows','linux')){
 $report=Get-Content -LiteralPath (Join-Path $out "$platform-before-promotion.json") -Raw -Encoding utf8|ConvertFrom-Json
 if(!$report.passed-or$report.binary_source_commit-ne'40b5e3bb16e1658e333cf2fddf965a22d4e878ff'){throw 'Both full payload preflights must pass'}
 if(![string]::Equals($report.actual_package,(Join-Path $source $platform),[StringComparison]::OrdinalIgnoreCase)){throw 'Preflight belongs to another source'}
}
$archivePreflight=Get-Content -LiteralPath (Join-Path $out 'archive-preflight.json') -Raw -Encoding utf8|ConvertFrom-Json
if(!$archivePreflight.passed){throw 'Accepted runtime/evidence archives not verified'}
$packages=Get-Content -LiteralPath (Join-Path $source 'packages.json') -Raw -Encoding utf8|ConvertFrom-Json
Copy-Item -LiteralPath (Join-Path $dist 'latest.json') -Destination (Join-Path $out 'latest-before.json')
foreach($p in $packages){
 $archive=Inside $p.archive
 $destination=Inside (Join-Path $dist (Split-Path $archive -Leaf))
 if(Test-Path -LiteralPath $destination){throw "New release archive already exists: $destination"}
 Copy-Item -LiteralPath $archive -Destination $destination
 if((Get-FileHash -LiteralPath $destination).Hash.ToLowerInvariant()-cne$p.archive_sha256){throw 'Copied archive checksum mismatch'}
}
New-Item -ItemType Directory -Path $backup|Out-Null
$moved=@()
try{
 foreach($platform in @('windows','linux')){
  $from=Inside (Join-Path $source $platform)
  $target=Inside (Join-Path $dist $platform)
  $retired=Inside (Join-Path $backup $platform)
  Copy-Item -LiteralPath (Join-Path $target 'manifest.json') -Destination (Join-Path $out "$platform-retired-manifest.json")
  Move-Item -LiteralPath $target -Destination $retired
  try{Move-Item -LiteralPath $from -Destination $target}
  catch{Move-Item -LiteralPath $retired -Destination $target;throw}
  $moved+=,@{platform=$platform;source=$from;target=$target;retired=$retired}
 }
}catch{
 for($i=$moved.Count-1;$i-ge0;$i--){
  $entry=$moved[$i]
  Move-Item -LiteralPath (Inside $entry.target) -Destination (Inside $entry.source)
  Move-Item -LiteralPath (Inside $entry.retired) -Destination (Inside $entry.target)
 }
 throw
}
@{status='staged; latest pointer unchanged pending final-directory GUI and verification';created_utc=[DateTimeOffset]::UtcNow.ToString('o');
 rebuilt=$false;repacked=$false;product_commit='40b5e3bb16e1658e333cf2fddf965a22d4e878ff';moves=$moved;
 old_runtime_backup=$backup;input_packages=$packages}|ConvertTo-Json -Depth 20|Set-Content -LiteralPath (Join-Path $out 'staged.json') -Encoding utf8
Write-Output 'R29 complete runtimes staged at dist/windows and dist/linux; accepted archive bytes copied unchanged. Publish latest only after final-directory validation.'
