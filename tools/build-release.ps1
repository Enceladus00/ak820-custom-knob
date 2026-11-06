# Builds a release: the installer and a portable zip, both in dist\.
#
#   powershell -ExecutionPolicy Bypass -File tools\build-release.ps1
#
# Needs Rust (cargo) and Inno Setup 6 (winget install JRSoftware.InnoSetup).
# -SkipBuild packages the exes already in target\release.

param([switch]$SkipBuild)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

# The workspace version, from [workspace.package] in Cargo.toml.
$match = Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
if (-not $match) { throw 'No version found in Cargo.toml' }
$version = $match.Matches[0].Groups[1].Value
Write-Host "ak820-custom-knob $version"

if (-not $SkipBuild) {
    cargo build --release --locked -p ak820-custom-knob -p knob-settings
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
}

$exes = 'ak820-custom-knob.exe', 'knob-settings.exe' | ForEach-Object { Join-Path 'target\release' $_ }
foreach ($exe in $exes) {
    if (-not (Test-Path $exe)) { throw "$exe is missing; build first" }
}

$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null

# Portable zip: unpack anywhere and run ak820-custom-knob.exe.
$staging = Join-Path ([IO.Path]::GetTempPath()) "ak820-custom-knob-$version"
Remove-Item -Recurse -Force $staging -ErrorAction SilentlyContinue
New-Item -ItemType Directory $staging | Out-Null
Copy-Item $exes $staging
Copy-Item LICENSE (Join-Path $staging 'LICENSE.txt')
$zip = Join-Path $dist "ak820-custom-knob-$version-portable.zip"
Remove-Item $zip -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $staging '*') -DestinationPath $zip
Remove-Item -Recurse -Force $staging
Write-Host "Portable: $zip"

# Installer.
$iscc = (Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source
if (-not $iscc) {
    $iscc = @(
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $iscc) { throw 'Inno Setup 6 not found: winget install JRSoftware.InnoSetup' }
& $iscc /Q "/DAppVersion=$version" installer\ak820-custom-knob.iss
if ($LASTEXITCODE -ne 0) { throw 'Inno Setup failed' }
Write-Host "Installer: $(Join-Path $dist "ak820-custom-knob-$version-setup.exe")"
