# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$Exe="$PSScriptRoot\..\artifacts\global-search\MDcard.exe",[switch]$LoadFunctionsOnly,[string]$ReviewStage='master-review')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
. "$PSScriptRoot\workbench-native-input.ps1"
if($ReviewStage-notmatch'^[A-Za-z0-9_-]+$'){throw 'Use a simple fresh stage name'}
$output=Join-Path $root "evidence\search\$ReviewStage\windows"
$fixtures=Join-Path $root "artifacts\search\$ReviewStage\workspace"
if(!$LoadFunctionsOnly-and(Test-Path -LiteralPath (Join-Path $output 'result.json'))){throw 'Retain previous evidence; use a fresh Stage'}
New-Item -ItemType Directory -Force $output,$fixtures,(Join-Path $fixtures 'nested')|Out-Null
$settings=Join-Path $output 'settings.json'
$report=Join-Path $output 'state.json'
$a=Join-Path $fixtures 'a.md';$b=Join-Path $fixtures 'b.md';$c=Join-Path $fixtures 'nested\c.md'
$original="# Main 🦀`n`ncat 12`ncat 34`n"
$other="# Second`ncat 56`n"
$excluded="# Nested`ncat 78`n"
$jumpPath=Join-Path $fixtures 'jump.md'
$jumpText=((1..349|ForEach-Object {"line $_ 中文 🦀 context"})-join"`n")+"`n前缀 🦀 jump-unique-marker 后缀`nlast`n"
$utf8=[Text.UTF8Encoding]::new($false)
if(!$LoadFunctionsOnly){
 Add-Type @'
using System; using System.Runtime.InteropServices;
public static class SearchReviewInputNative {
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern IntPtr GetKeyboardLayout(uint t);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr LoadKeyboardLayout(string n,uint f);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
}
'@
 [IO.File]::WriteAllText($a,$original,$utf8)
 [IO.File]::WriteAllText($b,$other,$utf8)
 [IO.File]::WriteAllText($c,$excluded,$utf8)
 [IO.File]::WriteAllText($jumpPath,$jumpText,$utf8)
 @{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
}
function State {
 if(!(Test-Path -LiteralPath $report)){return $null}
 try {Get-Content -Raw -LiteralPath $report -Encoding UTF8|ConvertFrom-Json} catch {$null}
}
function Await([scriptblock]$Condition,[string]$Message,[int]$Seconds=12) {
 $until=[DateTime]::UtcNow.AddSeconds($Seconds)
 while([DateTime]::UtcNow-lt$until){$s=State;if($s -and (& $Condition $s)){return $s};if($script:app.HasExited){throw "App exited: $Message"};Start-Sleep -Milliseconds 80}
 Shot 'failure'
 throw "$Message; last state: $(State|Select-Object open,quick_open,busy,path,error,query,query_pending,results,preview_files,controls|ConvertTo-Json -Depth 5 -Compress)"
}
function Control([string]$Name) {
 $s=Await {param($s) @($s.controls|Where-Object name -eq $Name).Count -gt 0} "No control $Name"
 Start-Sleep -Milliseconds 200
 $s=Await {param($s) @($s.controls|Where-Object name -eq $Name).Count -gt 0} "No settled control $Name"
 $r=@($s.controls|Where-Object name -eq $Name)[0].rect
 Click (($r[0]+$r[2])/2) (($r[1]+$r[3])/2)
}
function Paste([string]$Text){Set-Clipboard -Value $Text;Keys '^v'}
function Fill([string]$Name,[string]$Text){Control $Name;Keys '^a';if($Text){Paste $Text}else{Keys '{BACKSPACE}'}}
function Same([string]$Actual,[string]$Expected,[string]$Case){if($Actual-cne$Expected){throw "$Case mismatch. Actual=[$Actual] Expected=[$Expected]"}}
function StartReview {
 $launch=[Diagnostics.ProcessStartInfo]::new((Resolve-Path -LiteralPath $Exe).Path);$launch.UseShellExecute=$false
 $launch.Environment['MDCARD_SETTINGS_PATH']=$settings;$launch.Environment['MDCARD_SEARCH_REPORT']=$report
 $launch.Environment['MDCARD_READING_STATE_PATH']=(Join-Path $output 'reading-state.json')
 $launch.Environment['MDCARD_RENDERER_REPORT']=(Join-Path $output 'renderer.json')
 $renderer=Join-Path (Split-Path (Resolve-Path -LiteralPath $Exe).Path -Parent) 'renderer'
 if(!(Test-Path -LiteralPath $renderer -PathType Container)){$renderer=Join-Path $root 'dist\windows\renderer'}
 $launch.Environment['MDCARD_RENDERER_DIR']=$renderer
 $launch.ArgumentList.Add($a)
 $script:app=[Diagnostics.Process]::Start($launch);Start-Sleep -Seconds 3;$script:app.Refresh()
 if($script:app.HasExited-or$script:app.MainWindowHandle-eq0){throw 'No test window'}
 [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,50,50,1550,1025,$true)
 [void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-1),0,0,0,0,0x43)
 [void][MdcardUi]::SetForegroundWindow($script:app.MainWindowHandle);Start-Sleep -Milliseconds 250;Click 500 27
 [void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-2),0,0,0,0,0x43)
 if([MdcardUi]::GetForegroundWindow()-ne$script:app.MainWindowHandle){throw 'Foreground ownership failed'}
 [uint32]$owner=0;$thread=[SearchReviewInputNative]::GetWindowThreadProcessId($script:app.MainWindowHandle,[ref]$owner)
 if($owner-ne$script:app.Id){throw 'Input layout target does not belong to the test process'}
 $script:searchReviewOldLayout=[SearchReviewInputNative]::GetKeyboardLayout($thread)
 $english=[SearchReviewInputNative]::LoadKeyboardLayout('00000409',0)
 [void][SearchReviewInputNative]::PostMessage($script:app.MainWindowHandle,0x50,[IntPtr]::Zero,$english)
 Start-Sleep -Milliseconds 250
}
function StopReview {
 if($script:searchReviewOldLayout){[void][SearchReviewInputNative]::PostMessage($script:app.MainWindowHandle,0x50,[IntPtr]::Zero,$script:searchReviewOldLayout)}
 Keys '^q';if(!$script:app.WaitForExit(5000)){throw 'Test window failed to close'}
}
if($LoadFunctionsOnly){return}
$priorClipboard=$null
try{$priorClipboard=[Windows.Forms.Clipboard]::GetDataObject()}catch{}
$verificationId=[Guid]::NewGuid().ToString()
try {
 StartReview
 Keys '^h';Fill 'find-query' '(?m)^(?<animal>cat) (\d+)$';Control 'regex';Fill 'replace-query' '$<animal>=$2'
 Await {param($s)$s.results-eq2 -and !$s.error} 'Current regex results'|Out-Null
 Control 'preview';Await {param($s)$s.preview_files-eq1} 'Current diff'|Out-Null;Shot '01-captures-diff'
 Control 'apply';Await {param($s)!$s.open-and!$s.busy} 'Single replace complete'|Out-Null
 $after=$original.Replace('cat 12','cat=12').Replace('cat 34','cat=34')
 Same (State).text $after 'Single document memory'
 Keys '^s';Same ([IO.File]::ReadAllText($a)) $after 'Single save'
 Keys '^z';Keys '^s';Same ([IO.File]::ReadAllText($a)) $original 'One CtrlZ restores all replacements'
 Keys '^+z';Keys '^s';Same ([IO.File]::ReadAllText($a)) $after 'One redo restores all replacements'
 Shot '02-atomic-redo'
 Keys '^z';Keys '^s';Same ([IO.File]::ReadAllText($a)) $original 'Restore fixture'
 Keys '^h';Fill 'find-query' '(';Await {param($s)[bool]$s.error} 'Inline invalid regex'|Out-Null;Shot '03-inline-regex-error'
 if($script:app.HasExited){throw 'Invalid regex crashed application'}
 Keys '{ESC}'
 Keys '^+f';Fill 'find-query' 'cat';Fill 'replace-query' 'dog';Fill 'include' '**/*.md';Fill 'exclude' 'nested/**'
 Await {param($s)$s.results-eq3-and!$s.error} 'Filtered workspace results'|Out-Null
 Control 'preview';Await {param($s)$s.preview_files-eq2} 'Workspace diff'|Out-Null;Shot '04-workspace-diff'
 Control 'apply';Await {param($s)!$s.busy-and$s.preview_files-eq0-and$s.text.Contains('dog 12')} 'Workspace commit memory'|Out-Null
 Same ([IO.File]::ReadAllText($a)) ($original.Replace('cat','dog')) 'Workspace main disk'
 Same ([IO.File]::ReadAllText($b)) ($other.Replace('cat','dog')) 'Workspace other disk'
 Same ([IO.File]::ReadAllText($c)) $excluded 'Glob excluded file'
 Shot '05-workspace-committed';Keys '{ESC}';StopReview
 StartReview;Keys '^+f';Control 'history';Start-Sleep -Milliseconds 600;Control 'rollback-0'
 Await {param($s)!$s.busy-and$s.text-eq$original} 'Restart rollback memory'|Out-Null
 Same ([IO.File]::ReadAllText($a)) $original 'Restart rollback main disk'
 Same ([IO.File]::ReadAllText($b)) $other 'Restart rollback other disk'
 Shot '06-restart-rollback';Keys '{ESC}';Keys '{ESC}';StopReview
 StartReview
 Keys '^p';Fill 'quick-query' 'b.md';Await {param($s)@($s.quick).Count-eq1} 'Fuzzy quick result'|Out-Null
 Shot '07-quick-switcher';Keys '+{ENTER}';Await {param($s)[bool]$s.secondary} 'Second document pane'|Out-Null
 Control 'secondary-editor';Keys '^{END}';Paste 'second-unsaved'
 Await {param($s)$s.secondary.dirty} 'Second document dirty'|Out-Null
 Keys '^s';Await {param($s)!$s.busy-and!$s.secondary.dirty} 'Secondary CtrlS'|Out-Null
 Same ([IO.File]::ReadAllText($b)) ($other+'second-unsaved') 'Secondary CtrlS disk'
 Same ([IO.File]::ReadAllText($a)) $original 'Main unchanged by secondary CtrlS'
 Control 'secondary-editor';Keys '^{END}';Paste '-new-dirty'
 Await {param($s)$s.secondary.dirty} 'Secondary new edit'|Out-Null
 [void]$script:app.CloseMainWindow();Start-Sleep -Milliseconds 650
 if($script:app.HasExited){throw 'Native close discarded dirty second document'}
 Same (State).secondary.text ($other+'second-unsaved-new-dirty') 'Native close retains secondary memory'
 Shot '08-secondary-close-protection'
 $script:app.Kill();$script:app.WaitForExit();StartReview
 Keys '^+f';Fill 'find-query' 'jump-unique-marker'
 Await {param($s)$s.results-eq1-and$s.groups[0].hits[0].line-eq350} 'Exact result at line 350'|Out-Null
 Shot '09-grouped-context-highlight';Control 'result-0-0'
 Await {param($s)!$s.open-and!$s.busy-and$s.path.EndsWith('jump.md')} 'Open line target'|Out-Null
 Keys '^c';Same ([Windows.Forms.Clipboard]::GetText()) 'jump-unique-marker' 'Precise Unicode-aware selection'
 Shot '10-jump-line-350'
 Keys '^p';Fill 'quick-query' 'md';$q=Await {param($s)@($s.quick).Count-ge2} 'Quick keyboard multiple choices'
 $expectedPath=$q.quick[0].path;Keys '{DOWN}';Keys '{UP}';Keys '{ENTER}'
 Await {param($s)!$s.quick_open-and!$s.busy-and$s.path-eq$expectedPath} 'ArrowDown ArrowUp Enter chosen path'|Out-Null
 Keys '^p';Fill 'quick-query' '';Await {param($s)@($s.quick).Count-gt0-and$s.quick[0].path-eq$expectedPath} 'MRU ranks most recent first'|Out-Null
 $newName='created-'+[Guid]::NewGuid().ToString('N')+'.md';Fill 'quick-query' $newName
 Await {param($s)@($s.controls|Where-Object name -eq 'quick-create').Count-eq1} 'Missing filename offers create'|Out-Null
 Shot '11-new-document-offer';Keys '{ENTER}'
 Await {param($s)!$s.quick_open-and!$s.busy-and$s.path.EndsWith($newName)} 'Enter creates and opens missing document'|Out-Null
 Same ([IO.File]::ReadAllText((Join-Path $fixtures $newName))) '' 'Created file is empty'
 Shot '12-created-document';StopReview
 @{
  passed=$true;stage=$ReviewStage;harness_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant()
  verification_id=$verificationId;date=[DateTimeOffset]::UtcNow.ToString('o');timezone='UTC';exe=(Resolve-Path -LiteralPath $Exe).Path;exe_sha256=(Get-FileHash -LiteralPath $Exe).Hash
  real_mouse_keyboard=$true;capture_diff=$true;single_atomic_undo_redo_exact_bytes=$true;inline_invalid_regex=$true
  workspace_diff_and_include_exclude=$true;workspace_memory_disk_sync=$true;restart_snapshot_rollback=$true
  quick_shift_enter_dual_documents=$true;secondary_ctrl_s_routes_to_secondary=$true;secondary_native_close_protected=$true
  grouped_highlight_context=$true;exact_jump_line_350_unicode_selection=$true;quick_arrow_enter_and_mru=$true;quick_missing_enter_creates=$true
 }|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
} catch {
 @{
  passed=$false;stage=$ReviewStage;verification_id=$verificationId
  date=[DateTimeOffset]::UtcNow.ToString('o');exe=(Resolve-Path -LiteralPath $Exe).Path
  exe_sha256=(Get-FileHash -LiteralPath $Exe).Hash
  harness_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant()
  error=$_.Exception.Message;error_stack=$_.ScriptStackTrace;real_mouse_keyboard=$true
 }|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
 throw
} finally {
 if($script:app-and!$script:app.HasExited){
  if($script:searchReviewOldLayout){[void][SearchReviewInputNative]::PostMessage($script:app.MainWindowHandle,0x50,[IntPtr]::Zero,$script:searchReviewOldLayout)}
  $script:app.Kill();$script:app.WaitForExit()
 }
 if($null-ne$priorClipboard){[Windows.Forms.Clipboard]::SetDataObject($priorClipboard,$true)}
}
Write-Output $output
