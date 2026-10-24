# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe,[string]$Stage='candidate')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\workbench-clipboard.ps1"
. "$PSScriptRoot\verify-search-master-windows.ps1" -Exe $Exe -LoadFunctionsOnly
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;using System.Text;using System.Runtime.InteropServices;
public static class WorkbenchPickerNative {
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint process);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,StringBuilder text,int length);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h,StringBuilder text,int length);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h,int id);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h,uint message,IntPtr wparam,IntPtr lparam);
}
'@
$output=Join-Path $root "evidence\workbench-redesign-2026-10-22\master\navigation-$Stage"
$fixtures=Join-Path $output 'first-workspace'
$nextRoot=Join-Path $output '第二工作区 long workspace'
New-Item -ItemType Directory -Force $output,$fixtures,$nextRoot,(Join-Path $nextRoot 'notes\architecture')|Out-Null
$a=Join-Path $fixtures 'original.md'
$nextFile=Join-Path $nextRoot 'notes\architecture\一份需要完整路径提示的很长设计文档名称.md'
$original="# Navigation protection`r`n`r`n中文稿件 🦀`r`n"
[IO.File]::WriteAllText($a,$original,[Text.UTF8Encoding]::new($true))
[IO.File]::WriteAllText($nextFile,"# 新工作区`n`nOnly in next workspace.",$utf8)
$initialHash=(Get-FileHash $a).Hash
$settings=Join-Path $output 'settings.json';$report=Join-Path $output 'state.json'
$env:MDCARD_UI_REPORT=Join-Path $output 'ui.json'
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$savedClipboard=[WorkbenchClipboard]::GetDataObject()
$result=@{verification_id=[Guid]::NewGuid().ToString();binary_sha256=(Get-FileHash $Exe).Hash;stage=$Stage;cases=@()}
function Click([float]$X,[float]$Y){[void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-1),0,0,0,0,0x43);[void][MdcardUi]::SetForegroundWindow($script:app.MainWindowHandle);if([MdcardUi]::GetForegroundWindow()-ne$script:app.MainWindowHandle){throw 'Main window did not own foreground before native mouse input'};$b=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0;[void][MdcardUi]::SetCursorPos($b.Left+[int]($X*$scale),$b.Top+[int]($Y*$scale));Start-Sleep -Milliseconds 180;[MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 60;[MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 300;[void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-2),0,0,0,0,0x43)}
function UiState {for($i=0;$i-lt30;$i++){try{return Get-Content -Raw -LiteralPath $env:MDCARD_UI_REPORT|ConvertFrom-Json}catch{Start-Sleep -Milliseconds 30}};throw 'No UI report'}
function UiClick([string]$Name){
 [void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-1),0,0,0,0,0x43);[void][MdcardUi]::SetForegroundWindow($script:app.MainWindowHandle)
 if([MdcardUi]::GetForegroundWindow()-ne$script:app.MainWindowHandle){throw 'App main window does not own foreground before UI click'}
 $r=@((UiState).controls|Where-Object {$_.name-ceq$Name-and$_.enabled})|Select-Object -Last 1
 if(!$r){throw "No enabled UI control $Name"}
 $x=($r.rect[0]+$r.rect[2])/2;$y=($r.rect[1]+$r.rect[3])/2;$b=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0
 [void][MdcardUi]::SetCursorPos($b.Left+[int]($x*$scale),$b.Top+[int]($y*$scale));Start-Sleep -Milliseconds 180;[MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 60;[MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 350
 [void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-2),0,0,0,0,0x43)
}
function OpenPicker([switch]$FromMenu) {
 if($FromMenu){
  [void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-1),0,0,0,0,0x43);[void][MdcardUi]::SetForegroundWindow($script:app.MainWindowHandle)
  Click 169 62
  $ui=UiState;$save=@($ui.controls|Where-Object {$_.name -eq 'Save'})|Select-Object -Last 1;$export=@($ui.controls|Where-Object {$_.name -eq 'Export…'})|Select-Object -Last 1
  if(!$save-or!$export){throw 'Visible File menu controls not found'}
  Click (($save.rect[0]+$save.rect[2])/2) (($save.rect[3]+$export.rect[1])/2)
  [void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-2),0,0,0,0,0x43)
 }else{UiClick 'Open folder…'}
 $until=[DateTime]::UtcNow.AddSeconds(5)
 while([DateTime]::UtcNow-lt$until){
  $handle=[MdcardUi]::GetForegroundWindow();$owner=[uint32]0;[void][WorkbenchPickerNative]::GetWindowThreadProcessId($handle,[ref]$owner)
  $class=[Text.StringBuilder]::new(256);$title=[Text.StringBuilder]::new(512);[void][WorkbenchPickerNative]::GetClassName($handle,$class,256);[void][WorkbenchPickerNative]::GetWindowText($handle,$title,512)
  if($owner-eq$script:app.Id-and$class.ToString()-eq'#32770'-and$title.ToString().Contains('Open folder')){$script:pickerHandle=$handle;$result.native_picker=@{process_id=$owner;class=$class.ToString();title=$title.ToString()};Start-Sleep -Milliseconds 300;return}
  Start-Sleep -Milliseconds 80
 }
 throw 'Native folder chooser never received foreground; no path input was sent'
}
function AwaitUi([scriptblock]$Predicate,[string]$Message){$until=[DateTime]::UtcNow.AddSeconds(15);while([DateTime]::UtcNow-lt$until){$s=UiState;if(&$Predicate $s){return $s};Start-Sleep -Milliseconds 80};Shot 'failure';throw $Message}
try{
 # Clear any capture selection opened by the superseded Alt+S driver attempt.
 [Windows.Forms.SendKeys]::SendWait('{ESC}')
 StartReview
 Click 450 250;Keys '^{END}';Paste ' UNSAVED'
 Await {param($s)$s.text-ceq($original+' UNSAVED')} 'Dirty buffer not established'|Out-Null
 Keys '+{HOME}';[WorkbenchClipboard]::SetText('SENTINEL-BEFORE-FOLDER');Keys '^c'
 $expectedSelection=[WorkbenchClipboard]::GetText();if($expectedSelection-cne' UNSAVED'){throw 'Could not establish exact folder-return selection'}
 $generation=(UiState).document_generation;$selection=(UiState).selection;$firstRoot=(UiState).workspace_root
 OpenPicker;Keys '{ESC}'
 $cancel=AwaitUi {param($s)$s.workspace_root-ceq$firstRoot-and$s.dirty} 'Folder cancel lost workspace or dirty state'
 Same (State).text ($original+' UNSAVED') 'Folder cancel source'
 [WorkbenchClipboard]::SetText('SENTINEL-AFTER-FOLDER-CANCEL');Keys '^c'
 if([WorkbenchClipboard]::GetText()-cne$expectedSelection){throw 'Native folder cancel did not restore exact editor focus and selection'}
 $result.cases+=@{name='native-folder-cancel';passed=$true};Shot '01-folder-cancel-keeps-draft'
 OpenPicker -FromMenu
 Keys '^l';Set-Clipboard -Value $nextRoot;Keys '^v';Keys '{ENTER}';Start-Sleep -Milliseconds 350
 $dialog=[Windows.Automation.AutomationElement]::FromHandle($script:pickerHandle)
 $elements=$dialog.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
 $nativeControls=@($elements|ForEach-Object {@{name=$_.Current.Name;id=$_.Current.AutomationId;class=$_.Current.ClassName;type=$_.Current.ControlType.ProgrammaticName;enabled=$_.Current.IsEnabled}})
 $nativeControls|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $output 'native-folder-controls.json') -Encoding utf8
 $select=$dialog.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.AndCondition]::new([Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::AutomationIdProperty,'1'),[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::Button)))
 if($select){
  $result.select_folder_button=@{name=$select.Current.Name;automation_id=$select.Current.AutomationId}
  ([Windows.Automation.InvokePattern]$select.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)).Invoke()
 }else{
  $nativeButton=[WorkbenchPickerNative]::GetDlgItem($script:pickerHandle,1)
  $caption=[Text.StringBuilder]::new(256);[void][WorkbenchPickerNative]::GetWindowText($nativeButton,$caption,256)
  if($nativeButton-eq[IntPtr]::Zero-or![WorkbenchPickerNative]::IsWindowVisible($nativeButton)-or($caption.ToString()-notmatch'选择文件夹|Select Folder')){throw "Visible native Select Folder button not found; caption=$caption"}
  $result.select_folder_button=@{name=$caption.ToString();native_control_id=1;visible=$true;uia_scope='Shell DirectUI omitted native sibling; invoked verified visible Win32 button'}
  [void][WorkbenchPickerNative]::SendMessage($nativeButton,0xF5,[IntPtr]::Zero,[IntPtr]::Zero)
 }
 $changed=AwaitUi {param($s)$s.workspace_root.EndsWith('第二工作区 long workspace')-and!$s.workspace_scanning} 'Native folder chooser did not publish the selected workspace'
 Same (State).text ($original+' UNSAVED') 'Folder change source'
 if(!$changed.dirty-or$changed.document_generation-ne$generation){throw 'Folder change replaced current dirty document identity'}
 if((Get-FileHash $a).Hash-cne$initialHash){throw 'Folder navigation changed disk bytes'}
 $result.cases+=@{name='native-open-folder-preserves-dirty-identity-and-disk';passed=$true};Shot '02-new-workspace-dirty-document'
 UiClick 'navigation:Outline';Shot '03-outline-view';UiClick 'navigation:Recent';Shot '04-recent-view';UiClick 'navigation:Files'
 UiClick 'tree:notes';UiClick 'tree:notes/architecture';Shot '05-expanded-long-path'
 # Global/quick workers must now use the explicitly selected root, even while
 # the current dirty document still belongs to the original workspace.
 Keys '^p';Fill 'quick-query' '一份需要完整路径提示'
 $quick=Await {param($s)!$s.query_pending-and$s.quick.Count-eq1} 'New workspace Quick Switcher did not use selected root'
 if(!$quick.quick[0].path.EndsWith('一份需要完整路径提示的很长设计文档名称.md')){throw 'Stale worker results'}
 Keys '{ESC}';Keys '^z'
 Await {param($s)$s.text-ceq$original} 'Undo history was lost across workspace navigation'|Out-Null
 Keys '^s'
 if((Get-FileHash $a).Hash-cne$initialHash){throw 'Undo/save did not restore original bytes'}
 $result.cases+=@{name='new-search-root-and-original-document-undo';passed=$true};Shot '06-original-draft-undo-preserved'
 Keys '^p';Fill 'quick-query' '一份需要完整路径提示';Await {param($s)!$s.query_pending-and$s.quick.Count-eq1} 'Secondary target ready'|Out-Null;Keys '+{ENTER}'
 Await {param($s)$null-ne$s.secondary} 'Secondary did not open'|Out-Null
 Control 'secondary-editor';AwaitUi {param($s)$s.active_pane-eq'secondary'} 'Secondary pane not marked active'|Out-Null
 UiClick 'Read';Click 450 110
 AwaitUi {param($s)$s.active_pane-eq'main'} 'Main header did not take active ownership'|Out-Null
 Start-Sleep -Milliseconds 300;if((UiState).active_pane-ne'main'){throw 'Stale secondary focus reclaimed main header'}
 Control 'secondary-editor';UiClick 'Read';Click 450 240
 AwaitUi {param($s)$s.active_pane-eq'main'} 'Main Read preview did not take active ownership'|Out-Null
 Same (State).text $original 'Main Read focus source'
 $result.cases+=@{name='main-header-and-read-preview-take-active-pane';passed=$true};Shot '07-main-read-and-secondary'
 Keys '^q';$result.clean_exit=$script:app.WaitForExit(5000)
 $result.passed=$result.clean_exit-and($result.cases.Count-eq4)
 $result.busy_guard='Deterministic real-egui/core regression: workspace_navigation_preserves_dirty_buffer_history_and_blocks_during_transaction; no synthetic UI report counted as native transaction coverage.'
}catch{$result.error=$_.Exception.Message;$result.passed=$false;Shot 'failure'}
finally{if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()};if($savedClipboard){[WorkbenchClipboard]::SetDataObject($savedClipboard,$true)};$result|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8}
$result|ConvertTo-Json -Depth 8
if(!$result.passed){throw 'Workspace navigation verification failed'}
