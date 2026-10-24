# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
    [Parameter(Mandatory=$true)][string]$Exe,
    [ValidateSet('before','after')][string]$Stage='before',
    [string]$Language='zh-CN',
    [ValidateSet('Light','Dark','System')][string]$Appearance='Light',
    [string]$RunName=''
)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
$output=Join-Path $root "evidence\workbench-redesign-2026-10-22\$Stage\$Language-$Appearance"
if($RunName){$output=Join-Path $root "evidence\workbench-redesign-2026-10-22\$Stage-$RunName\$Language-$Appearance"}
$scratch=Join-Path $root 'artifacts\workbench-redesign-2026-10-22'
New-Item -ItemType Directory -Force -Path $output,$scratch | Out-Null
$settings=Join-Path $output 'settings.json'
$fixtureDir=Join-Path $scratch 'visual-fixture'
New-Item -ItemType Directory -Force $fixtureDir | Out-Null
$fixture=Join-Path $fixtureDir 'ui-fixture.md'
$text=@'
# 给写作留一点空间

MDcard · 中文技术写作 / A calm place for your ideas.

## 从一个清晰的想法开始

好的工具让人专注于内容。中文、English 和 2026 数字混排应该清晰舒适，窗口变窄时，正文仍然自然换行。

> 记录灵感，整理思路，让每一次修改都更从容。

### 今日写作清单

- [x] 整理项目笔记与参考材料
- [ ] 完成第一版技术说明
- [ ] 校对代码示例与关键结论

| 章节 | 状态 | 下一步 |
| :--- | :---: | ---: |
| 背景与目标 | 已完成 | 复核 |
| 设计与实现 | 进行中 | 补充示例 |

```rust
fn main() {
    println!("你好，MDcard!");
}
```

## 留下可持续的思考

让正文成为焦点，用恰当的留白和节奏，支撑长时间的阅读与写作。
'@
[IO.File]::WriteAllText($fixture,$text,[Text.UTF8Encoding]::new($false))
if($RunName){
    New-Item -ItemType Directory -Force -Path (Join-Path $fixtureDir 'notes\architecture'),(Join-Path $fixtureDir 'drafts'),(Join-Path $fixtureDir 'empty')|Out-Null
    [IO.File]::WriteAllText((Join-Path $fixtureDir 'notes\architecture\一份需要完整路径提示的很长设计文档名称-workbench-design-notes.md'),"# Architecture`n`n副文档与中文说明。`n",[Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $fixtureDir 'drafts\daily.md'),"# Daily`n`nWrite a little every day.`n",[Text.UTF8Encoding]::new($false))
}
@{language=$Language;appearance=$Appearance;reduce_motion=$true;font_size=15;gpu_acceleration=$true} | ConvertTo-Json | Set-Content -LiteralPath $settings -Encoding utf8
$before=(Get-FileHash -LiteralPath $fixture).Hash
$captures=[Collections.Generic.List[string]]::new()
$oldUiReport=$env:MDCARD_UI_REPORT
$env:MDCARD_UI_REPORT=Join-Path $output 'ui-state.json'
function Capture([string]$Name){ [void][MdcardUi]::SetCursorPos(5,5);Start-Sleep -Milliseconds 450;Shot $Name; $captures.Add($Name); if(Test-Path -LiteralPath $env:MDCARD_UI_REPORT){Copy-Item -LiteralPath $env:MDCARD_UI_REPORT -Destination (Join-Path $output "$Name.json")} }
function ClickReported([string]$Name){$state=Get-Content -LiteralPath $env:MDCARD_UI_REPORT -Raw|ConvertFrom-Json;$r=@($state.controls|Where-Object name -eq $Name)|Select-Object -Last 1;if(!$r){throw "No actual control $Name"};$b=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0;$x=($r.rect[0]+$r.rect[2])/2;$y=($r.rect[1]+$r.rect[3])/2;[void][MdcardUi]::SetCursorPos($b.Left+[int]($x*$scale),$b.Top+[int]($y*$scale));Start-Sleep -Milliseconds 180;[MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 60;[MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 300}
try {
    Launch 'renderer' '' $fixture
    $dpi=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)
    $scale=$dpi/96.0
    [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,40,30,[int](1280*$scale),[int](800*$scale),$true)
    Capture 'split-1280x800'
    Keys '^e'; Capture 'read-1280x800'
    Keys '^e'; Capture 'write-1280x800'
    if($Stage-eq'after'){
        ClickReported 'navigation:Outline';Capture 'navigation-outline'
        ClickReported 'navigation:Recent';Capture 'navigation-recent'
        ClickReported 'navigation:Files'
        if($RunName){ClickReported 'tree:notes';ClickReported 'tree:notes/architecture';Capture 'navigation-long-path'}
    }
    Keys '^+p'; Capture 'preferences'; Keys '{ESC}'
    Keys '^f'; Capture 'find-replace'; Keys '{ESC}'
    Keys '^p'; Capture 'quick-switcher'; Keys '{ESC}'
    Keys '^+e'; Capture 'encoding'; Keys '{ESC}'
    Keys '^+t'; Capture 'structure'; Keys '^+t'
    [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,40,30,[int](800*$scale),[int](600*$scale),$true)
    Capture 'compact-800x600'
    Keys '^e'; Keys '^e'; Capture 'compact-read-800x600'
    Keys '^+p'; Capture 'compact-preferences'; Keys '{ESC}'
    Keys '^f'; Capture 'compact-find-replace'; Keys '{ESC}'
    Keys '^p'; Capture 'compact-quick-switcher'; Keys '{ESC}'
    Keys '^+e'; Capture 'compact-encoding'; Keys '{ESC}'
    Keys '^+t'; Capture 'compact-structure'; Keys '^+t'
    [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,40,30,[int](1920*$scale),[int](1080*$scale),$true)
    Capture 'read-1920x1080'
    Keys '^%e'; Capture 'export-wide'
    [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,40,30,[int](800*$scale),[int](600*$scale),$true)
    Capture 'export-compact'
    Keys '^q'
    if(!$script:app.WaitForExit(7000)){throw 'Test app did not exit cleanly'}
    if((Get-FileHash -LiteralPath $fixture).Hash-ne$before){throw 'View operations changed fixture bytes'}
    @{verification_id=[Guid]::NewGuid().ToString();stage=$Stage;language=$Language;appearance=$Appearance;dpi=$dpi;logical_sizes=@('1280x800','800x600','1920x1080');exe=(Resolve-Path -LiteralPath $Exe).Path;sha256=(Get-FileHash -LiteralPath $Exe).Hash;fixture_sha256=$before;source_unchanged=$true;captures=$captures.ToArray();process_id=$script:app.Id;real_windows_gui=$true} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
} finally {
    if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()}
    $env:MDCARD_UI_REPORT=$oldUiReport
}
Write-Output $output
