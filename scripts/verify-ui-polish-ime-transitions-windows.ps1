# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe,[string]$Stage='baseline')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-search-master-windows.ps1" -Exe $Exe -LoadFunctionsOnly
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class UiImeProbe {
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern IntPtr GetKeyboardLayout(uint t);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr LoadKeyboardLayout(string n,uint f);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("imm32.dll")] public static extern IntPtr ImmGetContext(IntPtr h);
 [DllImport("imm32.dll")] public static extern bool ImmReleaseContext(IntPtr h,IntPtr c);
 [DllImport("imm32.dll")] public static extern bool ImmSetOpenStatus(IntPtr c,bool o);
 [DllImport("imm32.dll")] public static extern bool ImmGetOpenStatus(IntPtr c);
 [DllImport("imm32.dll")] public static extern bool ImmSetConversionStatus(IntPtr c,uint v,uint s);
}
'@
$output=Join-Path $root "evidence\ui-polish-2026-10-22\master\ime-$Stage"
$fixtures=Join-Path $output 'workspace'
New-Item -ItemType Directory -Force $output,$fixtures | Out-Null
$settings=Join-Path $output 'settings.json';$report=Join-Path $output 'state.json';$a=Join-Path $fixtures 'ime.md'
$original="# IME verification`n`nBASE:"
[IO.File]::WriteAllText($a,$original,$utf8)
@{language='zh-CN';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$priorClipboard=[Windows.Forms.Clipboard]::GetDataObject()
$record=@{verification_id=[Guid]::NewGuid().ToString();binary_sha256=(Get-FileHash -LiteralPath $Exe).Hash;stage=$Stage;method='Windows Microsoft Pinyin TSF selected by WM_INPUTLANGCHANGEREQUEST; physical virtual-key SendKeys, no Unicode paste';screenshots_require_visual_review=$true}
try {
 StartReview
 Click 450 250;Keys '^{END}'
 [uint32]$processId=0
 $thread=[UiImeProbe]::GetWindowThreadProcessId($script:app.MainWindowHandle,[ref]$processId)
 $previousLayout=[UiImeProbe]::GetKeyboardLayout($thread)
 $chinese=[UiImeProbe]::LoadKeyboardLayout('00000804',0)
 $record.previous_layout=$previousLayout.ToInt64().ToString('X');$record.requested_layout=$chinese.ToInt64().ToString('X')
 [void][UiImeProbe]::PostMessage($script:app.MainWindowHandle,0x50,[IntPtr]::Zero,$chinese)
 Start-Sleep -Milliseconds 500
 $context=[UiImeProbe]::ImmGetContext($script:app.MainWindowHandle)
 $record.imm_context=$context.ToInt64();$record.open_requested=[UiImeProbe]::ImmSetOpenStatus($context,$true)
 $record.native_requested=[UiImeProbe]::ImmSetConversionStatus($context,1,0)
 $record.ime_open=[UiImeProbe]::ImmGetOpenStatus($context)
 [void][UiImeProbe]::ImmReleaseContext($script:app.MainWindowHandle,$context)
 Keys 'nihao'
 Shot '01-preedit-candidate'
 $record.preedit_text=(State).text
 Keys ' '
 Shot '02-committed'
 $record.committed_text=(State).text
 $record.commit_exactly_once=($record.committed_text-ceq($original+'你好'))
 Keys 'ceshi';Shot '03-cancel-preedit';Keys '{ESC}'
 $record.after_cancel=(State).text;$record.cancel_preserves_committed=($record.after_cancel-ceq$record.committed_text)
 $record.transitions=@()
 foreach($transition in @('preferences','view-cycle')) {
  Click 450 250;Keys '^{END}';Keys 'ceshi'
  $preedit=(State).text;Shot "$transition-preedit"
  if($transition-eq'preferences'){Keys '^+p';Shot "$transition-open";Keys '{ESC}'}
  else {Keys '^e';Shot "$transition-read";Keys '^e'}
  $afterSwitch=(State).text
  Keys '{ESC}';$afterCancel=(State).text
  $record.transitions+=@{action=$transition;preedit=$preedit;after_switch=$afterSwitch;after_cancel=$afterCancel;committed_prefix_preserved=$afterCancel.StartsWith($record.committed_text);no_orphan_preedit=($afterCancel-ceq$record.committed_text-or$afterCancel-ceq($record.committed_text+'测试'))}
  # Normalize only this isolated fixture before the next independent case.
  Click 450 250;Keys '^a';Paste $record.committed_text;Keys '^{END}'
 }
 Keys '^+p';Shot '04-preferences';Keys '{ESC}';Keys '^e';Keys '^e';Keys '^e'
 $record.after_view_operations=(State).text;$record.view_text_unchanged=($record.after_view_operations-ceq$record.committed_text)
 Keys '^s'
 $record.saved_text=[IO.File]::ReadAllText($a);$record.save_exact=($record.saved_text-ceq$record.committed_text)
 $record.actual_layout=([UiImeProbe]::GetKeyboardLayout($thread)).ToInt64().ToString('X')
 [void][UiImeProbe]::PostMessage($script:app.MainWindowHandle,0x50,[IntPtr]::Zero,$previousLayout)
 Keys '^q';$record.clean_exit=$script:app.WaitForExit(5000)
} catch { $record.error=$_.Exception.Message }
finally {
 if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()}
 if($priorClipboard){[Windows.Forms.Clipboard]::SetDataObject($priorClipboard,$true)}
 $record|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
}
$record|ConvertTo-Json -Depth 5
