#define Root SourcePath + "\..\..\.."
#ifndef Package
#define Package Root + "\dist\Orvyn-portable"
#endif
#ifndef OutputSuffix
#define OutputSuffix ""
#endif
#ifndef AppVersion
  #define AppVersion "0.7.0"
#endif
[Setup]
AppId=OrvynPortable
AppName=Orvyn
AppVersion={#AppVersion}
AppVerName=Orvyn {#AppVersion}
VersionInfoDescription=Orvyn installer {#AppVersion}{#OutputSuffix}
DefaultDirName={src}\Orvyn
DefaultGroupName=Orvyn
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Uninstallable=yes
CreateUninstallRegKey=IsTaskSelected('registerapp')
UninstallDisplayIcon={app}\Orvyn.exe
UninstallDisplayName=Orvyn
DisableProgramGroupPage=yes
UsePreviousAppDir=no
UsePreviousTasks=no
UsePreviousLanguage=no
DisableWelcomePage=no
DisableDirPage=no
LanguageDetectionMethod=none
ShowLanguageDialog=no
OutputDir={#Root}\dist
OutputBaseFilename=Orvyn-Setup-{#AppVersion}{#OutputSuffix}
SetupIconFile={#Root}\apps\manager\assets\orvyn.ico
Compression=zip/1
SolidCompression=no
WizardStyle=modern
MinVersion=10.0
CloseApplications=yes
RestartApplications=no
InfoBeforeFile={#Package}\README.txt
LicenseFile={#Root}\LICENSE
[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
[Tasks]
Name: "startmenu"; Description: "Create a Start menu shortcut"; GroupDescription: "Shortcuts:"
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked
Name: "registerapp"; Description: "List Orvyn in Windows installed apps"; GroupDescription: "Windows integration:"; Flags: unchecked
[Files]
Source: "{#Package}\Orvyn.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Package}\WebView2Loader.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Package}\START.bat"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Package}\README.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Package}\apps\web-ui\dist\*"; DestDir: "{app}\apps\web-ui\dist"; Flags: ignoreversion recursesubdirs
Source: "{#Package}\runtime\llamafile\llamafile-0.10.6.exe"; DestDir: "{app}\runtime\llamafile"; Flags: ignoreversion
; Never overwrite or uninstall a user's model or settings.
#if OutputSuffix == ""
Source: "{#Package}\models\desktop-active\llama32-1b.gguf"; DestDir: "{app}\models\desktop-active"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "{#Package}\models\desktop-active\llama32-1b.gguf.verified.json"; DestDir: "{app}\models\desktop-active"; Flags: onlyifdoesntexist uninsneveruninstall
#endif
Source: "{#Package}\config\settings.json"; DestDir: "{app}\config"; DestName: "settings.json"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "{#Package}\config\custom-models.json"; DestDir: "{app}\config"; DestName: "custom-models.json"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "{#Package}\config\model-catalog.json"; DestDir: "{app}\config"; DestName: "model-catalog.json"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "{#Package}\README.txt"; DestDir: "{app}"; DestName: "INSTALL.txt"; Flags: ignoreversion
Source: "{#Root}\apps\manager\packaging\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion uninsneveruninstall
[Dirs]
Name: "{app}\data"; Flags: uninsneveruninstall
Name: "{app}\data\webview-profile"; Flags: uninsneveruninstall
Name: "{app}\backups"; Flags: uninsneveruninstall
Name: "{app}\logs\current"; Flags: uninsneveruninstall
[Icons]
Name: "{userprograms}\Orvyn\Orvyn"; Filename: "{app}\Orvyn.exe"; WorkingDir: "{app}"; Tasks: startmenu
Name: "{userprograms}\Orvyn\Uninstall Orvyn"; Filename: "{uninstallexe}"; Tasks: startmenu
Name: "{userdesktop}\Orvyn"; Filename: "{app}\Orvyn.exe"; WorkingDir: "{app}"; Tasks: desktopicon
[Run]
Filename: "{app}\Orvyn.exe"; Description: "Open Orvyn"; WorkingDir: "{app}"; Flags: nowait postinstall skipifsilent

[Code]
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Target: String;
begin
  Result := '';
  Target := ExpandFileName(ExpandConstant('{app}'));
  if (CompareText(AddBackslash(Target), AddBackslash(ExtractFileDrive(Target))) = 0) or
     DirExists(Target + '\.git') or DirExists(Target + '\apps\manager\src') then
    Result := 'Choose a separate Orvyn folder, not a drive root or a source-code repository.';
end;
