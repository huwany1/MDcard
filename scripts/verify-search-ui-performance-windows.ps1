# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$Exe="$PSScriptRoot\..\artifacts\global-search\MDcard.exe",[string]$OutputDir='', [int]$Rounds=100, [int]$CpuWorkers=0)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-search-master-windows.ps1" -Exe $Exe -LoadFunctionsOnly
$reviewControl=${function:Control}
function Control([string]$Name){
 $handle=$script:app.MainWindowHandle
 [void][MdcardUi]::SetWindowPos($handle,[IntPtr]::new(-1),0,0,0,0,0x43)
 [void][MdcardUi]::SetForegroundWindow($handle)
 if([MdcardUi]::GetForegroundWindow()-ne$handle){throw 'Performance test window must own foreground before input'}
 & $reviewControl $Name
}
$output=Join-Path $root 'evidence\search\master-review\windows-ui-performance'
if($OutputDir){$output=[IO.Path]::GetFullPath($OutputDir)}
$corpus=Join-Path $root 'artifacts\search\master-review\performance-workspace'
if($OutputDir){$corpus=Join-Path $output 'workspace'}
New-Item -ItemType Directory -Force $output,$corpus|Out-Null
Remove-Item -LiteralPath (Join-Path $corpus 'created.md') -ErrorAction SilentlyContinue
$settings=Join-Path $output 'settings.json';$report=Join-Path $output 'state.json'
$a=Join-Path $corpus 'note-0000.md'
$utf8=[Text.UTF8Encoding]::new($false);$bytes=0
for($i=0;$i-lt1600;$i++){
 $body="# Document $i`nshared-search-term`nneedle-$($i.ToString('0000'))`n"+$(if($i%100-eq0){"中文关键字`n"}else{''})+('An ordinary paragraph keeps the corpus realistic.'+"`n")*170
 $raw=$utf8.GetBytes($body);$bytes+=$raw.Length
 [IO.File]::WriteAllBytes((Join-Path $corpus ('note-'+$i.ToString('0000')+'.md')),$raw)
}
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$priorClipboard=$null;try{$priorClipboard=[Windows.Forms.Clipboard]::GetDataObject()}catch{}
$records=[Collections.Generic.List[object]]::new();$verification=[Guid]::NewGuid().ToString()
$clock=[Diagnostics.Stopwatch]::StartNew()
$load=@()
Remove-Item -LiteralPath (Join-Path $output 'performance.json') -ErrorAction SilentlyContinue
try{
 StartReview;Keys '^+f';$initial=Await {param($s)$s.index.files-eq1600-and!$s.query_pending} '1600-document index ready'
 Control 'regex';Fill 'find-query' '(?m)^needle-(?:0731|0800)$'
 Await {param($s)$s.results-eq2-and!$s.query_pending-and!$s.error} 'Regex control settled'|Out-Null
 for($n=0;$n-lt$CpuWorkers;$n++){
  $psi=[Diagnostics.ProcessStartInfo]::new('python');$psi.UseShellExecute=$false;$psi.CreateNoWindow=$true
  $psi.ArgumentList.Add('-c');$psi.ArgumentList.Add("import hashlib`nb=b'x'*1048576`nwhile True: hashlib.sha256(b).digest()")
  $load+=,[Diagnostics.Process]::Start($psi)
 }
 $queries=@('needle-0731','missing-constant','shared-search-term','中文关键字','(?m)^needle-(?:0731|0800)$')
 for($i=0;$i-lt$Rounds;$i++){
  $query=$queries[$i%5]
  Fill 'find-query' $query
  $s=Await {param($s)$s.query-eq$query-and!$s.query_pending-and!$s.error} 'Timed query complete'
  $expected=@(1,0,1600,16,2)[$i%5]
  if($s.results-ne$expected){throw "Query $query expected $expected matches, got $($s.results)"}
  $records.Add([pscustomobject]@{query=$query;visible_ms=$s.visible_ms;worker_ms=$s.worker_ms;engine_ms=$s.engine_ms;matches=$s.results;queue_ms=$s.queue_ms;processing_ms=$s.processing_ms;poll_ms=$s.poll_ms})
  if($i-eq30){[IO.File]::WriteAllText((Join-Path $corpus 'note-0731.md'),"# Changed`nneedle-0731`nshared-search-term`n",$utf8)}
  if($i-eq60){[IO.File]::WriteAllText((Join-Path $corpus 'created.md'),"# Added`nneedle-new`n",$utf8)}
  if($i-eq80){Remove-Item -LiteralPath (Join-Path $corpus 'created.md')}
  Start-Sleep -Milliseconds 60
 }
 Shot 'native-1600-documents'
 function Stats([string]$key){$v=@($records|ForEach-Object {$_.$key}|Sort-Object);return @{p50_ms=$v[[int][Math]::Floor($v.Count*.5)];p95_ms=$v[[int][Math]::Floor($v.Count*.95)];p99_ms=$v[[int][Math]::Floor($v.Count*.99)];max_ms=$v[-1]}}
 $result=@{verification_id=$verification;date=[DateTimeOffset]::UtcNow.ToString('o');timezone='UTC';exe_sha256=(Get-FileHash -LiteralPath $Exe).Hash;documents=1600;bytes=$bytes;rounds=$Rounds;cpu_load_workers=$CpuWorkers;load_processes_alive=(@($load|Where-Object HasExited).Count-eq0);cpu_load='SHA256 1MiB continuous per process';duration_seconds=$clock.Elapsed.TotalSeconds;visible=(Stats 'visible_ms');worker=(Stats 'worker_ms');engine=(Stats 'engine_ms');debounce_ms=0;incremental_mutation_add_remove=$true;records=$records.ToArray()}
 $result.initial_index=$initial.index;$result.final_index=(State).index
 foreach($name in @('queue_ms','processing_ms','poll_ms')){if(@($records|Where-Object {$null-ne$_.$name}).Count-eq$Rounds){$result[$name]=Stats $name}}
 if($null-ne$result.final_index.full_verifications-and$result.final_index.full_verifications-le$result.initial_index.full_verifications){throw 'Periodic full verification was not observed'}
 $result|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $output 'performance.json') -Encoding utf8
 $result|Select-Object -ExcludeProperty records|ConvertTo-Json -Depth 5
 if($result.visible.p95_ms-ge50){throw "Visible p95 exceeds 50ms: $($result.visible.p95_ms)"}
 if($result.worker.max_ms-ge50){throw "Worker max exceeds 50ms: $($result.worker.max_ms)"}
 Keys '{ESC}';StopReview
}finally{
 if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()}
 if($null-ne$priorClipboard){[Windows.Forms.Clipboard]::SetDataObject($priorClipboard,$true)}
 foreach($p in $load){if(!$p.HasExited){$p.Kill();$p.WaitForExit()};$p.Dispose()}
}
