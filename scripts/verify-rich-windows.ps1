# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$Exe="$PSScriptRoot\..\artifacts\rich\MDcard.exe",[switch]$Inspect,[string]$OutputDir=$env:MDCARD_RICH_OUTPUT,[switch]$BomCrLf,[switch]$UsePackagedRenderer)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
$output=if($OutputDir){[IO.Path]::GetFullPath($OutputDir)}else{Join-Path $root 'evidence\rich\windows'}
New-Item -ItemType Directory -Force $output | Out-Null
$settings=Join-Path $output 'settings.json'
$fixture=Join-Path $output 'fixture.md'
$source=@'
# Formula and diagram

Before $x^2$ beside $\alpha+\beta$ after. Cost: $100 and $200.

```mermaid
flowchart LR
A[Source] --> B[Preview]
```

```math
\ce{2H2 + O2 -> 2H2O}
```
'@
if($BomCrLf){$source=$source.Replace("`r`n","`n").Replace("`n","`r`n")}
[IO.File]::WriteAllText($fixture,$source,[Text.UTF8Encoding]::new([bool]$BomCrLf))
$initialHash=(Get-FileHash $fixture).Hash
Remove-Item -LiteralPath (Join-Path $output 'result.json') -ErrorAction SilentlyContinue
@{language='en';appearance='Light';gpu_acceleration=$true;reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content -LiteralPath $settings -Encoding utf8
$previousEnv=@{MDCARD_RENDERER_DIR=$env:MDCARD_RENDERER_DIR;MDCARD_RICH_REPORT=$env:MDCARD_RICH_REPORT};$previousClipboard=$null;try{$previousClipboard=[Windows.Forms.Clipboard]::GetDataObject()}catch{}
if($UsePackagedRenderer){Remove-Item Env:MDCARD_RENDERER_DIR -ErrorAction SilentlyContinue}else{$env:MDCARD_RENDERER_DIR=Join-Path $root 'renderer'}
$env:MDCARD_RICH_REPORT=Join-Path $output 'counters.json'
function Report {for($retry=0;$retry-lt25;$retry++){try{return Get-Content -LiteralPath $env:MDCARD_RICH_REPORT -Raw -ErrorAction Stop|ConvertFrom-Json}catch{Start-Sleep -Milliseconds 20}};throw 'No current native report'}
function Control([string]$Name){$c=(Report).controls|Where-Object name -eq $Name|Select-Object -Last 1;if(!$c){throw "No native control: $Name"};$c.rect}
function ClickName([string]$Name){$r=Control $Name;Click (($r[0]+$r[2])/2) (($r[1]+$r[3])/2)}
function PasteText([string]$Value){Set-Clipboard -Value $Value;Keys '^v';Start-Sleep -Milliseconds 650}
function FocusFloatingEditor {
 for($attempt=0;$attempt-lt8;$attempt++) {
  try {ClickName 'floating-editor'} catch {Start-Sleep -Milliseconds 150;continue}
  Keys '^a'
  $last=@((Report).inputEvents|Where-Object {$_.StartsWith('key:A:')})|Select-Object -Last 1
  if($last-match'focus=floating$'){return}
 }
 throw 'Floating formula editor never received keyboard focus'
}
function Saved {Keys '^s';$value=[IO.File]::ReadAllText($fixture);if($BomCrLf){$bytes=[IO.File]::ReadAllBytes($fixture);if($bytes[0]-ne239-or$bytes[1]-ne187-or$bytes[2]-ne191-or$value.Replace("`r`n",'').Contains("`n")){throw 'BOM/CRLF disk contract changed'}};return $value}
function Export([string]$Format){$target=Join-Path $output "diagram.$($Format.ToLower())";Remove-Item -LiteralPath $target -ErrorAction SilentlyContinue;ClickName "node-2-$Format";Start-Sleep -Milliseconds 600;Keys '%n';Set-Clipboard -Value $target;Keys '^a';Keys '^v';Keys '{ENTER}';Start-Sleep -Milliseconds 700;if(!(Test-Path $target)){throw "Export $Format missing"};return $target}
try {
Launch 'native-renderer' '' $fixture
Start-Sleep -Seconds 8
Shot '01-native-preview'
if($Inspect){Write-Output $script:app.Id;return}
if((Report).counters.completed-ne4 -or (Report).counters.errors-ne0){throw 'Native engines did not finish cleanly'}
Keys '^s'
if((Get-FileHash $fixture).Hash-ne$initialHash){throw 'Preview changed source bytes'}
ClickName 'node-2-Source';$editRect=Control 'node-2-editor';Click ($editRect[0]+30) ($editRect[1]+12);Keys '^a';PasteText "flowchart LR`nA[Alpha] --> B[Beta]"
Shot '01b-source';$edited=Saved
if(!$edited.Contains('A[Alpha] --> B[Beta]')){Shot 'failed-source-edit';throw 'Diagram source did not synchronize'}
Keys '^z';if((Saved)-ne$source){throw 'Node/source undo did not restore exact original'}
Keys '^+z';if((Saved)-ne$edited){throw 'Node/source redo did not restore edit'}
ClickName 'node-2-Split';$sr=Control 'node-2-editor';$cr=Control 'node-2-canvas';$wr=Bounds;$logicalWidth=($wr.Right-$wr.Left)/([MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.);if($sr[2]-gt$cr[0]-or$cr[2]-gt$logicalWidth-10){throw 'Source/split canvas escaped the visible preview'};Shot '02-editable-split'
ClickName 'node-2-Preview'
$beforePan=Saved
$r=Control 'node-2-Preview';$cx=($r[0]+$r[2])/2+80;$cy=$r[3]+75
Click $cx $cy
$b=Bounds;$s=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.
[void][MdcardUi]::SetCursorPos($b.Left+[int]($cx*$s),$b.Top+[int]($cy*$s));[MdcardUi]::mouse_event(0x800,0,0,480,[UIntPtr]::Zero);Start-Sleep -Milliseconds 400
[MdcardUi]::mouse_event(2,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 150
for($step=1;$step-le9;$step++){[void][MdcardUi]::SetCursorPos($b.Left+[int](($cx+5*$step)*$s),$b.Top+[int](($cy+2*$step)*$s));Start-Sleep -Milliseconds 40}
[MdcardUi]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
Shot '03-zoom-pan';$view=(Report).viewStates|Where-Object index -eq 2;if($view.zoom-le1.1 -or ($view.pan[0]-eq0 -and $view.pan[1]-eq0)){throw 'Zoom/pan did not change the actual native canvas state'};if((Saved)-ne$beforePan){throw 'Canvas modified source'}
$svg=Export 'SVG';$png=Export 'PNG'
if(![IO.File]::ReadAllText($svg).StartsWith('<svg')){throw 'SVG export invalid'}
$pngBytes=[IO.File]::ReadAllBytes($png);if($pngBytes[0]-ne137 -or $pngBytes[1]-ne80){throw 'PNG export invalid'}
ClickName 'node-0';FocusFloatingEditor;PasteText 'z^3';Keys '{ESC}'
$withFormula=Saved;if(!$withFormula.Contains('$z^3$')){throw 'Formula source did not synchronize'}
Shot '04-formula-edited'
Keys '^z';if((Saved)-ne$edited){throw 'Formula edit undo affected other source'}
Keys '^+z';if((Saved)-ne$withFormula){throw 'Formula edit redo failed'}
$stable=(Report).counters.submissions;Start-Sleep -Seconds 1;if((Report).counters.submissions-ne$stable){throw 'Unchanged nodes submitted new engine work'}
$themeSource=Saved;Keys '^+p';ClickName 'appearance-Dark';Start-Sleep -Seconds 3;Keys '{ESC}'
if((Get-Content $settings -Raw|ConvertFrom-Json).appearance-ne'Dark'){throw 'Dark theme selection failed'}
Shot '05-dark';if((Saved)-ne$themeSource){throw 'Theme changed Markdown bytes'}
Keys '^+p';ClickName 'appearance-Light';Start-Sleep -Milliseconds 700;Keys '{ESC}'
if((Saved)-ne$themeSource){throw 'Light theme changed Markdown bytes'}
Keys '^q';if(!$script:app.WaitForExit(5000)){throw 'Renderer shutdown exceeded 5s'}
@{runId=[guid]::NewGuid().ToString();date=[DateTimeOffset]::Now.ToString('o');passed=$true;sourceUnchangedByPreview=$true;bomCrLf=[bool]$BomCrLf;packagedRenderer=[bool]$UsePackagedRenderer;editableModes=$true;formulaEdit=$true;undoRedo=$true;zoomPan=$true;svg=(Get-FileHash $svg).Hash;png=(Get-FileHash $png).Hash;exeSha256=(Get-FileHash $Exe).Hash;report=Report}|ConvertTo-Json -Depth 8|Set-Content (Join-Path $output 'result.json') -Encoding utf8

} finally {
 if(!$Inspect -and $script:app -and !$script:app.HasExited){[void]$script:app.CloseMainWindow();if(!$script:app.WaitForExit(2500)){$script:app.Kill($true);$script:app.WaitForExit()}}
 if($previousClipboard){try{[Windows.Forms.Clipboard]::SetDataObject($previousClipboard,$true)}catch{}}
 foreach($name in $previousEnv.Keys){if($null-eq$previousEnv[$name]){Remove-Item "Env:$name" -ErrorAction SilentlyContinue}else{Set-Item "Env:$name" $previousEnv[$name]}}
}
