# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
 [string]$Exe="$PSScriptRoot\..\artifacts\recovery-2026-10-26\final-r4\mdcard.exe",
 [ValidateSet('Core','Drafts','Performance','Large')][string]$Suite='Core',
 [string]$FaultExe="$PSScriptRoot\..\artifacts\recovery-2026-10-26\final-r4\mdcard-faults.exe",
 [string]$OutputDirectory=''
)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
. "$PSScriptRoot\recovery-json.ps1"
if(!$OutputDirectory){$OutputDirectory=Join-Path $root ('evidence\recovery\2026-10-26\windows-'+$Suite.ToLower()+'-'+(Get-Date -Format 'HHmmss'))}
$output=[IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force $output|Out-Null
$settings=Join-Path $output 'settings.json'
$state=Join-Path $output 'state'
$variables=@('MDCARD_UI_REPORT','MDCARD_SEARCH_REPORT','MDCARD_READING_STATE_PATH','MDCARD_RECOVERY_DIR','MDCARD_RICH_REPORT','MDCARD_PERFORMANCE_REPORT','MDCARD_STRUCTURE_REPORT','MDCARD_ACCEPTANCE_TABLE_PATCH_FAILURE','MDCARD_RENDERER_DIR')
$previousEnv=@{};foreach($name in $variables){$previousEnv[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
$env:MDCARD_UI_REPORT=Join-Path $output 'ui.json'
$env:MDCARD_SEARCH_REPORT=Join-Path $output 'search.json'
$env:MDCARD_READING_STATE_PATH=Join-Path $output 'reading.json'
$env:MDCARD_RECOVERY_DIR=$state
$env:MDCARD_RICH_REPORT=Join-Path $output 'rich.json'
$env:MDCARD_PERFORMANCE_REPORT=Join-Path $output 'performance.json'
$env:MDCARD_STRUCTURE_REPORT=Join-Path $output 'structure.json'
$env:MDCARD_RENDERER_DIR=Join-Path $root 'renderer'
$verificationId=[Guid]::NewGuid().ToString()
$utf8=[Text.UTF8Encoding]::new($false)
$steps=[Collections.Generic.List[object]]::new()
$processes=[Collections.Generic.List[Diagnostics.Process]]::new()
Add-Type @'
public static class MdcardRecoveryHash {
 public static string Of(string text) {unchecked {ulong h=0xcbf29ce484222325UL;foreach(byte b in System.Text.Encoding.UTF8.GetBytes(text)){h=(h^b)*0x100000001b3UL;}return h.ToString("x16");}}
}
'@
function Assert([bool]$Condition,[string]$Message){if(!$Condition){throw $Message}}
function Click([float]$X,[float]$Y){
 $r=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0
 [void][MdcardUi]::SetCursorPos($r.Left+[int]($X*$scale),$r.Top+[int]($Y*$scale));Start-Sleep -Milliseconds 80
 [MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 80
 [MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 350
}
function Report {ReadJsonStable $env:MDCARD_UI_REPORT @('controls','recovery','revision','autosave')}
function WaitUntil([scriptblock]$Condition,[string]$Message,[int]$Seconds=20){$end=[DateTime]::UtcNow.AddSeconds($Seconds);do{if(&$Condition){return};Start-Sleep -Milliseconds 100}while([DateTime]::UtcNow-lt$end);throw $Message}
function Ctl([string]$Name){(Report).controls|Where-Object name -EQ $Name|Select-Object -First 1}
function ClickCtl([string]$Name){
 WaitUntil { $script:controlCandidate=Ctl $Name;$null-ne$script:controlCandidate-and$null-ne$script:controlCandidate.clip } "Visible control missing: $Name"
 $c=$script:controlCandidate;$left=[Math]::Max($c.rect[0],$c.clip[0]);$top=[Math]::Max($c.rect[1],$c.clip[1]);$right=[Math]::Min($c.rect[2],$c.clip[2]);$bottom=[Math]::Min($c.rect[3],$c.clip[3])
 Assert ($right-gt$left-and$bottom-gt$top) "Control clipped: $Name"
 if($Name-eq'main-source-editor'){
  for($attempt=0;$attempt-lt4;$attempt++){Click ([Math]::Min($left+80,$right-4)) ([Math]::Min($top+15,$bottom-4));if((Ctl $Name).focus){return};Start-Sleep -Milliseconds 350}
  Shot 'debug-focus';throw 'Real source editor did not receive keyboard focus'
 }else{Click (($left+$right)/2) (($top+$bottom)/2)}
}
function Paste([string]$Text){Set-Clipboard -Value $Text;Keys '^v'}
function ForceStop {if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()};Start-Sleep -Milliseconds 200}
function StartApp([string]$Name,[string]$Document=''){Launch $Name '' $Document;$processes.Add($script:app)}
function Checkpoint([bool]$Secondary=$false,[int]$After=0){
 WaitUntil { $r=Report;$s=if($Secondary){$r.recovery.secondary}else{$r.recovery.main};$s.committed_at-gt0-and!$s.working-and$s.current_version-and$s.schedule_count-gt$After-and$null-eq$s.error } 'Latest recovery checkpoint did not commit' 30
 if($Secondary){return(Report).recovery.secondary}else{return(Report).recovery.main}
}
function ReadCopy([string]$Key){
 $copies=@();foreach($name in @('0.json','1.json')){$p=Join-Path $state "records\$Key\$name";if(Test-Path -LiteralPath $p){try{$envelope=Get-Content -LiteralPath $p -Raw|ConvertFrom-Json;$copies+=($envelope.payload|ConvertFrom-Json)}catch{}}}
 return($copies|Sort-Object sequence -Descending|Select-Object -First 1)
}
function RecoverKey([string]$Key){
 $snapshot=Report;$items=@($snapshot.recovery.items);$index=-1;for($i=0;$i-lt$items.Count;$i++){if($items[$i].key-eq$Key){$index=$i;break}}
 @{expected_key=$Key;items=@($items|ForEach-Object {$_.key});active_key=$snapshot.recovery.main.key;script_secondary_key=(Get-Variable -Name s -Scope Script -ErrorAction SilentlyContinue).Value.key;time=[DateTimeOffset]::Now.ToString('o')}|ConvertTo-Json -Compress|Add-Content -LiteralPath (Join-Path $output 'recovery-identity-trace.jsonl') -Encoding utf8
 Assert ($index-ge0) "Expected recovery identity missing: $Key; visible=$($items.key-join',')"
 if(!(Report).recovery.open){ClickCtl 'recovery-open'}
 ClickCtl "recovery-restore-$index"
 WaitUntil {(Report).recovery.main.key-eq$Key-and!(Report).recovery.operation} 'Recovery UI did not install selected copy'
 Assert ((Report).dirty-and$null-eq(Report).document_path) 'Recovered copy must be dirty and have no original path'
}
function Body {ClickCtl 'Write';ClickCtl 'main-source-editor';Keys '^a';[Windows.Forms.Clipboard]::Clear();Keys '^c';return [string](Get-Clipboard -Raw)}
function AssertBody([string]$Expected,[string]$Message){
 $visible=Body;AssertSourceDigest $Expected $Message
 Assert ($visible.Replace("`r`n","`n")-ceq$Expected.Replace("`r`n","`n")) "$Message (visible clipboard)"
}
function AssertSourceDigest([string]$Expected,[string]$Message){$r=Report;Assert ($r.recovery_origin-and$r.document_text_bytes-eq$utf8.GetByteCount($Expected)-and$r.document_digest-ceq[MdcardRecoveryHash]::Of($Expected)) "$Message (exact UTF-8 length and digest)"}
function RichCtl([string]$Name){(ReadJsonStable $env:MDCARD_RICH_REPORT @('controls')).controls|Where-Object name -EQ $Name|Select-Object -First 1}
function ClickRich([string]$Name){WaitUntil {$null-ne(RichCtl $Name)} "Rich control missing: $Name";$c=RichCtl $Name;Click (($c.rect[0]+$c.rect[2])/2) (($c.rect[1]+$c.rect[3])/2)}
function MoveFloatingRight {
 $c=RichCtl 'floating-window';$fromX=($c.rect[0]+$c.rect[2])/2;$fromY=$c.rect[1]+12;$toX=920;$toY=180
 $r=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.;[void][MdcardUi]::SetCursorPos($r.Left+[int]($fromX*$scale),$r.Top+[int]($fromY*$scale));Start-Sleep -Milliseconds 80;[MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
 for($i=1;$i-le12;$i++){[void][MdcardUi]::SetCursorPos($r.Left+[int](($fromX+($toX-$fromX)*$i/12)*$scale),$r.Top+[int](($fromY+($toY-$fromY)*$i/12)*$scale));Start-Sleep -Milliseconds 20};[MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 300
}
function DiscardMain {Keys '^n';WaitUntil {(Report).autosave.pending_navigation} 'Unsaved dialog did not appear';ClickCtl 'unsaved-discard';WaitUntil {!(Report).autosave.pending_navigation-and!(Report).dirty} 'Explicit discard did not finish'}
function SaveAs([string]$Path){Keys '^+s';Start-Sleep -Milliseconds 400;Keys '%n';Paste $Path;Keys '{ENTER}';WaitUntil {Test-Path -LiteralPath $Path} 'Save As did not create selected path';WaitUntil {!(Report).autosave.main.saving} 'Save did not finish'}
function Step([string]$Name,[object]$Details){$steps.Add(@{name=$Name;details=$Details;time=[DateTimeOffset]::Now.ToString('o')});Shot $Name}
$previousClipboard=[Windows.Forms.Clipboard]::GetDataObject()
try {
 @{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
 if($Suite-eq'Core'){
  $main=Join-Path $output 'main.md';$second=Join-Path $output 'secondary.md'
  [IO.File]::WriteAllText($main,"# Main original`r`n`r`n中文 🙂 original.`r`n",[Text.UTF8Encoding]::new($true))
  [IO.File]::WriteAllText($second,"# Secondary original`n`nSecond original.`n",$utf8)
  $mainHash=(Get-FileHash $main).Hash;$secondHash=(Get-FileHash $second).Hash
  StartApp '01-main' $main
  Assert ((Report).recovery.enabled-and!(Report).autosave.enabled) 'Missing settings must enable recovery and leave original autosave off'
  ClickCtl 'Write';ClickCtl 'main-source-editor';Keys '^{END}';Paste "`r`nMain unsaved recovery 中文 α🙂."
  $focus=(Report).focused;$selection=(Report).selection|ConvertTo-Json -Compress
  $m=Checkpoint;$mainRecoveryKey=[string]$m.key
  Assert ((Report).dirty-and(Get-FileHash $main).Hash-eq$mainHash) 'Recovery wrote the original or cleared dirty'
  Assert ((Report).focused-eq$focus-and((Report).selection|ConvertTo-Json -Compress)-eq$selection) 'Checkpoint changed focus or selection'
  $mainText=(ReadCopy $m.key).text
  Step '01-main-checkpoint' $m
  Keys '^p';Paste 'secondary.md';Start-Sleep -Milliseconds 900;Keys '+{ENTER}';Start-Sleep -Milliseconds 900
  Keys '^{END}';Paste "`r`nSecondary unsaved 한글.`r`n"
  $s=Checkpoint $true;$secondaryRecoveryKey=[string]$s.key;$secondText=(ReadCopy $s.key).text
  Assert ($m.key-ne$s.key-and(Get-FileHash $second).Hash-eq$secondHash) 'Secondary identity or original protection failed'
  Step '02-main-secondary-checkpoint' @{main=$m;secondary=$s}
  [void]$script:app.CloseMainWindow();Start-Sleep -Milliseconds 500
  Assert (!$script:app.HasExited) 'Native X bypassed unsaved protection';Keys '{ESC}'
  Assert ((Report).recovery.main.key-eq$m.key) 'Cancel close removed recovery protection'
  Step '03-cancel-close' @{}
  ForceStop
  [IO.File]::WriteAllText($main,'EXTERNAL VERSION',$utf8);Remove-Item -LiteralPath $second
  $externalHash=(Get-FileHash $main).Hash
  StartApp '04-restart-discovery'
  WaitUntil {(Report).recovery.pending-eq2} 'Restart did not discover both main and secondary'
  Step '04-recovery-list' (Report).recovery
  ClickCtl 'recovery-later';Assert ((Report).recovery.pending-eq2) 'Later removed pending manuscripts'
  ClickCtl 'recovery-open'
  RecoverKey $m.key
  AssertBody $mainText 'Visible recovered main text differs from committed checkpoint'
  Assert ((Get-FileHash $main).Hash-eq$externalHash) 'Recovery changed external original'
  Step '05-restored-main-unsaved' (Report).recovery
  $before=(Report).recovery.main.schedule_count
  ClickCtl 'main-source-editor';Keys '^{END}';Paste "`r`nAfter recovery second crash."
  $r=Checkpoint $false $before;$secondCrashText=(ReadCopy $r.key).text;Assert ($secondCrashText.Contains('After recovery second crash.')) 'Second crash edit was not in checkpoint';ForceStop
  # An explicit original-autosave preference must not bind a recovered copy to its old path.
  $pref=Get-Content -LiteralPath $settings -Raw|ConvertFrom-Json;$pref|Add-Member autosave $true -Force;$pref|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
  StartApp '06-second-restart';RecoverKey $m.key
  AssertBody $secondCrashText 'Recovery after second forced termination lost text'
  Start-Sleep -Milliseconds 2800
  Assert ((Get-FileHash $main).Hash-eq$externalHash-and$null-eq(Report).document_path) 'Recovered original-autosave wrote old original'
  Step '06-second-crash-restored' @{}
  $saved=Join-Path $output 'explicit-recovered-save.md';SaveAs $saved
  Assert ([IO.File]::ReadAllText($saved)-eq$secondCrashText) 'Explicit recovered save changed text'
  Assert ((Get-FileHash $main).Hash-eq$externalHash) 'Save As changed old original'
  WaitUntil {$null-eq(Report).recovery.main.key} 'Successful exact save did not resolve recovery'
  Keys '^n';ClickCtl 'recovery-open';RecoverKey $secondaryRecoveryKey
  AssertBody $secondText 'Deleted secondary original prevented body recovery';Assert (!(Test-Path -LiteralPath $second)) 'Deleted original was recreated'
  Step '07-deleted-secondary-restored' @{}
  DiscardMain;ForceStop;StartApp '08-discarded-restart'
  Assert ((Report).recovery.pending-eq0) 'Explicitly discarded or saved manuscripts reappeared'
  Keys '^n';ClickCtl 'Write';ClickCtl 'main-source-editor';Paste 'Untitled 中文 checkpoint to recover.';$u=Checkpoint;Step '08-untitled-checkpoint' $u;ForceStop
  StartApp '09-untitled-restart';RecoverKey $u.key;AssertBody 'Untitled 中文 checkpoint to recover.' 'Untitled UI recovery lost body'
  $before=(Report).recovery.main.schedule_count
  ClickCtl 'main-source-editor';Keys '^a';Keys '{DELETE}';$empty=Checkpoint $false $before
  Assert ((ReadCopy $empty.key).text-eq''-and(Report).dirty) 'Deleting entire untitled body lost empty valid checkpoint'
  ForceStop;StartApp '10-empty-restart';RecoverKey $u.key;AssertBody '' 'Empty manuscript did not recover'
  DiscardMain
  ClickCtl 'main-source-editor';Paste 'Delete-confirmation fixture.';$d=Checkpoint;ForceStop;StartApp '11-delete-restart'
  ClickCtl 'recovery-delete-0';ClickCtl 'recovery-delete-cancel';Assert ((Report).recovery.pending-eq1) 'Cancel delete removed record'
  ClickCtl 'recovery-delete-0';ClickCtl 'recovery-delete-confirm';WaitUntil {(Report).recovery.pending-eq0} 'Confirmed delete did not remove record'
  Step '11-delete-confirmation' @{}
  ForceStop;StartApp '12-clean-restart';Assert ((Report).recovery.pending-eq0-and!(Report).recovery.open) 'Empty recovery prompt appeared'
  Step '12-clean-no-prompt' @{}
  Keys '^n';ClickCtl 'Write';ClickCtl 'main-source-editor';Paste 'Protection switch retained checkpoint.';$protected=Checkpoint
  Keys '^+p';ClickCtl 'Crash recovery';Keys '{ESC}'
  Assert (!(Get-Content -LiteralPath $settings -Raw|ConvertFrom-Json).recovery) 'Recovery disabled choice was not persisted'
  ClickCtl 'main-source-editor';Keys '^{END}';Paste ' intentionally not checkpointed while disabled';Start-Sleep -Milliseconds 2800
  Assert ((ReadCopy $protected.key).text-ceq'Protection switch retained checkpoint.') 'Disabled recovery wrote a new checkpoint'
  ForceStop;StartApp '13-disabled-restart';Assert (!(Report).recovery.enabled-and(Report).recovery.pending-eq1) 'Disabled choice or retained pending copy did not survive restart'
  RecoverKey $protected.key;AssertBody 'Protection switch retained checkpoint.' 'Disabled recovery could not recover existing copy'
  Keys '^+p';ClickCtl 'Crash recovery';Keys '{ESC}';Assert ((Get-Content -LiteralPath $settings -Raw|ConvertFrom-Json).recovery) 'Recovery enabled choice was not persisted'
  ClickCtl 'main-source-editor';Keys '^{END}';Paste ' enabled again';$protected=Checkpoint
  Step '13-switch-persisted-and-pending-kept' $protected
  $record=Join-Path $state "records\$($protected.key)";$current=[int](Get-Content -LiteralPath (Join-Path $record 'current') -Raw);$blocked=Join-Path $record "$((1-$current)).json"
  Assert ([IO.Path]::GetFullPath($blocked).StartsWith([IO.Path]::GetFullPath($state)+[IO.Path]::DirectorySeparatorChar)) 'Fault path escaped isolated state'
  if(Test-Path -LiteralPath $blocked){Remove-Item -LiteralPath $blocked -Force}
  [void][IO.Directory]::CreateDirectory($blocked)
  ClickCtl 'main-source-editor';Keys '^{END}';Paste ' retry must retain this B edit'
  WaitUntil {$null-ne(Report).recovery.main.error} 'Storage commit error was not shown' 20
  Assert ((ReadCopy $protected.key).text-ceq'Protection switch retained checkpoint. enabled again') 'Storage failure destroyed the last valid version'
  Step '14-storage-failure-visible' (Report).recovery.main
  Remove-Item -LiteralPath $blocked -Force
  $before=(Report).recovery.main.schedule_count;ClickCtl 'recovery-retry-main';$retried=Checkpoint $false $before
  Assert ((ReadCopy $retried.key).text.Contains('retry must retain this B edit')) 'Recovery retry did not protect latest input'
  Step '15-storage-retry-current-version' $retried
  DiscardMain;ForceStop;StartApp '16-final-clean-restart';Assert ((Report).recovery.pending-eq0) 'Final discarded copy reappeared'
  Step '16-final-clean-restart' @{}
 }
 if($Suite-eq'Large'){
  $env:MDCARD_SEARCH_REPORT=$null
  $p=Join-Path $output 'large-16.md';$line="A measured manuscript paragraph with plain ASCII text for consistent byte counts. `n";$length=16MB-256
  $original=($line*[int][Math]::Ceiling($length/$line.Length)).Substring(0,$length)
  [IO.File]::WriteAllText($p,$original,$utf8);$originalHash=(Get-FileHash $p).Hash
  StartApp 'large-01-edit' $p;ClickCtl 'Write';ClickCtl 'main-source-editor';Keys '^{END}'
  $edit="`nLarge checkpoint 中文 α🙂."
  Paste $edit;$checkpoint=Checkpoint;$key=$checkpoint.key;$expected=$original+$edit
  Assert ((ReadCopy $key).text-ceq$expected) 'Large committed snapshot differs from actual input'
  Assert ((Get-FileHash $p).Hash-eq$originalHash-and(Report).dirty) 'Large checkpoint changed original or dirty state'
  Step 'large-01-current-checkpoint' $checkpoint;ForceStop
  StartApp 'large-02-restart';WaitUntil {(Report).recovery.pending-eq1} 'Large copy was not discovered';RecoverKey $key
  AssertBody $expected 'Large recovered text mismatch';Assert ((Get-FileHash $p).Hash-eq$originalHash) 'Large recovery changed original'
  Step 'large-02-visible-recovery' @{bytes=$utf8.GetByteCount($expected);digest=[MdcardRecoveryHash]::Of($expected);original_sha256=$originalHash}
  $before=(Report).recovery.main.schedule_count;ClickCtl 'main-source-editor';Keys '^{END}';$second=' Further committed edit.';Paste $second;$expected+=$second
  $checkpoint=Checkpoint $false $before;Assert ((ReadCopy $key).text-ceq$expected) 'Large second checkpoint differs from input';ForceStop
  StartApp 'large-03-second-restart';WaitUntil {(Report).recovery.pending-eq1} 'Large recovered-copy crash lost protection';RecoverKey $key
  AssertBody $expected 'Large second recovery mismatch';Assert ((Get-FileHash $p).Hash-eq$originalHash) 'Large repeated recovery changed original'
  Step 'large-03-repeat-crash-recovery' @{bytes=$utf8.GetByteCount($expected);digest=[MdcardRecoveryHash]::Of($expected);original_sha256=$originalHash}
 }
 if($Suite-eq'Performance'){
  $env:MDCARD_SEARCH_REPORT=$null
  $measurements=@()
  foreach($mib in @(1,8,16)){
   $p=Join-Path $output "performance-$mib.md";$line="A measured manuscript paragraph with plain ASCII text for consistent byte counts. `n";$length=$mib*1MB-256
   $text=($line*[int][Math]::Ceiling($length/$line.Length)).Substring(0,$length)
   [IO.File]::WriteAllText($p,$text,$utf8);$hash=(Get-FileHash $p).Hash
   StartApp "performance-$mib" $p;ClickCtl 'Write';ClickCtl 'main-source-editor';Keys '^{END}';Paste "`nRecovery measurement edit."
   $samples=[Collections.Generic.List[object]]::new();$commits=@{};$savedDuringWorker=$false
   Set-Clipboard -Value 'x'
   for($i=0;$i-lt120;$i++){
    [Windows.Forms.SendKeys]::SendWait('^v');Start-Sleep -Milliseconds 80
    $script:app.Refresh();$r=Report;$slots=@($r.recovery.main,$r.recovery.secondary)
    $samples.Add(@{at=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();working_set=$script:app.WorkingSet64;writers=@($slots|Where-Object working).Count;pending_versions=($slots|Measure-Object pending_versions -Sum).Sum;main=$r.recovery.main})
    if($r.recovery.main.committed_at-gt0){$commits[$r.recovery.main.committed_at]=$r.recovery.main}
    if($r.recovery.main.working-and!$savedDuringWorker){Assert ((Get-FileHash $p).Hash-eq$hash) 'Backup wrote the original before manual save';Keys '^s';$savedDuringWorker=$true}
    if($i-eq45){[MdcardUi]::mouse_event(0x800,0,0,120,[UIntPtr]::Zero)}
    if($i-eq70){ClickCtl 'Read';ClickCtl 'Write';ClickCtl 'main-source-editor';Keys '^{END}'}
   }
   $checkpoint2=Checkpoint
   $commits[$checkpoint2.committed_at]=$checkpoint2
   $beforeSaveHash=(Get-FileHash $p).Hash
   Keys '^s';WaitUntil {!(Report).dirty-and!(Report).autosave.main.saving} 'Manual save failed during performance scenario' 30
   $manualSaved=[IO.File]::ReadAllText($p);$afterManualHash=(Get-FileHash $p).Hash
   Assert ($manualSaved.Contains('Recovery measurement edit.')) 'Manual save lost performance edits'
   ClickCtl 'main-source-editor';Keys '^{END}';Paste ' final protected edit';$checkpoint3=Checkpoint
   Assert ((Get-FileHash $p).Hash-eq$afterManualHash-and(Report).dirty) 'Performance recovery changed original after manual save'
   $perf=ReadJsonStable $env:MDCARD_PERFORMANCE_REPORT @('update_cpu_ms','input_event_to_update_finish_ms')
   Copy-Item -LiteralPath $env:MDCARD_PERFORMANCE_REPORT -Destination (Join-Path $output "performance-$mib-report.json")
   $samples.ToArray()|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $output "performance-$mib-samples.json") -Encoding utf8
   $measurements+=@{requested_mib=$mib;source_bytes=$length;commits=@($commits.Values);after_manual_save=$checkpoint3;peak_sampled_working_set=($samples|Measure-Object working_set -Maximum).Maximum;native_peak_working_set=$script:app.PeakWorkingSet64;sample_count=$samples.Count;observed_max_writers=($samples|Measure-Object writers -Maximum).Maximum;observed_max_pending_versions=($samples|Measure-Object pending_versions -Maximum).Maximum;implementation_writer_limit=2;queued_snapshot_limit=0;latest_version_metadata_limit=2;update_cpu_ms=$perf.update_cpu_ms;input_event_to_update_finish_ms=$perf.input_event_to_update_finish_ms;input_samples_with_worker=@($samples|Where-Object writers -GT 0).Count;scroll_and_view_switch=$true;manual_save=$true;manual_save_observed_during_worker=$savedDuringWorker;before_manual_save_sha256=$beforeSaveHash;diagnostics=@('UI','performance','rich metadata','structure metadata');search_manuscript_diagnostic_disabled=$true}
   Step "performance-$mib" $checkpoint2;ForceStop
   # Each measurement uses its own store; pending copies are retained separately.
   $state=Join-Path $output "state-after-$mib";$env:MDCARD_RECOVERY_DIR=$state
  }
  $measurements|ConvertTo-Json -Depth 12|Set-Content -LiteralPath (Join-Path $output 'measurements.json') -Encoding utf8
 }
 if($Suite-eq'Drafts'){
  $normalExe=$Exe
  foreach($case in @('math-valid','math-invalid','diagram-valid','diagram-invalid','rich-conflict','table-normal','table-fault')){
   $state=Join-Path $output "state-$case";$env:MDCARD_RECOVERY_DIR=$state;$Exe=$normalExe;$env:MDCARD_ACCEPTANCE_TABLE_PATCH_FAILURE=$null
   if($case-eq'table-fault'){$Exe=$FaultExe;$env:MDCARD_ACCEPTANCE_TABLE_PATCH_FAILURE='1'}
   $fixture=Join-Path $output "$case.md"
   if($case.StartsWith('diagram')){$original="# Diagram recovery`n`n``````mermaid`ngraph TD; A-->B;`n```````n`nUntouched body.`n"}
   elseif($case.StartsWith('table')){$original="# Table recovery`n`n| Name | Value |`n| --- | --- |`n| First | 10 |`n| Second | 20 |`n"}
   else{$original="# Formula recovery`n`nInline `$x^2`$.`n`nUntouched body.`n"}
   [IO.File]::WriteAllText($fixture,$original,$utf8);$hash=(Get-FileHash $fixture).Hash
   StartApp $case $fixture
   if($case.StartsWith('table')){
    Keys '^+t';ClickCtl 'structure-cell-0-2-0';Keys '^a';Paste "Pending table 中文 $case"
   }else{
    ClickCtl 'Read';if($case.StartsWith('diagram')){ClickRich 'node-0-Edit'}else{ClickRich 'node-0'}
    ClickRich 'floating-editor';Keys '^a'
    if($case-eq'math-invalid'-or$case-eq'rich-conflict'){Paste '$'}
    elseif($case-eq'diagram-invalid'){Paste "graph TD; A-->B;`n```````nDifferent block"}
    else{
     Set-Clipboard -Value $(if($case-eq'math-valid'){'z^3'}else{'graph TD; A-->B;'});[Windows.Forms.SendKeys]::SendWait('^v')
     Set-Clipboard -Value $(if($case-eq'math-valid'){'x'}else{' '})
     for($i=0;$i-lt115;$i++){[Windows.Forms.SendKeys]::SendWait('^v');Start-Sleep -Milliseconds 100}
    }
    if($case-eq'rich-conflict'){
     Start-Sleep -Milliseconds 500;MoveFloatingRight;ClickCtl 'Write';ClickCtl 'main-source-editor';Keys '^a';Paste '# Source changed while the node draft remained separate.'
     ClickRich 'floating-editor';Keys '^a';Paste 'q^4'
    }
   }
   if($case-in@('math-valid','diagram-valid')){
    $key=(Report).recovery.main.key;Assert ($null-ne$key) 'Continuous valid draft was never checkpointed';$copy=ReadCopy $key
   }else{$slot=Checkpoint;$key=$slot.key;$copy=ReadCopy $key}
   Assert ((Get-FileHash $fixture).Hash-eq$hash) 'Draft checkpoint wrote original'
   if($case-eq'table-normal'){Assert ($copy.text.Contains("Pending table 中文 $case")) 'Normal table input was not synchronized and protected';Assert ($copy.drafts.Count-eq0) 'Successfully synchronized table input remained an independent draft'}
   else{Assert ($copy.drafts.Count-gt0) 'Independent draft missing from committed checkpoint'}
   $bodyExpected=$copy.text;$draftExpected=if($copy.drafts.Count-gt0){$copy.drafts[0].text}else{''}
   Step "$case-before-kill" @{key=$key;drafts=$copy.drafts.Count;candidate_sha256=(Get-FileHash $Exe).Hash;injected_patch_failure=($case-eq'table-fault')}
   ForceStop;StartApp "$case-restart";RecoverKey $key
   AssertSourceDigest $bodyExpected 'Recovered body differed from committed draft checkpoint'
   if($copy.drafts.Count-gt0){WaitUntil {(Report).recovery.main.restored_drafts-gt0} 'Restored independent draft count missing';[Windows.Forms.Clipboard]::Clear();ClickCtl 'recovery-draft-copy-0';$visible=[string](Get-Clipboard -Raw);Assert ($visible.Replace("`r`n","`n")-ceq$draftExpected.Replace("`r`n","`n")) 'Visible independent draft copy differed from committed draft';ClickCtl 'recovery-drafts-close'}
   AssertBody $bodyExpected 'Visible body differs from committed checkpoint'
   Assert ((Get-FileHash $fixture).Hash-eq$hash) 'Draft UI recovery changed original'
   Step "$case-restored" @{key=$key;body_exact=$true;draft_copy_exact=($copy.drafts.Count-gt0);original_sha256=$hash;injected_patch_failure=($case-eq'table-fault')}
   if($case-eq'table-normal'){
    $chosen=Join-Path $output 'table-normal-explicit-save.md';SaveAs $chosen;WaitUntil {!(Report).dirty-and$null-eq(Report).recovery.main.key} 'Manual table save left a false recovery requirement'
    Keys '^+p';ClickCtl 'Auto save';Keys '{ESC}';Keys '^+t';ClickCtl 'structure-cell-0-2-0';Keys '^a';Paste 'Automatic table edit after recovery'
    WaitUntil {!(Report).dirty-and$null-eq(Report).recovery.main.key-and[IO.File]::ReadAllText($chosen).Contains('Automatic table edit after recovery')} 'Synchronized table draft blocked auto save or left false recovery data' 30
    Step 'table-normal-manual-and-auto-save-clean' @{drafts_cleared=$true;original_fixture_unchanged=((Get-FileHash $fixture).Hash-eq$hash)}
    ForceStop;StartApp 'table-normal-clean-restart';Assert ((Report).recovery.pending-eq0) 'Saved table produced a false crash recovery prompt';Step 'table-normal-no-false-prompt' @{}
    # The fault case below starts from the same explicit original-autosave-off rule.
    ForceStop;@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15;autosave=$false}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
   }
   ForceStop
  }
  $Exe=$normalExe
 }
 @{
  verification_id=$verificationId;date=[DateTimeOffset]::Now.ToString('o');timezone='Asia/Tokyo';suite=$Suite;exe=(Resolve-Path -LiteralPath $Exe).Path;exe_sha256=(Get-FileHash -LiteralPath $Exe).Hash
  real_native_window=$true;real_mouse_keyboard=$true;forced_termination='Process.Kill on only the launched test process';os=[Environment]::OSVersion.VersionString;steps=$steps.ToArray();isolated_state=$env:MDCARD_RECOVERY_DIR
 }|ConvertTo-Json -Depth 14|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
} catch {
 if($script:app-and!$script:app.HasExited){Shot 'failure'}
 throw
} finally {
 foreach($p in $processes){if(!$p.HasExited){$p.Kill();$p.WaitForExit()}}
 if($null-ne$previousClipboard){[Windows.Forms.Clipboard]::SetDataObject($previousClipboard,$true)}
 foreach($name in $variables){[Environment]::SetEnvironmentVariable($name,$previousEnv[$name],'Process')}
}
Write-Output $output
