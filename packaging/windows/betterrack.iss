; Inno Setup script. Build with:  iscc /DAppVersion=1.0.0 packaging\windows\betterrack.iss
; Expects the staged folder from packaging/stage.sh at dist\BetterRack.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{B1F09FF5-3ABF-4898-B554-060C89B8FF22}
AppName=Better Rack
AppVersion={#AppVersion}
AppPublisher=AminPerez
DefaultDirName={autopf}\BetterRack
DefaultGroupName=Better Rack
UninstallDisplayName=Better Rack {#AppVersion}
UninstallDisplayIcon={app}\betterrack-gpui.exe
SetupIconFile=..\icon.ico
OutputDir=..\..\dist
OutputBaseFilename=BetterRack-Setup-{#AppVersion}
Compression=lzma2/max
SolidCompression=yes
; Per-user install by default, like the NSIS one (`perMachine: false`); the dialog lets users pick.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern
DisableProgramGroupPage=yes
LicenseFile=..\..\dist\BetterRack\NOTICE.txt

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Shortcuts:"

[Files]
Source: "..\..\dist\BetterRack\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

[Icons]
Name: "{group}\Better Rack"; Filename: "{app}\betterrack-gpui.exe"
Name: "{autodesktop}\Better Rack"; Filename: "{app}\betterrack-gpui.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\betterrack-gpui.exe"; Description: "Launch Better Rack"; Flags: nowait postinstall skipifsilent

[Code]
// Same question as build/installer.nsh: offer to delete the app data (library database, settings,
// logs). Comic files are never touched. Skipped for silent uninstalls and for updates.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if (CurUninstallStep = usPostUninstall) and not UninstallSilent then
    if MsgBox('Do you also want to delete all Better Rack application data (library database, settings and logs)?' + #13#10 + #13#10 + 'Your comic files will not be deleted.', mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
    begin
      DelTree(ExpandConstant('{userappdata}\BetterRack'), True, True, True);
      DelTree(ExpandConstant('{localappdata}\BetterRack'), True, True, True);
    end;
end;
