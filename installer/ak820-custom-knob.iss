; Installer for ak820-custom-knob (Inno Setup 6).
; Build it with tools\build-release.ps1, which passes the version and puts
; the result in dist\. Installs per user: no admin rights needed.

#ifndef AppVersion
  #error Pass the version: ISCC /DAppVersion=1.2.3 ak820-custom-knob.iss
#endif

#define AppName "AK820 Custom Knob"
#define DaemonExe "ak820-custom-knob.exe"
#define SettingsExe "knob-settings.exe"
#define RunKey "Software\Microsoft\Windows\CurrentVersion\Run"
; Same value name the program itself uses for "Start with Windows".
#define RunValue "ak820-custom-knob"

[Setup]
AppId={{C266CF7C-CAA1-4461-824F-1BC1ED2DAC1C}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Enceladus00
AppPublisherURL=https://github.com/Enceladus00/ak820-custom-knob
AppSupportURL=https://github.com/Enceladus00/ak820-custom-knob/issues
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
LicenseFile=..\LICENSE
SetupIconFile=..\crates\knob-settings\icons\icon.ico
UninstallDisplayIcon={app}\{#DaemonExe}
UninstallDisplayName={#AppName}
OutputDir=..\dist
OutputBaseFilename=ak820-custom-knob-{#AppVersion}-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
SetupMutex=ak820-custom-knob-setup

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
en.AutostartTask=Start {#AppName} when I sign in to Windows
en.DesktopTask=Create a desktop shortcut
en.SettingsShortcut=AK820 Knob Settings
en.LaunchNow=Start {#AppName} now
en.OpenSettings=Open Settings to set up the knob
en.WebViewDownloading=Downloading the Microsoft Edge WebView2 Runtime (needed by the Settings window)…
en.WebViewFailed=The Microsoft Edge WebView2 Runtime could not be installed:%n%n%1%n%nThe knob works without it, but the Settings window will not open until it is installed. You can get it from Microsoft: https://go.microsoft.com/fwlink/p/?LinkId=2124703
en.RemoveSettings=Also delete your knob settings and log files?%n%nChoose No to keep them for a later reinstall.

[Tasks]
Name: "autostart"; Description: "{cm:AutostartTask}"
Name: "desktopicon"; Description: "{cm:DesktopTask}"; Flags: unchecked

[Files]
Source: "..\target\release\{#DaemonExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\{#SettingsExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#DaemonExe}"
Name: "{group}\{cm:SettingsShortcut}"; Filename: "{app}\{#SettingsExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#DaemonExe}"; Tasks: desktopicon

[Registry]
; Written only when the task is ticked. Unticking it on an upgrade leaves a
; choice made later in the tray menu alone.
Root: HKCU; Subkey: "{#RunKey}"; ValueType: string; ValueName: "{#RunValue}"; \
  ValueData: """{app}\{#DaemonExe}"""; Tasks: autostart

[Run]
Filename: "{app}\{#DaemonExe}"; Description: "{cm:LaunchNow}"; \
  Flags: nowait postinstall skipifsilent
Filename: "{app}\{#SettingsExe}"; Description: "{cm:OpenSettings}"; \
  Flags: nowait postinstall skipifsilent

[Code]
const
  WM_CLOSE = $0010;
  WebView2Guid = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';
  WebView2Url = 'https://go.microsoft.com/fwlink/p/?LinkId=2124703';
  TrayClass = 'Ak820KnobTray';

var
  DownloadPage: TDownloadWizardPage;

{ ---------- WebView2 (only the Settings window needs it) }

function WebView2Version(Root: Integer; Key: String): String;
begin
  if not RegQueryStringValue(Root, Key, 'pv', Result) then
    Result := '';
end;

function HasWebView2: Boolean;
var
  V: String;
begin
  V := WebView2Version(HKLM32, 'SOFTWARE\Microsoft\EdgeUpdate\Clients\' + WebView2Guid);
  if (V = '') or (V = '0.0.0.0') then
    V := WebView2Version(HKCU, 'Software\Microsoft\EdgeUpdate\Clients\' + WebView2Guid);
  Result := (V <> '') and (V <> '0.0.0.0');
end;

procedure InitializeWizard;
begin
  DownloadPage := CreateDownloadPage(SetupMessage(msgWizardPreparing),
    CustomMessage('WebViewDownloading'), nil);
end;

procedure InstallWebView2;
var
  Code: Integer;
begin
  DownloadPage.Clear;
  DownloadPage.Add(WebView2Url, 'MicrosoftEdgeWebview2Setup.exe', '');
  DownloadPage.Show;
  try
    try
      DownloadPage.Download;
      if not Exec(ExpandConstant('{tmp}\MicrosoftEdgeWebview2Setup.exe'),
        '/silent /install', '', SW_HIDE, ewWaitUntilTerminated, Code) or (Code <> 0) then
        RaiseException('The WebView2 installer exited with code ' + IntToStr(Code) + '.');
    except
      if not WizardSilent then
        MsgBox(FmtMessage(CustomMessage('WebViewFailed'), [GetExceptionMessage]),
          mbInformation, MB_OK);
    end;
  finally
    DownloadPage.Hide;
  end;
end;

function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if (CurPageID = wpReady) and not HasWebView2 then
    InstallWebView2;
end;

{ ---------- closing a running copy before files are replaced }

procedure StopRunningApp;
var
  Wnd: HWND;
  I, Code: Integer;
begin
  { Ask the tray to quit, so it removes its icon and unhooks cleanly. }
  Wnd := FindWindowByClassName(TrayClass);
  if Wnd <> 0 then
  begin
    PostMessage(Wnd, WM_CLOSE, 0, 0);
    for I := 1 to 30 do
    begin
      if FindWindowByClassName(TrayClass) = 0 then
        Break;
      Sleep(100);
    end;
  end;
  { Still running (or never had a window): end it. }
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM {#DaemonExe}', '', SW_HIDE,
    ewWaitUntilTerminated, Code);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM {#SettingsExe}', '', SW_HIDE,
    ewWaitUntilTerminated, Code);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  StopRunningApp;
  Result := '';
end;

{ ---------- uninstall }

function InitializeUninstall: Boolean;
begin
  StopRunningApp;
  Result := True;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep <> usPostUninstall then
    Exit;
  { Also covers "Start with Windows" turned on from the tray or Settings. }
  RegDeleteValue(HKCU, '{#RunKey}', '{#RunValue}');
  if not UninstallSilent and
    (MsgBox(CustomMessage('RemoveSettings'), mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES) then
  begin
    DelTree(ExpandConstant('{userappdata}\ak820-custom-knob'), True, True, True);
    DelTree(ExpandConstant('{localappdata}\ak820-custom-knob'), True, True, True);
    { The Settings window's WebView2 data. }
    DelTree(ExpandConstant('{localappdata}\io.github.enceladus00.ak820-custom-knob'), True, True, True);
  end;
end;
