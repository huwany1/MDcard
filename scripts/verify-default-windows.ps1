# SPDX-License-Identifier: AGPL-3.0-only
# Native default startup/Open/New/Save As, standard user directories, running-app guard.
param([string]$Exe,[string]$Installer,[string]$OutputDirectory)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
. "$PSScriptRoot\recovery-json.ps1"
$output=[IO.Path]::GetFullPath($OutputDirectory);New-Item -ItemType Directory -Force $output|Out-Null
$profileRoot=Join-Path $output 'user-profile';$roaming=Join-Path $profileRoot 'Roaming';$local=Join-Path $profileRoot 'Local'
New-Item -ItemType Directory -Force (Join-Path $roaming 'MDcard'),$local|Out-Null
@{language='en';reduce_motion=$true;font_size=15}|ConvertTo-Json|Set-Content (Join-Path $roaming 'MDcard\settings.json') -Encoding utf8
$ui=Join-Path $output 'ui.json';$env:MDCARD_UI_REPORT=$ui
function Report {ReadJsonStable $ui @('controls','document_path')}
function Control([string]$Name){
 for($attempt=0;$attempt-lt6;$attempt++){
  $c=(Report).controls|Where-Object name -eq $Name|Select-Object -First 1;if(!$c){throw "Missing control $Name"}
  $l=[Math]::Max($c.rect[0],$c.clip[0]);$t=[Math]::Max($c.rect[1],$c.clip[1]);$r=[Math]::Min($c.rect[2],$c.clip[2]);$b=[Math]::Min($c.rect[3],$c.clip[3])
  $x=if($Name-eq'main-source-editor'){$l+50}else{($l+$r)/2};$y=if($Name-eq'main-source-editor'){$t+14}else{($t+$b)/2}
  $bounds=Bounds;$scale=[MdcardUi]::GetDpiForWindow($script:app.MainWindowHandle)/96.;[void][MdcardUi]::SetCursorPos($bounds.Left+[int]($x*$scale),$bounds.Top+[int]($y*$scale));Start-Sleep -Milliseconds 200
  $next=(Report).controls|Where-Object name -eq $Name|Select-Object -First 1
  if(($next.rect-join',')-ne($c.rect-join',')){continue};Click $x $y;return
 };throw "Control geometry did not stabilize: $Name"
}
function WaitFor([scriptblock]$Condition,[string]$Reason){$end=[DateTime]::UtcNow.AddSeconds(30);while([DateTime]::UtcNow-lt$end){if(&$Condition){return};Start-Sleep -Milliseconds 80};throw $Reason}
function Paste([string]$Text){Set-Clipboard -Value $Text;Start-Sleep -Milliseconds 100;Keys '^v'}
function NativePath([string]$Path){Start-Sleep -Milliseconds 700;Keys '%n';Paste $Path;Keys '{ENTER}';Start-Sleep -Milliseconds 300}
$clipboard=[Windows.Forms.Clipboard]::GetDataObject();$result=@{}
try {
 $launch=[Diagnostics.ProcessStartInfo]::new((Resolve-Path $Exe).Path);$launch.UseShellExecute=$false;$launch.WorkingDirectory=$output
 foreach($name in @($launch.Environment.Keys)){if($name.StartsWith('MDCARD_')-or$name-in@('NODE_OPTIONS','NODE_PATH','JAVA_HOME','CARGO_HOME','RUSTUP_HOME')){[void]$launch.Environment.Remove($name)}}
 $launch.Environment['MDCARD_UI_REPORT']=$ui;$launch.Environment['APPDATA']=$roaming;$launch.Environment['LOCALAPPDATA']=$local;$launch.Environment['PATH']=Join-Path $env:SystemRoot 'System32'
 $script:app=[Diagnostics.Process]::Start($launch);Start-Sleep -Seconds 3;$script:app.Refresh();if($script:app.MainWindowHandle-eq0){throw 'Default entry did not create a window'}
 [void][MdcardUi]::MoveWindow($script:app.MainWindowHandle,50,50,1550,1025,$true)
 [void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-1),0,0,0,0,0x43);[void][MdcardUi]::SetForegroundWindow($script:app.MainWindowHandle);Click 500 27;[void][MdcardUi]::SetWindowPos($script:app.MainWindowHandle,[IntPtr]::new(-2),0,0,0,0,0x43)
 Keys '^n';Control 'Write';Control 'main-source-editor';Keys '^a';Paste "# 新建文档`n`n中文 and English."
 $new=Join-Path $output '新建 中文 空格.md';Keys '^s';NativePath $new;WaitFor {(Test-Path -LiteralPath $new)-and!(Report).dirty} 'Default New/Save dialog failed';Shot '01-new-save-native'
 $open=Join-Path $output '打开 中文 空格.md';[IO.File]::WriteAllText($open,"# Open native`n`nOpen 中文.",[Text.UTF8Encoding]::new($false))
 Keys '^o';NativePath $open;WaitFor {(Report).document_path-like'*打开 中文 空格.md'} 'Default Open dialog did not choose selected file';Shot '02-open-native'
 $copy=Join-Path $output '另存为 中文 空格.md';Keys '^+s';NativePath $copy;WaitFor {Test-Path -LiteralPath $copy} 'Native Save As did not create copy';if([IO.File]::ReadAllText($copy)-cne[IO.File]::ReadAllText($open)){throw 'Save As changed text'};Shot '03-save-as-native'
 Keys '^+p';Control 'Auto save';Keys '{ESC}'
 $settings=Join-Path $roaming 'MDcard\settings.json';WaitFor {(Get-Content -Raw $settings|ConvertFrom-Json).autosave} 'Standard APPDATA preferences not persisted'
 Keys '^+p';Control 'Auto save';Keys '{ESC}';Control 'Write';Control 'main-source-editor';Keys '^{END}';Paste "`nProtected unsaved 中文."
 WaitFor {(Report).recovery.main.current_version-and(Report).recovery.main.committed_at-gt0} 'Default LOCALAPPDATA recovery did not checkpoint'
 $recovery=Join-Path $local 'MDcard\recovery';if(!(Test-Path -LiteralPath $recovery)){throw 'Recovery used wrong default user directory'}
 $hash=(Get-FileHash $Exe).Hash;$clock=[Diagnostics.Stopwatch]::StartNew();$setup=Start-Process -FilePath $Installer -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART',('/DIR="'+(Split-Path $Exe)+'"'),('/LOG="'+(Join-Path $output 'running-install-refused.log')+'"')) -WindowStyle Hidden -Wait -PassThru
 if($setup.ExitCode-eq0-or$script:app.HasExited-or(Get-FileHash $Exe).Hash-ne$hash-or!(Report).dirty){throw 'Installer did not safely refuse a running editor'}
 $result=@{defaultStartNoDocumentArgument=$true;nativeNewSave=$true;nativeOpen=$true;nativeSaveAs=$true;standardAppData=$settings;standardLocalAppDataRecovery=$recovery;developmentVariablesRemoved=$true;path='System32 only';cwd=$output;noProjectDirectory=$true;runningInstallRefused=$true;runningInstallerExitCode=$setup.ExitCode;appPreservedDirtyBuffer=$true;binarySHA256=$hash;hostHasDeveloperTools=$true;unicodeClipboardInput=$true;imeComposition='separate test required'}
 Keys '^s';WaitFor {!(Report).dirty} 'Final test document save failed';Keys '^q';if(!$script:app.WaitForExit(10000)){throw 'Default test did not exit'}
 $result|ConvertTo-Json -Depth 6|Set-Content (Join-Path $output 'result.json') -Encoding utf8
} finally {if($script:app-and!$script:app.HasExited){$script:app.Kill();$script:app.WaitForExit()};if($null-ne$clipboard){[Windows.Forms.Clipboard]::SetDataObject($clipboard,$true)}}
