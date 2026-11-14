<#
.SYNOPSIS  Builds the installer and portable ZIP, then writes SHA256SUMS.txt into dist\release.
           Contents: CSVRescue-Setup-<version>.exe, CSVRescue-<version>-win64-portable.zip, SHA256SUMS.txt
#>
param([string]$Version)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

if (-not $Version) {
    $cargoToml = Get-Content (Join-Path $root 'Cargo.toml') -Raw
    if ($cargoToml -match '(?m)^version\s*=\s*"([^"]+)"') {
        $Version = $Matches[1]
    } else {
        throw 'Could not determine workspace version from Cargo.toml'
    }
}

$stage = Join-Path $root 'dist\stage'
$release = Join-Path $root 'dist\release'

& (Join-Path $PSScriptRoot 'build-installer.ps1') -Version $Version | Out-Null

if (Test-Path $release) { Remove-Item $release -Recurse -Force }
New-Item -ItemType Directory -Force $release | Out-Null

Copy-Item (Join-Path $root "dist\installer\CSVRescue-Setup-$Version.exe") $release -Force

$portableZip = Join-Path $release "CSVRescue-$Version-win64-portable.zip"
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $portableZip -CompressionLevel Optimal -Force

# LF endings so `sha256sum -c` works on Linux and macOS too
$sums = Get-ChildItem $release -File | Where-Object Name -ne 'SHA256SUMS.txt' | Sort-Object Name | ForEach-Object {
    "$((Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower())  $($_.Name)"
}
[IO.File]::WriteAllText((Join-Path $release 'SHA256SUMS.txt'), ($sums -join "`n") + "`n")

Get-ChildItem $release | Select-Object Name, @{n='MB';e={[math]::Round($_.Length / 1MB, 2)}}, Length
