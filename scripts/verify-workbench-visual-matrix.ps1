# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot;$Exe=(Resolve-Path -LiteralPath $Exe).Path
$output=Join-Path $root 'evidence\workbench-redesign-2026-10-22\visual-final';New-Item -ItemType Directory -Force $output|Out-Null
$expected=(Get-FileHash $Exe).Hash;$cases=[Collections.Generic.List[object]]::new();$result=@{verification_id=[Guid]::NewGuid().ToString();binary_sha256=$expected;native_windows_gui=$true;passed=$false}
try{
 foreach($language in @('zh-CN','zh-TW','en','ja','ko','ru')){
  foreach($appearance in @('Light','Dark')){
   & (Join-Path $PSHOME 'pwsh.exe') -NoProfile -STA -File (Join-Path $PSScriptRoot 'capture-workbench-windows.ps1') -Exe $Exe -Stage after -RunName final -Language $language -Appearance $appearance
   if($LASTEXITCODE-ne0){throw "Native visual matrix failed for $language/$appearance"}
   $file=Join-Path $root "evidence\workbench-redesign-2026-10-22\after-final\$language-$appearance\result.json"
   $case=Get-Content -LiteralPath $file -Raw|ConvertFrom-Json
   if(!$case.source_unchanged-or$case.sha256-ne$expected-or$case.captures.Count-ne21){throw "Incomplete or mismatched visual evidence $language/$appearance"}
   $cases.Add($case);Write-Output "Verified $language/$appearance : $($case.captures.Count) native captures"
  }
 }
 $result.passed=$cases.Count-eq12
}catch{$result.error=$_.Exception.Message;throw}
finally{$result.cases=$cases.ToArray();$result.captures=(@($cases|ForEach-Object {$_.captures.Count})|Measure-Object -Sum).Sum;$result|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8}
