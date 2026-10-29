; SPDX-License-Identifier: AGPL-3.0-only
#ifndef PayloadRoot
  #error PayloadRoot must name a freshly staged complete Windows payload
#endif
#ifndef ReleaseOutput
  #define ReleaseOutput "..\..\dist\0.1.5"
#endif
[Setup]
AppId={{6F259284-88A3-44E2-A5A8-7F110F5F7045}
AppName=MDcard
AppVersion=0.1.5
AppVerName=MDcard 0.1.5
AppPublisher=MDcard contributors
AppPublisherURL=https://github.com/huwany1/MDcard
DefaultDirName={localappdata}\Programs\MDcard
DefaultGroupName=MDcard
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.22000
OutputDir={#ReleaseOutput}
OutputBaseFilename=MDcard-0.1.5-windows-x64-setup
SetupIconFile=..\..\assets\branding\mdcard-icon\ico\mdcard-color.ico
UninstallDisplayIcon={app}\MDcard.exe
; GPL/third-party texts remain in the complete payload. Running the editor does
; not require a separate click-through agreement. Use the OS language directly.
ShowLanguageDialog=no
DisableDirPage=no
DisableReadyPage=yes
DisableFinishedPage=yes
Compression=lzma2/normal
LZMADictionarySize=65536
SolidCompression=yes
WizardStyle=modern
CloseApplications=no
RestartApplications=no
ChangesAssociations=yes
SetupLogging=yes
Uninstallable=yes
[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesesimp"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"
Name: "chinesetrad"; MessagesFile: "compiler:Languages\ChineseTraditional.isl"
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
[Messages]
english.ButtonInstall=&Install and launch MDcard
chinesesimp.ButtonInstall=安装并启动 MDcard(&I)
chinesetrad.ButtonInstall=安裝並啟動 MDcard(&I)
japanese.ButtonInstall=インストールして起動(&I)
korean.ButtonInstall=설치하고 실행(&I)
russian.ButtonInstall=Установить и запустить(&I)
[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut / 创建桌面快捷方式"; Flags: unchecked
Name: "association"; Description: "Add MDcard to Open with for Markdown / 添加 Markdown 打开方式"; Flags: unchecked
[Files]
Source: "rollback.ps1"; Flags: dontcopy
Source: "{#PayloadRoot}\manifest.json"; DestName: "rollback-new-manifest.json"; Flags: dontcopy
Source: "{#PayloadRoot}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
[Icons]
Name: "{userprograms}\MDcard"; Filename: "{app}\MDcard.exe"; WorkingDir: "{app}"
Name: "{userdesktop}\MDcard"; Filename: "{app}\MDcard.exe"; WorkingDir: "{app}"; Tasks: desktopicon
[Registry]
Root: HKCU; Subkey: "Software\Classes\MDcard.Markdown"; ValueType: string; ValueData: "Markdown document"; Flags: uninsdeletekey; Tasks: association
Root: HKCU; Subkey: "Software\Classes\MDcard.Markdown\DefaultIcon"; ValueType: string; ValueData: "{app}\MDcard.exe,0"; Tasks: association
Root: HKCU; Subkey: "Software\Classes\MDcard.Markdown\shell\open\command"; ValueType: string; ValueData: """{app}\MDcard.exe"" ""%1"""; Tasks: association
Root: HKCU; Subkey: "Software\Classes\.md\OpenWithProgids"; ValueType: none; ValueName: "MDcard.Markdown"; Flags: uninsdeletevalue; Tasks: association
Root: HKCU; Subkey: "Software\Classes\.markdown\OpenWithProgids"; ValueType: none; ValueName: "MDcard.Markdown"; Flags: uninsdeletevalue; Tasks: association
[Run]
Filename: "{app}\MDcard.exe"; Flags: nowait skipifsilent
[Code]
var UpgradeBackupReady, InstallSucceeded: Boolean;
function RunRollback(const Operation: String): Boolean;
var Code: Integer; Args: String;
begin
  Code := -1;
  Args := '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + ExpandConstant('{tmp}\rollback.ps1') + '" -Operation ' + Operation +
    ' -InstallRoot "' + ExpandConstant('{app}') + '" -BackupRoot "' + ExpandConstant('{app}.mdcard-rollback') +
    '" -NewManifest "' + ExpandConstant('{tmp}\rollback-new-manifest.json') + '"';
  Result := ExecAndLogOutputWithNativeSysDir(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'), Args, '', SW_HIDE, ewWaitUntilTerminated, Code, nil);
  Result := Result and (Code = 0);
  Log('Upgrade backup operation ' + Operation + ': ' + IntToStr(Code));
end;
function ProductRunning(): Boolean;
var Locator, Service, Processes: Variant;
begin
  Result := False;
  try
    Locator := CreateOleObject('WbemScripting.SWbemLocator');
    Service := Locator.ConnectServer('', 'root\CIMV2');
    Processes := Service.ExecQuery('SELECT ProcessId FROM Win32_Process WHERE Name=''MDcard.exe'' OR Name=''mdcard.exe''');
    Result := Processes.Count > 0;
  except
    { Fail closed: do not update files if process checks are unavailable. }
    Result := True;
  end;
end;
function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if ProductRunning() then
    Result := 'Please save your documents and close MDcard before installing. / 请保存文档并关闭 MDcard 后重试。'
  else if DirExists(ExpandConstant('{app}.mdcard-rollback')) and not UpgradeBackupReady then
    Result := 'A previous interrupted upgrade backup exists. No files were overwritten. Follow RECOVERY.txt in: ' + ExpandConstant('{app}.mdcard-rollback')
  else if FileExists(ExpandConstant('{app}\MDcard.exe')) and not UpgradeBackupReady then begin
    ExtractTemporaryFile('rollback.ps1');
    ExtractTemporaryFile('rollback-new-manifest.json');
    if RunRollback('Prepare') then UpgradeBackupReady := True
    else Result := 'Could not finish a verified previous-version backup. No application files were overwritten. Check free disk space or the existing recovery folder: ' + ExpandConstant('{app}.mdcard-rollback');
  end;
end;
procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssDone then begin
    InstallSucceeded := True;
    if UpgradeBackupReady and not RunRollback('Commit') then
      MsgBox('The previous-version backup was retained. Check the installation log and RECOVERY.txt in: ' + ExpandConstant('{app}.mdcard-rollback'), mbError, MB_OK);
  end;
end;
procedure DeinitializeSetup();
begin
  if UpgradeBackupReady and not InstallSucceeded then begin
    if not RunRollback('Restore') then
      MsgBox('Previous-version backup is preserved. Save and close MDcard and follow RECOVERY.txt in: ' + ExpandConstant('{app}.mdcard-rollback'), mbError, MB_OK);
  end;
end;
function InitializeUninstall(): Boolean;
begin
  Result := not ProductRunning();
  if not Result then MsgBox('Please save your documents and close MDcard before uninstalling. / 请保存文档并关闭 MDcard 后卸载。', mbError, MB_OK);
end;
