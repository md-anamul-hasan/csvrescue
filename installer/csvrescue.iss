; CSVRescue installer. Build with scripts\build-installer.ps1 (publishes a self-contained build to dist\stage first).
#define AppName "CSVRescue"
#define AppFullName "CSVRescue - CSV Encoding, Delimiter and Excel Cleaner"
#ifndef AppVersion
  #define AppVersion "1.1.0"
#endif
#define StageDir "..\dist\stage"

[Setup]
AppId={{7B9F7875-EC40-5640-B663-3D4A2CFD66AE}
AppName={#AppFullName}
AppVersion={#AppVersion}
AppPublisher=Md Anamul Hasan
AppPublisherURL=https://github.com/md-anamul-hasan/csvrescue
AppSupportURL=https://github.com/md-anamul-hasan/csvrescue/issues
DefaultDirName={autopf}\CSVRescue
DefaultGroupName=CSVRescue
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\CSVRescue.exe
UninstallDisplayName={#AppFullName}
SetupIconFile=..\Assets/icon.ico
LicenseFile=..\LICENSE
OutputDir=..\dist\installer
OutputBaseFilename=CSVRescue-Setup-{#AppVersion}
Compression=lzma2/ultra
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Shortcuts:"

[Files]
Source: "{#StageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs; Excludes: "*.pdb"

[Icons]
Name: "{autoprograms}\CSVRescue"; Filename: "{app}\CSVRescue.exe"
Name: "{autodesktop}\CSVRescue"; Filename: "{app}\CSVRescue.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\CSVRescue.exe"; Description: "Launch CSVRescue"; Flags: nowait postinstall skipifsilent
