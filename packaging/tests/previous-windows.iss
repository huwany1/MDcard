; SPDX-License-Identifier: AGPL-3.0-only
; QA ONLY: wrap the SHA-verified genuine public 0.1.0 portable application in
; a synthetic installer, to exercise this new installer's future upgrade path.
; This is not an assertion that a historical 0.1.0 installer was ever released.
#ifndef PreviousBinary
 #error PreviousBinary is required
#endif
#ifndef FixtureOutput
 #error FixtureOutput is required
#endif
[Setup]
AppId={{6F259284-88A3-44E2-A5A8-7F110F5F7045}
AppName=MDcard
AppVersion=0.1.0
DefaultDirName={localappdata}\Programs\MDcard
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#FixtureOutput}
OutputBaseFilename=QA-ONLY-synthetic-previous-installer
Compression=zip
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\MDcard.exe
[Files]
Source: "{#PreviousBinary}"; DestDir: "{app}"; DestName: "MDcard.exe"; Flags: ignoreversion
