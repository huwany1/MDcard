# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
# Every actual click must be delivered to the owned native test window.
function Click([float]$X,[float]$Y){
 $handle=$script:app.MainWindowHandle
 [void][MdcardUi]::SetWindowPos($handle,[IntPtr]::new(-1),0,0,0,0,0x43)
 [void][MdcardUi]::SetForegroundWindow($handle)
 if([MdcardUi]::GetForegroundWindow()-ne$handle){
  # Windows may reject SetForegroundWindow until actual user input. The known
  # test PID/window is temporarily topmost; click its neutral titlebar, then
  # require ownership before delivering the requested control/input action.
  $activation=Bounds;$activationScale=[MdcardUi]::GetDpiForWindow($handle)/96.0
  [void][MdcardUi]::SetCursorPos($activation.Left+[int](320*$activationScale),$activation.Top+[int](20*$activationScale));Start-Sleep -Milliseconds 180
  [MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 60;[MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 250
  $script:activationClicks++
 }
 if([MdcardUi]::GetForegroundWindow()-ne$handle){throw 'Test main window does not own foreground before native click'}
 $b=Bounds;$scale=[MdcardUi]::GetDpiForWindow($handle)/96.0
 [void][MdcardUi]::SetCursorPos($b.Left+[int]($X*$scale),$b.Top+[int]($Y*$scale))
 Start-Sleep -Milliseconds 180
 [MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 60
 [MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 300
 [void][MdcardUi]::SetWindowPos($handle,[IntPtr]::new(-2),0,0,0,0,0x43)
}
