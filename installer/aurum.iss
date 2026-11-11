#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppName=Aurum
AppVersion={#AppVersion}
AppPublisher=duchuynguyen93
AppPublisherURL=https://github.com/duchuynguyen93/aurum
DefaultDirName={autopf}\Aurum
DefaultGroupName=Aurum
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
SetupIconFile=..\crates\aurum-app\assets\icon\aurum.ico
UninstallDisplayIcon={app}\aurum.exe
OutputDir=..\dist
OutputBaseFilename=aurum-{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\aurum.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Aurum"; Filename: "{app}\aurum.exe"
Name: "{autodesktop}\Aurum"; Filename: "{app}\aurum.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\aurum.exe"; Description: "{cm:LaunchProgram,Aurum}"; Flags: nowait postinstall skipifsilent
