# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
$out=Join-Path $root 'evidence/cleanup-icon-suite-20261024'
$plan=Get-Content -LiteralPath (Join-Path $out 'plan.json') -Raw -Encoding utf8|ConvertFrom-Json
if($plan.workspace-ne$root-or!$plan.no_follow){throw 'Unexpected cleanup workspace or traversal policy'}
if((Get-FileHash -LiteralPath (Join-Path $root 'dist/latest.json')).Hash.ToLowerInvariant()-ne$plan.latest_sha256){throw 'Release identity changed after the plan'}
if((Get-FileHash -LiteralPath (Join-Path $out 'verification-before.json')).Hash.ToLowerInvariant()-ne$plan.verification_sha256){throw 'Pre-cleanup verification changed'}
$allowed=@('artifacts/icon-integration-20261024/packages','artifacts/icon-integration-20261024/master/最终解压 中文 空格','artifacts/icon-integration-20261024/master/迁移后 新目录 中文 空格','dist/.before-icon-suite-20261024-r3','artifacts/icon-design/2026-10-24/browser-profile','artifacts/icon-design/2026-10-24/browser-profile-no-extensions','artifacts/icon-design/2026-10-24/unpacked-3404e05d5dcc','artifacts/icon-design/2026-10-24/unpacked-af77c9454bff','assets/branding/mdcard-icon/tools/node_modules','scripts/__pycache__','artifacts/icon-integration-20261024/implementation/MDcard.exe')
$tracked=@(git -C $root ls-files);if($LASTEXITCODE-ne0){throw 'Cannot inspect tracked source'}
function Checked($entry){
 if($entry.path-notin$allowed){throw "Target was not explicitly authorized: $($entry.path)"}
 $full=[IO.Path]::GetFullPath((Join-Path $root $entry.path))
 if(!$full.StartsWith($root+'\',[StringComparison]::OrdinalIgnoreCase)-or$full-ne[IO.Path]::GetFullPath($entry.absolute)){throw 'Target escaped workspace or changed identity'}
 foreach($name in $tracked){if($name-eq$entry.path-or$name.StartsWith($entry.path+'/')){throw "Tracked source under target: $name"}}
 $parent=$full
 while($parent.Length-gt$root.Length){if(([IO.File]::GetAttributes($parent)-band[IO.FileAttributes]::ReparsePoint)-ne0){throw "Linked target/ancestor: $parent"};$parent=[IO.Path]::GetDirectoryName($parent)}
 return $full
}
function MeasureNoFollow([string]$path){
 $stack=[Collections.Generic.Stack[string]]::new();$stack.Push($path);$n=[long]0;$bytes=[long]0;$links=0
 while($stack.Count){$current=$stack.Pop();$attr=[IO.File]::GetAttributes($current)
  if($attr-band[IO.FileAttributes]::ReparsePoint){$links++}
  elseif($attr-band[IO.FileAttributes]::Directory){foreach($child in [IO.Directory]::EnumerateFileSystemEntries($current)){$stack.Push($child)}}
  else{$n++;$bytes+=([IO.FileInfo]::new($current)).Length}}
 return @{files=$n;bytes=$bytes;links=$links}
}
function DeleteNoFollow([string]$path){
 $attr=[IO.File]::GetAttributes($path)
 if($attr-band[IO.FileAttributes]::ReparsePoint){if($attr-band[IO.FileAttributes]::Directory){[IO.Directory]::Delete($path,$false)}else{[IO.File]::Delete($path)};return}
 if($attr-band[IO.FileAttributes]::Directory){foreach($child in [IO.Directory]::EnumerateFileSystemEntries($path)){DeleteNoFollow $child};[IO.Directory]::Delete($path,$false)}
 else{if($attr-band[IO.FileAttributes]::ReadOnly){[IO.File]::SetAttributes($path,($attr-band(-bnot[IO.FileAttributes]::ReadOnly)))};[IO.File]::Delete($path)}
}
$processes=@(Get-CimInstance Win32_Process)
foreach($entry in $plan.targets){
 $full=Checked $entry
 foreach($process in $processes){if(($process.ExecutablePath-and$process.ExecutablePath.StartsWith($full,[StringComparison]::OrdinalIgnoreCase))-or($entry.category-eq'browser_cache'-and$process.CommandLine-and$process.CommandLine.Contains($full))){throw "Target is in use: $full"}}
 $measure=MeasureNoFollow $full
 if($measure.files-ne$entry.files-or$measure.bytes-ne$entry.bytes-or$measure.links){throw "Target changed after planning: $full"}
}
$logPath=Join-Path $out 'deletion-results.jsonl';if(Test-Path -LiteralPath $logPath){throw 'Preserve the previous execution log'}
$freeBefore=(Get-PSDrive D).Free;$start=[DateTimeOffset]::UtcNow.ToString('o');$rows=@()
foreach($entry in $plan.targets){
 $full=Checked $entry
 DeleteNoFollow $full
 if([IO.File]::Exists($full)-or[IO.Directory]::Exists($full)){throw "Target still exists: $full"}
 $row=@{path=$entry.path;status='deleted';files=$entry.files;bytes=$entry.bytes;markdown_files=$entry.markdown_files;category=$entry.category;reason=$entry.reason}
 $rows+=,$row;$row|ConvertTo-Json -Compress|Add-Content -LiteralPath $logPath -Encoding utf8
 Write-Output "Deleted $($entry.path): $($entry.files) files"
}
$result=@{complete=$true;started_utc=$start;finished_utc=[DateTimeOffset]::UtcNow.ToString('o');targets=$rows.Count;deleted_files=($rows|Measure-Object files -Sum).Sum;deleted_bytes=($rows|Measure-Object bytes -Sum).Sum;deleted_markdown_files=($rows|Measure-Object markdown_files -Sum).Sum;free_before=$freeBefore;free_after=(Get-PSDrive D).Free;plan_sha256=(Get-FileHash -LiteralPath (Join-Path $out 'plan.json')).Hash.ToLowerInvariant();results=$rows}
$result|ConvertTo-Json -Depth 12|Set-Content -LiteralPath (Join-Path $out 'result.json') -Encoding utf8
@{complete=$true;targets=$result.targets;deleted_files=$result.deleted_files;deleted_bytes=$result.deleted_bytes;deleted_markdown_files=$result.deleted_markdown_files}|ConvertTo-Json
