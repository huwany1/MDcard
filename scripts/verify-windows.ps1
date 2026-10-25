# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$Exe = "$PSScriptRoot\..\target\release\mdcard.exe")
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class MdcardWin {
  [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr w);
  [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr w);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr w, out Rect r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint x, uint y, uint data, UIntPtr extra);
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr w, int x, int y, int width, int height, bool repaint);
}
'@
[void][MdcardWin]::SetThreadDpiAwarenessContext([IntPtr]::new(-4))
$root = Split-Path $PSScriptRoot
$evidence = Join-Path $root ("evidence\windows\" + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Force $evidence | Out-Null
function Keys([string]$Value) { [System.Windows.Forms.SendKeys]::SendWait($Value); Start-Sleep -Milliseconds 350 }
function Paste([string]$Value) { Set-Clipboard -Value $Value; Keys '^v' }
function Shot([string]$Name, $Process) {
  $Process.Refresh()
  $rect = [MdcardWin+Rect]::new()
  [void][MdcardWin]::GetWindowRect($Process.MainWindowHandle, [ref]$rect)
  $bmp = [System.Drawing.Bitmap]::new($rect.Right-$rect.Left, $rect.Bottom-$rect.Top)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($rect.Left,$rect.Top,0,0,$bmp.Size)
  $bmp.Save((Join-Path $evidence $Name),[System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
}
function Click([int]$X, [int]$Y, $Process) {
  $rect = [MdcardWin+Rect]::new()
  [void][MdcardWin]::GetWindowRect($Process.MainWindowHandle,[ref]$rect)
  [void][MdcardWin]::SetCursorPos($rect.Left+$X,$rect.Top+$Y)
  [MdcardWin]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
  [MdcardWin]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
  Start-Sleep -Milliseconds 250
}
function ModalChoice([string]$Choice, $Process) {
  $rect = [MdcardWin+Rect]::new()
  [void][MdcardWin]::GetWindowRect($Process.MainWindowHandle,[ref]$rect)
  $scale = [MdcardWin]::GetDpiForWindow($Process.MainWindowHandle) / 96.0
  $offset = if ($Choice -eq 'Discard') { -88 } else { 5 }
  Click ([int](($rect.Right-$rect.Left)/2 + $offset*$scale)) ([int](($rect.Bottom-$rect.Top)/2 + 55*$scale)) $Process
  Start-Sleep -Milliseconds 500
}
$previousClipboard = [System.Windows.Forms.Clipboard]::GetDataObject()
try {
$exePath = (Resolve-Path -LiteralPath $Exe).Path
$launch = [System.Diagnostics.ProcessStartInfo]::new($exePath)
$launch.UseShellExecute = $false
$launch.Environment['MDCARD_SCREENSHOT'] = (Join-Path $evidence 'welcome-native.png')
$app = [System.Diagnostics.Process]::Start($launch)
Start-Sleep -Seconds 4
$app.Refresh()
if ($app.HasExited -or $app.MainWindowHandle -eq 0) { throw 'Application did not create a window' }
[void][MdcardWin]::MoveWindow($app.MainWindowHandle,50,50,1240,820,$true)
[void][MdcardWin]::SetForegroundWindow($app.MainWindowHandle)
Start-Sleep -Seconds 1
Shot 'welcome.png' $app
Keys '^n'
$text = "# Windows 与中文验证`n`nRust Markdown roundtrip: **你好** 🦀`n`n- [x] Saved through the real UI`n"
Paste $text
Keys '^s'
Start-Sleep -Seconds 1
$path = Join-Path $evidence '中文-roundtrip.md'
Paste $path
Keys '{ENTER}'
Start-Sleep -Seconds 2
if (!(Test-Path -LiteralPath $path)) { throw 'UI save did not produce a file' }
$actual = [IO.File]::ReadAllText($path)
if ($actual -cne $text) { throw 'UTF-8 roundtrip mismatch' }
Shot 'saved.png' $app
Keys '^o'
Start-Sleep -Seconds 1
Paste $path
Keys '{ENTER}'
Start-Sleep -Seconds 1
Keys '^e'
Shot 'read-mode.png' $app
Keys '^e'
Click 400 260 $app
Keys '^{END}'
Paste 'unsaved'
Keys '^n'
Shot 'dirty-new.png' $app
Keys '{ESC}'
Keys '^q'
Shot 'dirty-quit.png' $app
Keys '{ESC}'
Keys '^s'
Start-Sleep -Seconds 1
$saved = [IO.File]::ReadAllText($path)
if (!$saved.Contains('unsaved')) { throw 'Cancel lost unsaved edits' }
Keys '^+s'
Start-Sleep -Seconds 1
Keys '{ESC}'
Click 400 260 $app
Keys '^{END}'
Paste ' save-as-cancel-content'
Keys '^s'
Start-Sleep -Seconds 1
$saved = [IO.File]::ReadAllText($path)
if (!$saved.Contains('save-as-cancel-content')) { throw 'Save As cancellation lost edits' }
Keys '^+s'
Start-Sleep -Seconds 1
$copyPath = Join-Path $evidence 'save-as-copy.md'
Paste $copyPath
Keys '{ENTER}'
Start-Sleep -Seconds 1
if ([IO.File]::ReadAllText($copyPath) -cne $saved) { throw 'Save As content differs' }
Click 400 260 $app
Keys '^{END}'
Paste ' discard-only'
Keys '^n'
Shot 'discard-dialog.png' $app
ModalChoice 'Discard' $app
Keys '^s'
Start-Sleep -Seconds 1
$discardPath = Join-Path $evidence 'after-discard.md'
Paste $discardPath
Keys '{ENTER}'
Start-Sleep -Seconds 1
if ([IO.File]::ReadAllText($discardPath) -ne '') { throw 'Discard did not continue New' }
if ([IO.File]::ReadAllText($copyPath).Contains('discard-only')) { throw 'Discard unexpectedly saved edits' }
Click 400 260 $app
Paste '# Saved through modal'
Keys '^n'
Shot 'save-changes-dialog.png' $app
ModalChoice 'Save' $app
if ([IO.File]::ReadAllText($discardPath) -ne '# Saved through modal') { throw 'Save changes did not persist the old document' }
Keys '^n'
Keys '^z'
Keys '^s'
Start-Sleep -Seconds 1
$emptyPath = Join-Path $evidence 'new-after-undo.md'
Paste $emptyPath
Keys '{ENTER}'
Start-Sleep -Seconds 1
if ([IO.File]::ReadAllText($emptyPath) -ne '') { throw 'Undo leaked old document into new document' }
[void][MdcardWin]::MoveWindow($app.MainWindowHandle,70,70,680,480,$true)
Start-Sleep -Seconds 1
Shot 'compact.png' $app
Keys '^q'
if (!$app.WaitForExit(5000)) { throw 'Clean close failed' }
@{
  platform = [Environment]::OSVersion.VersionString
  architecture = 'x86_64-pc-windows-gnu'
  real_gui = $true
  utf8_chinese_roundtrip = $true
  dirty_new_and_quit_cancel_preserves_edits = $true
  undo_does_not_cross_documents = $true
  save_as_success_and_cancel = $true
  modal_discard_continues_new_without_saving = $true
  modal_save_persists_before_new = $true
  compact_window = '680x480'
  exit_code = $app.ExitCode
  executable_sha256 = (Get-FileHash -LiteralPath $exePath -Algorithm SHA256).Hash
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidence 'result.json') -Encoding UTF8
Write-Output $evidence
} finally {
  if ($null -ne $previousClipboard) { [System.Windows.Forms.Clipboard]::SetDataObject($previousClipboard, $true) }
}
