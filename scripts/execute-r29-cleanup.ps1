# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$Plan='evidence/release-r29-2026-10-24/cleanup-plan.json')
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
$out=Join-Path $root 'evidence/release-r29-2026-10-24'
$planPath=[IO.Path]::GetFullPath((Join-Path $root $Plan))
$data=Get-Content -LiteralPath $planPath -Raw -Encoding utf8|ConvertFrom-Json
if($data.workspace-ne$root-or!$data.no_follow){throw 'Wrong workspace or unsafe plan'}
$promotion=Join-Path $out 'promotion.json'
if((Get-FileHash -LiteralPath $promotion).Hash.ToLowerInvariant()-cne$data.promotion_sha256){throw 'Promotion evidence changed'}
$latest=Get-Content -LiteralPath (Join-Path $root 'dist/latest.json') -Raw -Encoding utf8|ConvertFrom-Json
if(@($latest).Count-ne2-or@($latest|Where-Object {$_.release_channel-ne'stable'-or$_.build_id-ne'precision-b-20261024-r29'}).Count){throw 'R29 is not the current stable release'}
$active=@(Get-CimInstance Win32_Process|Where-Object {$_.Name-match'^(MDcard|cargo|rustc|rustdoc).exe$'-and$_.ExecutablePath-and$_.ExecutablePath.StartsWith($root+'\',[StringComparison]::OrdinalIgnoreCase)})
if($active.Count){throw 'A workspace application/compiler is still running'}
$tracked=@(git -C $root ls-files)
if($LASTEXITCODE-ne0){throw 'Cannot inspect tracked source'}
function Checked([string]$Path){
 $full=[IO.Path]::GetFullPath($Path)
 if(!$full.StartsWith($root+'\',[StringComparison]::OrdinalIgnoreCase)){throw "Outside workspace: $full"}
 $relative=$full.Substring($root.Length+1).Replace('\','/')
 if($relative-notmatch'^(target-[^/]+(?:/|$)|tests/[^/]+/target(?:/|$)|scripts/__pycache__(?:/|$)|artifacts/|evidence/reading-(precision|state)/|dist/(.before-r29-promotion-20261024|reading-state-final-20261023|MDcard-))'){throw "Outside authorized cleanup categories: $relative"}
 if($relative-match'^dist/(windows|linux)(/|$)'-or$relative-eq'dist/latest.json'-or$relative-like'artifacts/reading-precision/delivery/*'){throw 'Protected release/evidence path'}
 foreach($name in $tracked){if($name-eq$relative-or$name.StartsWith($relative+'/',[StringComparison]::Ordinal)){throw "Tracked source below delete target: $relative"}}
 # Reject linked ancestors before any operation. Reparse leaves are removed as leaves.
 $parent=[IO.Path]::GetDirectoryName($full)
 while($parent.Length-gt$root.Length){
  if(([IO.File]::GetAttributes($parent)-band[IO.FileAttributes]::ReparsePoint)-ne0){throw "Linked ancestor: $parent"}
  $parent=[IO.Path]::GetDirectoryName($parent)
 }
 return $full
}
function DeleteTreeNoFollow([string]$Path){
 $attributes=[IO.File]::GetAttributes($Path)
 if($attributes-band[IO.FileAttributes]::ReparsePoint){
  if($attributes-band[IO.FileAttributes]::Directory){[IO.Directory]::Delete($Path,$false)}else{[IO.File]::Delete($Path)}
  return
 }
 if($attributes-band[IO.FileAttributes]::Directory){
  foreach($child in [IO.Directory]::EnumerateFileSystemEntries($Path)){DeleteTreeNoFollow $child}
  [IO.Directory]::Delete($Path,$false)
 }else{
  if($attributes-band[IO.FileAttributes]::ReadOnly){[IO.File]::SetAttributes($Path,($attributes-band(-bnot[IO.FileAttributes]::ReadOnly)))}
  [IO.File]::Delete($Path)
 }
}
if(Test-Path -LiteralPath (Join-Path $out 'deletion-results.jsonl')){throw 'Keep prior deletion evidence'}
$freeBefore=(Get-PSDrive D).Free
$log=[IO.StreamWriter]::new((Join-Path $out 'deletion-results.jsonl'),$false,[Text.UTF8Encoding]::new($false))
$removed=$files=$bytes=$links=$skipped=0
$started=[DateTimeOffset]::UtcNow.ToString('o')
try{
 foreach($entry in $data.targets){
  $absolute=Checked $entry.absolute
  if($absolute-ne[IO.Path]::GetFullPath((Join-Path $root $entry.path))){throw 'Plan path mismatch'}
  $attributes=[IO.File]::GetAttributes($absolute)
  $changed=$false
  if($entry.kind-eq'file'-and$entry.mtime_ns){
   $expectedTicks=[long][Numerics.BigInteger]::Divide([bigint]$entry.mtime_ns,[bigint]100)+[long]621355968000000000
   $info=[IO.FileInfo]::new($absolute)
   $changed=$info.Length-ne[long]$entry.bytes-or$info.LastWriteTimeUtc.Ticks-ne$expectedTicks
  }
  if($changed){$skipped++;$status='retained: changed after plan'}else{
   # The checked absolute tree is removed using .NET in this PowerShell process;
   # reparse entries are always deleted as leaves and never enumerated.
   DeleteTreeNoFollow $absolute
   if([IO.File]::Exists($absolute)-or[IO.Directory]::Exists($absolute)){throw "Still present: $absolute"}
   $removed++;$files+=[long]$entry.files;$bytes+=[long]$entry.bytes;$links+=[long]$entry.links;$status='deleted'
  }
  $log.WriteLine((@{path=$entry.path;status=$status;files=$entry.files;bytes=$entry.bytes;reason=$entry.reason}|ConvertTo-Json -Compress))
  if($entry.kind-eq'tree'-or($removed%2000-eq0)){$log.Flush();Write-Output "Processed $removed targets; deleted $files files / $bytes logical bytes"}
 }
}finally{$log.Flush();$log.Dispose()}
@{complete=$true;started_utc=$started;finished_utc=[DateTimeOffset]::UtcNow.ToString('o');deleted_targets=$removed;deleted_files=$files;deleted_bytes=$bytes;
 deleted_link_entries=$links;retained_changed=$skipped;free_before=$freeBefore;free_after=(Get-PSDrive D).Free;plan_sha256=(Get-FileHash -LiteralPath $planPath).Hash.ToLowerInvariant();
 boundary='Logical bytes are not physical reclaimed space. Current source/docs/Git/runtime/evidence archive protected; all link entries removed without traversing targets.'}|
 ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $out 'cleanup-result.json') -Encoding utf8
Write-Output 'Explicit workspace cleanup completed.'
