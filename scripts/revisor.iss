; Inno Setup script for the Windows installer. Build: ISCC.exe /DVersion=1.2.3 scripts\revisor.iss
; Installs for the current user only, so installs and in-app updates need no administrator.

[Setup]
AppId={{6C2E8D41-6F0B-4C55-9F7A-2E1B5D3A9C10}
AppName=Revisor
AppVersion={#Version}
AppPublisher=Viacheslav Solomin
AppPublisherURL=https://github.com/Selnapenek/revisor
DefaultDirName={autopf}\Revisor
DisableProgramGroupPage=yes
DisableDirPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\target\dist
OutputBaseFilename=Revisor-{#Version}-x64-setup
SetupIconFile=..\brand\icon.ico
UninstallDisplayIcon={app}\Revisor.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; The in-app updater renames the running Revisor.exe first, so no file is in use.
CloseApplications=no

[Files]
Source: "..\target\release\revisor.exe"; DestDir: "{app}"; DestName: "Revisor.exe"; Flags: ignoreversion
Source: "..\LICENSE.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Revisor"; Filename: "{app}\Revisor.exe"

[Run]
Filename: "{app}\Revisor.exe"; Description: "Start Revisor"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: files; Name: "{app}\Revisor.exe.old"
