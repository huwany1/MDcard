# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe,[string]$EvidenceName='states-final')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
$output=Join-Path $root "evidence\workbench-redesign-2026-10-22\reports\$EvidenceName"
New-Item -ItemType Directory -Force $output|Out-Null
$settings=Join-Path $output 'settings.json';$rich=Join-Path $output 'rich.json';$uiReport=Join-Path $output 'ui.json'
$fixture=Join-Path $root 'artifacts\workbench-redesign-2026-10-22\visual-fixture\ui-fixture.md'
@{language='en';appearance='Light';accent=0;reduce_motion=$true;font_size=15;gpu_acceleration=$true}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$savedEnv=@{};foreach($name in @('MDCARD_UI_REPORT','MDCARD_RICH_REPORT')){$savedEnv[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
$env:MDCARD_UI_REPORT=$uiReport;$env:MDCARD_RICH_REPORT=$rich
$sourceBefore=(Get-FileHash -LiteralPath $fixture).Hash
$checks=[Collections.Generic.List[object]]::new()
function Json([string]$Path){for($n=0;$n-lt20;$n++){try{return Get-Content -Raw -LiteralPath $Path|ConvertFrom-Json}catch{Start-Sleep -Milliseconds 25}};throw "No JSON state $Path"}
function Named([string]$Name,[switch]$UseRich){$state=Json $(if($UseRich){$rich}else{$uiReport});$c=@($state.controls|Where-Object name -eq $Name);if(!$c.Count){throw "No real control $Name"};$r=$c[0].rect;$b=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.;[void][MdcardUi]::SetCursorPos($b.Left+[int](($r[0]+$r[2])*.5*$scale),$b.Top+[int](($r[1]+$r[3])*.5*$scale));Start-Sleep -Milliseconds 60;[MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 60;[MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 400}
try{
    Launch 'renderer' '' $fixture
    $dpi=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)
    foreach($appearance in @('Light','Dark','System')){
        Keys '^+p';Named "appearance-$appearance" -UseRich
        foreach($accent in 0..5){
            Named (@('Blue','Purple','Pink','Orange','Green','Graphite')[$accent])
            $p=Json $settings;if($p.appearance-ne$appearance-or$p.accent-ne$accent){throw "Preference click did not persist $appearance/$accent"}
            Shot "preferences-$appearance-$accent"
            $checks.Add(@{appearance=$appearance;accent=$accent;native_click=$true;persisted=$true})
        }
        Keys '{ESC}';Shot "workspace-$appearance"
    }
    Keys '^+p';Named 'Reduce motion';if((Json $settings).reduce_motion){throw 'Reduce motion did not switch off'};Named 'Reduce motion';if(!(Json $settings).reduce_motion){throw 'Reduce motion did not switch back on'};Keys '{ESC}'
    [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,40,30,1000,750,$true);Shot 'compact-disabled-split'
    $s=Json $uiReport;$split=@($s.controls|Where-Object name -eq 'Split')[0]
    if($split.enabled-or$s.can_split-or$s.mode-ne'Write'){throw 'Compact Split is not disabled consistently'}
    Keys '^q';if(!$script:app.WaitForExit(5000)){throw 'State test did not exit cleanly'}
    if((Get-FileHash -LiteralPath $fixture).Hash-ne$sourceBefore){throw 'Preference/view operations modified the source'}
    @{verification_id=[Guid]::NewGuid().ToString();binary_sha256=(Get-FileHash -LiteralPath $Exe).Hash;native_matrix=$checks.ToArray();reduce_motion_roundtrip=$true;compact_split_disabled=$true;source_unchanged=$true;system_scope='System selected against current OS theme only; no forced Windows theme changes';dpi=$dpi;passed=$true}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
}finally{
    if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()}
    foreach($name in $savedEnv.Keys){[Environment]::SetEnvironmentVariable($name,$savedEnv[$name],'Process')}
}
