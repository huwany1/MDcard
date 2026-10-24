# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([Parameter(Mandatory=$true)][string]$Exe)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
$output = Join-Path $root 'evidence\bug-audit\master-review\stale-draft'
New-Item -ItemType Directory -Force $output | Out-Null
Remove-Item -LiteralPath (Join-Path $output 'result.json') -ErrorAction SilentlyContinue
$settings = Join-Path $output 'settings.json'
$fixture = Join-Path $output 'stale.md'
$richPath = Join-Path $output 'rich.json'
$statePath = Join-Path $output 'state.json'
$original = "# Draft conflict`n`nBefore `$x^2`$ after.`n"
$outside = "# Current source`n`nBefore `$outside`$ after.`n"
[IO.File]::WriteAllText($fixture,$original,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText($settings,'{"language":"en","appearance":"Light","reduce_motion":true,"gpu_acceleration":true,"font_size":15}')
$oldRich = $env:MDCARD_RICH_REPORT
$oldState = $env:MDCARD_SEARCH_REPORT
$oldClipboard = [Windows.Forms.Clipboard]::GetDataObject()
$env:MDCARD_RICH_REPORT = $richPath
$env:MDCARD_SEARCH_REPORT = $statePath
function Rich {
    for ($i=0;$i-lt30;$i++) { try { return Get-Content $richPath -Raw | ConvertFrom-Json } catch { Start-Sleep -Milliseconds 20 } }
    throw 'No rich report'
}
function State { Get-Content $statePath -Raw | ConvertFrom-Json }
function RControl([string]$Name) {
    $until=[DateTime]::UtcNow.AddSeconds(10)
    do {
        $c=(Rich).controls|Where-Object name -eq $Name|Select-Object -Last 1
        if($c){Start-Sleep -Milliseconds 250;$c=(Rich).controls|Where-Object name -eq $Name|Select-Object -Last 1;if($c){Click (($c.rect[0]+$c.rect[2])/2) (($c.rect[1]+$c.rect[3])/2);return}}
        Start-Sleep -Milliseconds 50
    }while([DateTime]::UtcNow-lt$until)
    throw "No control $Name"
}
function FocusDraft {
    for ($attempt = 0; $attempt -lt 6; $attempt++) {
        RControl 'floating-editor'
        Keys '^a'
        $lastSelect = @((Rich).inputEvents | Where-Object { $_.StartsWith('key:A:') }) | Select-Object -Last 1
        if ($lastSelect -match 'focus=floating$') { return }
    }
    throw 'Floating source editor did not receive focus'
}
try {
    Launch 'renderer' '' $fixture
    RControl 'node-0'
    RControl 'floating-editor'
    Click 450 430
    Keys '^a'
    Set-Clipboard -Value $outside
    Keys '^v'
    Start-Sleep -Milliseconds 700
    if((State).text-cne$outside){throw 'Source editor did not receive replacement'}
    FocusDraft
    Set-Clipboard -Value 'z^3'
    Keys '^v'
    Start-Sleep -Milliseconds 700
    RControl 'discard-node-drafts'
    Shot '01-discard-confirmation'
    RControl 'cancel-discard-node-drafts'
    FocusDraft
    Keys '^c'
    if([Windows.Forms.Clipboard]::GetText()-cne'z^3'){throw 'Cancel lost node draft'}
    if((State).text-cne$outside){throw 'Cancel changed document source'}
    RControl 'discard-node-drafts'
    RControl 'confirm-discard-node-drafts'
    Start-Sleep -Milliseconds 500
    if((State).text-cne$outside){throw 'Discard changed document source'}
    if([IO.File]::ReadAllText($fixture)-cne$original){throw 'Discard unexpectedly wrote to disk'}
    Keys '^s'
    if([IO.File]::ReadAllText($fixture)-cne$outside){throw 'Cannot save source after discarding stale node draft'}
    Shot '02-current-source-saved'
    Keys '^q'
    if(!$script:app.WaitForExit(5000)){throw 'Application cannot close after explicit discard'}
    @{verification_id=[Guid]::NewGuid().ToString();passed=$true;exe_sha256=(Get-FileHash $Exe).Hash;cancel_keeps_draft=$true;copied_draft='z^3';discard_keeps_source=$true;discard_does_not_write_disk=$true;source_can_save=$true;exit_code=$script:app.ExitCode}|ConvertTo-Json|Set-Content (Join-Path $output 'result.json') -Encoding utf8
} finally {
    if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()}
    if($null-eq$oldRich){Remove-Item Env:MDCARD_RICH_REPORT -ErrorAction SilentlyContinue}else{$env:MDCARD_RICH_REPORT=$oldRich}
    if($null-eq$oldState){Remove-Item Env:MDCARD_SEARCH_REPORT -ErrorAction SilentlyContinue}else{$env:MDCARD_SEARCH_REPORT=$oldState}
    if($oldClipboard){[Windows.Forms.Clipboard]::SetDataObject($oldClipboard,$true)}
}
Get-Content (Join-Path $output 'result.json') -Raw
