# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
. "$PSScriptRoot\workbench-native-input.ps1"
$output=Join-Path $root 'evidence\workbench-redesign-2026-10-22\outline-final';New-Item -ItemType Directory -Force $output|Out-Null
$settings=Join-Path $output 'settings.json';$report=Join-Path $output 'performance.json';$uiReport=Join-Path $output 'ui.json';$fixture=Join-Path $output 'outline-3000.md'
$text=((0..2999|ForEach-Object {"# Heading $($_.ToString('00000'))`n`n中文全局坐标与目录行检查。`n`n"})-join'')
[IO.File]::WriteAllText($fixture,$text,[Text.UTF8Encoding]::new($false));$original=(Get-FileHash $fixture).Hash
@{language='zh-CN';appearance='Light';reduce_motion=$true;gpu_acceleration=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$oldPerf=$env:MDCARD_PERFORMANCE_REPORT;$oldUi=$env:MDCARD_UI_REPORT;$env:MDCARD_PERFORMANCE_REPORT=$report;$env:MDCARD_UI_REPORT=$uiReport
function State {try{Get-Content -LiteralPath $report -Raw|ConvertFrom-Json}catch{$null}}
function WaitFor([scriptblock]$Predicate,[string]$Message){$until=[DateTime]::UtcNow.AddSeconds(15);while([DateTime]::UtcNow-lt$until){$s=State;if($s-and(&$Predicate $s)){return $s};Start-Sleep -Milliseconds 80};throw $Message}
$result=@{verification_id=[Guid]::NewGuid().ToString();binary_sha256=(Get-FileHash $Exe).Hash;headings=3000;passed=$false}
try{
 Launch 'renderer' '' $fixture
 $ui=Get-Content -LiteralPath $uiReport -Raw|ConvertFrom-Json;$r=@($ui.controls|Where-Object name -eq 'navigation:Outline')|Select-Object -Last 1;Click (($r.rect[0]+$r.rect[2])/2) (($r.rect[1]+$r.rect[3])/2)
 $first=WaitFor {param($s)$s.toc_controls.Count-gt5-and$s.toc_scrollbar} 'Outline did not publish real visible rows'
 $rect=$first.toc_scrollbar;$b=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.0;$x=$b.Left+[int](($rect[0]+$rect[2])*.5*$scale)
 [void][MdcardUi]::SetCursorPos($x,$b.Top+[int](($rect[1]+4)*$scale));Start-Sleep -Milliseconds 180;[MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 60
 foreach($step in 1..20){[void][MdcardUi]::SetCursorPos($x,$b.Top+[int](($rect[1]+4+($rect[3]-$rect[1]-8)*$step/20)*$scale));Start-Sleep -Milliseconds 30};[MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
 $last=WaitFor {param($s)@($s.toc_controls|Where-Object title -eq 'Heading 02999').Count-gt0} 'Final heading is not reachable by actual outline scroll'
 $rows=@($last.toc_controls|Sort-Object {$_.rect[1]});for($i=1;$i-lt$rows.Count;$i++){if($rows[$i-1].rect[3]-gt$rows[$i].rect[1]+0.5){throw 'Virtual outline rows overlap'}}
 [void][MdcardUi]::SetCursorPos(5,5);Shot '01-last-outline-headings'
 $target=@($last.toc_controls|Where-Object title -eq 'Heading 02999')[0]
 Click (($target.rect[0]+$target.rect[2])/2) (($target.rect[1]+$target.rect[3])/2)
 $expected=([Text.Encoding]::UTF8.GetString([Text.Encoding]::UTF8.GetBytes($text),0,[int]$target.byte)).Length
 $selected=WaitFor {param($s)$s.selection[0]-eq$expected-and$s.selection[1]-eq$expected} 'Outline jump lost the full-document source coordinate'
 [void][MdcardUi]::SetCursorPos(5,5);Shot '02-final-global-source-position'
 if((Get-FileHash $fixture).Hash-ne$original){throw 'Outline view/jump changed source bytes'}
 $result.expected_source_character=$expected;$result.parser_byte=[int]$target.byte;$result.selection=$selected.selection;$result.last_visible_rows=$rows;$result.source_unchanged=$true
 Keys '^q';$result.clean_exit=$script:app.WaitForExit(5000);$result.passed=$result.clean_exit
}catch{$result.error=$_.Exception.Message}
finally{if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()};$env:MDCARD_PERFORMANCE_REPORT=$oldPerf;$env:MDCARD_UI_REPORT=$oldUi;$result|ConvertTo-Json -Depth 7|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8}
if(!$result.passed){throw 'Large outline native validation failed'}
