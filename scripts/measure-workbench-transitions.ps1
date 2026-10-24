# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe,[ValidateSet('before','after')][string]$Stage)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
$output=Join-Path $root "evidence\workbench-redesign-2026-10-22\reports\transitions-$Stage"
New-Item -ItemType Directory -Force $output|Out-Null
$settings=Join-Path $output 'settings.json'
$report=Join-Path $output 'performance.json'
$rich=Join-Path $output 'rich.json'
$fixture=Join-Path $output 'transitions.md'
$text=(1..3000|ForEach-Object{"# Heading $_`n`n中文写作与界面响应 · reference paragraph $_`n"})-join"`n"
[IO.File]::WriteAllText($fixture,$text,[Text.UTF8Encoding]::new($false))
@{language='en';appearance='Light';reduce_motion=$true;font_size=15;gpu_acceleration=$true}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$envNames=@('MDCARD_PERFORMANCE_REPORT','MDCARD_PERFORMANCE_GPU_FINISH','MDCARD_RICH_REPORT')
$oldEnv=@{};foreach($name in $envNames){$oldEnv[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
$env:MDCARD_PERFORMANCE_REPORT=$report;$env:MDCARD_PERFORMANCE_GPU_FINISH='1';$env:MDCARD_RICH_REPORT=$rich
$records=[Collections.Generic.List[object]]::new()
$memory=[Collections.Generic.List[long]]::new()
function State {try{Get-Content -Raw -LiteralPath $report|ConvertFrom-Json}catch{$null}}
function WaitState([scriptblock]$Condition,[string]$Label){$until=[DateTime]::UtcNow.AddSeconds(8);while([DateTime]::UtcNow-lt$until){$s=State;if($s-and(&$Condition $s)){return $s};Start-Sleep -Milliseconds 1};throw "Timeout: $Label"}
function NativeClick([double]$X,[double]$Y){$b=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.;[void][MdcardUi]::SetCursorPos($b.Left+[int]($X*$scale),$b.Top+[int]($Y*$scale));[MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 60;[MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero)}
function PrepareClick([double]$X,[double]$Y){$b=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.;[void][MdcardUi]::SetCursorPos($b.Left+[int]($X*$scale),$b.Top+[int]($Y*$scale));Start-Sleep -Milliseconds 60}
function MeasureTransition([string]$Name,[scriptblock]$Action,[scriptblock]$Confirmed){
    if([MdcardUi]::GetForegroundWindow()-ne$script:app.MainWindowHandle){throw 'Test lost foreground'}
    $before=State;$frames=[long]$before.paint.frames
    $geometryBefore="$($before.source_rect-join ',')|$($before.preview_rect-join ',')"
    $watch=[Diagnostics.Stopwatch]::StartNew();&$Action
    $s=WaitState {param($s)
        $geometryNow="$($s.source_rect-join ',')|$($s.preview_rect-join ',')"
        $layoutChanged=$Name-eq'theme'-or$geometryNow-cne$geometryBefore
        $s.paint.frames-gt$frames-and$layoutChanged-and(&$Confirmed $s)
    } $Name
    $watch.Stop();$records.Add(@{action=$Name;milliseconds=$watch.Elapsed.TotalMilliseconds;frames_before=$frames;frames_after=[long]$s.paint.frames;update_cpu_samples=@($s.update_cpu_ms|Select-Object -Last 3)})
    $script:app.Refresh();$memory.Add($script:app.WorkingSet64)
    Start-Sleep -Milliseconds 80
}
function Stats($Rows){$v=@($Rows|ForEach-Object{[double]$_.milliseconds}|Sort-Object);@{samples=$v.Count;p50_ms=$v[[int][Math]::Floor($v.Count*.5)];p95_ms=$v[[Math]::Min($v.Count-1,[int][Math]::Floor($v.Count*.95))];max_ms=$v[-1]}}
$result=@{stage=$Stage;verification_id=[Guid]::NewGuid().ToString();binary_sha256=(Get-FileHash -LiteralPath $Exe).Hash;passed=$false;measurement='Native operation to observing a newly completed GPU frame report with changed source/preview layout rectangles for mode/resize, or persisted selected appearance for theme. Includes OS queue, report publication, SendWait overhead, PowerShell JSON parsing and polling. Theme pointer settles 60 ms before the timer; measured click includes a 60 ms physical-button hold, identical before/after, and must not be called application-only response time. This is observed layout-update latency, not final layout stability, swapchain presentation or scanout. No fixed SendKeys helper delay is inside timing.'}
try{
    Launch 'renderer' '' $fixture
    WaitState {param($s)$s.paint.frames-gt2} 'Initial complete frame'|Out-Null
    foreach($round in 1..12){
        MeasureTransition 'mode' {[Windows.Forms.SendKeys]::SendWait('^e')} {$true}
        $width=if($round%2){1700}else{1550}
        MeasureTransition 'resize' {[void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,40,30,$width,1000,$true)} {$true}
    }
    [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,40,30,1550,1000,$true)
    Keys '^+p'
    foreach($round in 1..12){
        $appearance=if($round%2){'Dark'}else{'Light'}
        $controls=Get-Content -Raw -LiteralPath $rich|ConvertFrom-Json
        $r=@($controls.controls|Where-Object name -eq "appearance-$appearance")[0].rect
        if(!$r){throw 'Actual appearance control not reported'}
        PrepareClick (($r[0]+$r[2])*.5) (($r[1]+$r[3])*.5)
        MeasureTransition 'theme' {NativeClick (($r[0]+$r[2])*.5) (($r[1]+$r[3])*.5)} {try{(Get-Content -Raw -LiteralPath $settings|ConvertFrom-Json).appearance-eq$appearance}catch{$false}}
    }
    Keys '{ESC}'
    $s=State;$s|ConvertTo-Json -Depth 14|Set-Content -LiteralPath (Join-Path $output 'final-state.json') -Encoding utf8
    $result.actions=@{};foreach($name in @('mode','resize','theme')){$result.actions[$name]=Stats @($records|Where-Object action -eq $name)}
    $result.samples=$records.ToArray();$result.working_set_samples_bytes=$memory.ToArray();$result.source_unchanged=([IO.File]::ReadAllText($fixture)-ceq$text)
    Keys '^q';$result.clean_exit=$script:app.WaitForExit(5000);$result.passed=$result.source_unchanged-and$result.clean_exit
}catch{$result.error=$_.Exception.Message;throw}
finally{
    $result.samples=$records.ToArray();$result.working_set_samples_bytes=$memory.ToArray()
    if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()}
    foreach($name in $envNames){[Environment]::SetEnvironmentVariable($name,$oldEnv[$name],'Process')}
    $result|ConvertTo-Json -Depth 12|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
}
$result.actions|ConvertTo-Json -Depth 4
