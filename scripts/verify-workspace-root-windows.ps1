# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
    [Parameter(Mandatory=$true)][string]$Exe,
    [string]$Stage='fixed'
)
$ErrorActionPreference='Stop'
if([Threading.Thread]::CurrentThread.GetApartmentState() -ne 'STA') {throw 'Run GUI clipboard verification with pwsh -STA -File scripts/verify-workspace-root-windows.ps1'}
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public class WorkspaceUi {
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int Left,Top,Right,Bottom;}
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int cx,int cy,uint flags);
 [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int h2,bool repaint);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint x,uint y,uint d,UIntPtr e);
}
'@
[void][WorkspaceUi]::SetThreadDpiAwarenessContext([IntPtr]::new(-4))
$root=Split-Path $PSScriptRoot
$output=Join-Path $root "evidence\workspace-root-2026-10-22\windows-$Stage"
$fixtures=Join-Path $output 'workspace'
New-Item -ItemType Directory -Force $output,$fixtures,(Join-Path $fixtures '.github\workflows'),(Join-Path $fixtures '中文 资源') | Out-Null
$settings=Join-Path $output 'settings.json'
$uiReport=Join-Path $output 'ui.json'
$main=Join-Path $fixtures 'main.md'
$nested=Join-Path $fixtures '.github\workflows\build.yml'
$sibling=Join-Path $fixtures '中文 资源\notes.md'
$utf8=[Text.UTF8Encoding]::new($false)
[IO.File]::WriteAllText($main,"# Main`n`n原始稿件",$utf8)
[IO.File]::WriteAllText($nested,"name: build`n",$utf8)
[IO.File]::WriteAllText($sibling,"# 相邻目录`n`n中文内容",$utf8)
@{language='en';appearance='Dark';gpu_acceleration=$true;reduce_motion=$true;font_size=15} | ConvertTo-Json | Set-Content -LiteralPath $settings -Encoding utf8
$originalHashes=@{};foreach($file in @($main,$nested,$sibling)){$originalHashes[$file]=(Get-FileHash -LiteralPath $file).Hash}
$record=@{verification_id=[Guid]::NewGuid().ToString();binary_sha256=(Get-FileHash -LiteralPath $Exe).Hash;stage=$Stage;passed=$false}
$priorClipboard=[Windows.Forms.Clipboard]::GetDataObject()
function State {try {Get-Content -Raw -LiteralPath $uiReport -Encoding utf8 | ConvertFrom-Json} catch {$null}}
function Bounds {$rect=[WorkspaceUi+Rect]::new();[void][WorkspaceUi]::GetWindowRect($script:app.MainWindowHandle,[ref]$rect);$rect}
function Shot([string]$Name) {
    Start-Sleep -Milliseconds 400
    $r=Bounds;$bitmap=[Drawing.Bitmap]::new($r.Right-$r.Left,$r.Bottom-$r.Top)
    $graphics=[Drawing.Graphics]::FromImage($bitmap)
    try {$graphics.CopyFromScreen($r.Left,$r.Top,0,0,$bitmap.Size);$bitmap.Save((Join-Path $output "$Name.png"))} finally {$graphics.Dispose();$bitmap.Dispose()}
    State | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $output "$Name.json") -Encoding utf8
}
function Await([scriptblock]$Condition,[string]$Message) {
    $deadline=[DateTime]::UtcNow.AddSeconds(12)
    while([DateTime]::UtcNow -lt $deadline){
        $state=State
        if($state -and (& $Condition $state)){return $state}
        if($script:app.HasExited){throw "App exited: $Message"}
        Start-Sleep -Milliseconds 80
    }
    Shot 'failure';throw "Timed out: $Message"
}
function Click([float]$X,[float]$Y) {
    [void][WorkspaceUi]::SetForegroundWindow($script:app.MainWindowHandle)
    Start-Sleep -Milliseconds 100
    $r=Bounds;$scale=[WorkspaceUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0
    [void][WorkspaceUi]::SetCursorPos($r.Left+[int]($X*$scale),$r.Top+[int]($Y*$scale))
    Start-Sleep -Milliseconds 180
    [WorkspaceUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [WorkspaceUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 250
}
function Keys([string]$Text) {
    [void][WorkspaceUi]::SetForegroundWindow($script:app.MainWindowHandle)
    Start-Sleep -Milliseconds 100
    if([WorkspaceUi]::GetForegroundWindow() -ne $script:app.MainWindowHandle){throw "Foreground lost before $Text"}
    [Windows.Forms.SendKeys]::SendWait($Text);Start-Sleep -Milliseconds 250
}
function Control([string]$Name) {
    $state=Await {param($s) @($s.controls | Where-Object {$_.name -ceq $Name -and $_.enabled}).Count -gt 0} "Control $Name"
    Start-Sleep -Milliseconds 250
    $state=Await {param($s) @($s.controls | Where-Object {$_.name -ceq $Name -and $_.enabled}).Count -gt 0} "Settled control $Name"
    $control=@($state.controls | Where-Object {$_.name -ceq $Name -and $_.enabled})[-1]
    Click (($control.rect[0]+$control.rect[2])/2) (($control.rect[1]+$control.rect[3])/2)
}
function NormalPath([string]$Path) {$Path -replace '^\\\\\?\\',''}
function Workspace([string]$Expected,[int]$Count) {
    Await {param($s) !( $s.workspace_scanning ) -and (NormalPath $s.workspace_root) -eq $Expected -and $s.workspace_files -eq $Count} "Workspace $Expected, $Count files" | Out-Null
}
function Current([string]$Expected) {Await {param($s) (NormalPath $s.document_path) -eq $Expected} "Document $Expected" | Out-Null}
function StartApp([string]$Document='') {
    $launch=[Diagnostics.ProcessStartInfo]::new((Resolve-Path -LiteralPath $Exe).Path)
    $launch.UseShellExecute=$false;$launch.WorkingDirectory=$fixtures
    $launch.Environment['MDCARD_SETTINGS_PATH']=$settings;$launch.Environment['MDCARD_UI_REPORT']=$uiReport
    if($Document){$launch.ArgumentList.Add($Document)}
    $script:app=[Diagnostics.Process]::Start($launch);Start-Sleep -Seconds 3;$script:app.Refresh()
    if($script:app.HasExited -or $script:app.MainWindowHandle -eq 0){throw 'No test window'}
    [void][WorkspaceUi]::MoveWindow($script:app.MainWindowHandle,50,50,1550,1025,$true)
    [void][WorkspaceUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-1),0,0,0,0,0x43)
    [void][WorkspaceUi]::SetForegroundWindow($script:app.MainWindowHandle);Start-Sleep -Milliseconds 250;Click 500 27
    [void][WorkspaceUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-2),0,0,0,0,0x43)
    if([WorkspaceUi]::GetForegroundWindow() -ne $script:app.MainWindowHandle){throw 'Test window did not receive focus'}
}
function StopApp {Keys '^q';if(!$script:app.WaitForExit(5000)){throw 'Test app failed to close'}}
try {
    StartApp $main;Workspace $fixtures 3
    $record.native_dpi=[WorkspaceUi]::GetDpiForWindow($script:app.MainWindowHandle)
    Control 'tree:.github';Control 'tree:.github/workflows'
    Shot '01-before-nested-open'
    Control 'tree:.github/workflows/build.yml';Current $nested;Workspace $fixtures 3
    foreach($name in @('tree:.github','tree:.github/workflows','tree:.github/workflows/build.yml','tree:中文 资源','tree:main.md')){
        if(!@((State).controls | Where-Object name -CEQ $name).Count){throw "Visible tree lost $name"}
    }
    Shot '02-nested-open-preserves-root'
    Control 'Refresh';Workspace $fixtures 3
    Control 'tree:中文 资源';Control 'tree:中文 资源/notes.md';Current $sibling;Workspace $fixtures 3
    Shot '03-sibling-still-accessible'
    Keys '^p';[Windows.Forms.Clipboard]::SetText('build.yml');Keys '^v'
    Start-Sleep -Seconds 1;Keys '{ENTER}';Current $nested;Workspace $fixtures 3
    Shot '04-quick-switch-keeps-workspace'
    Keys '^n';Workspace $fixtures 3
    if((State).document_path){throw 'New document was not untitled'}
    StopApp
    StartApp $nested;Workspace (Split-Path $nested) 1;Current $nested
    Shot '05-command-line-initial-root';StopApp
    StartApp;Workspace $fixtures 3
    Control 'tree:.github';Control 'tree:.github/workflows';Control 'tree:.github/workflows/build.yml'
    Current $nested;Workspace $fixtures 3
    Shot '06-no-argument-start-keeps-workspace';StopApp
    foreach($file in $originalHashes.Keys){if((Get-FileHash -LiteralPath $file).Hash -cne $originalHashes[$file]){throw "Source bytes changed: $file"}}
    $record.scenarios=@('nested-click','expanded-ancestors','sibling-click','refresh','quick-switch','new-document','command-line-start','no-argument-start','source-bytes-unchanged')
    $record.clean_exit=$true;$record.passed=$true
} catch {$record.error=$_.Exception.Message}
finally {
    if($script:app -and !$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()}
    if($priorClipboard){[Windows.Forms.Clipboard]::SetDataObject($priorClipboard,$true)}
    $record | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
}
$record | ConvertTo-Json -Depth 8
if(!$record.passed){exit 1}
