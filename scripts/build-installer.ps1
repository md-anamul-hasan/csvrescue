<#
.SYNOPSIS  Builds native Rust + Tauri CSVRescue and packages it as dist\installer\CSVRescue-Setup-<version>.exe (needs Inno Setup 6).
#>
param(
    [string]$Version,
    [switch]$PublishOnly,  # stop after filling dist\stage (CI signs CSVRescue.exe before packaging)
    [switch]$SkipPublish   # package an existing dist\stage as-is
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

if (-not $Version) {
    $cargoToml = Get-Content (Join-Path $root 'Cargo.toml') -Raw
    if ($cargoToml -match '(?m)^version\s*=\s*"([^"]+)"') {
        $Version = $Matches[1]
    }
    else {
        throw 'Could not determine workspace version from Cargo.toml'
    }
}

$stage = Join-Path $root 'dist\stage'
$uiDir = Join-Path $root 'src-ui'

if ($PublishOnly -and $SkipPublish) { throw '-PublishOnly and -SkipPublish cannot be combined' }

$iscc = @(
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $iscc -and -not $PublishOnly) {
    throw 'Inno Setup 6 not found. Install it with: winget install JRSoftware.InnoSetup'
}

if (-not $SkipPublish) {
    Push-Location $uiDir
    try {
        if (-not (Test-Path (Join-Path $uiDir 'node_modules'))) {
            npm ci --no-audit --no-fund
            if ($LASTEXITCODE -ne 0) { throw 'npm ci failed in src-ui' }
        }
        npm run build
        if ($LASTEXITCODE -ne 0) { throw 'npm run build failed in src-ui' }
    }
    finally {
        Pop-Location
    }

    Push-Location $root
    try {
        cargo build --release -p csvrescue-app --bin CSVRescue
        if ($LASTEXITCODE -ne 0) { throw 'cargo build --release failed' }
    }
    finally {
        Pop-Location
    }

    if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
    New-Item -ItemType Directory -Force $stage | Out-Null

    $exe = Join-Path $root 'target\release\CSVRescue.exe'
    if (-not (Test-Path $exe)) { throw "Built binary missing at $exe" }
    Copy-Item $exe (Join-Path $stage 'CSVRescue.exe') -Force

    foreach ($docName in @('LICENSE', 'THIRD_PARTY_NOTICES.md', 'README.md')) {
        $docPath = Join-Path $root $docName
        if (Test-Path $docPath) {
            Copy-Item $docPath (Join-Path $stage $docName) -Force
        }
    }
}

if ($PublishOnly) { return }

& $iscc "/DAppVersion=$Version" (Join-Path $root 'installer\csvrescue.iss')
if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed' }

Get-Item (Join-Path $root "dist\installer\CSVRescue-Setup-$Version.exe") |
Select-Object Name, @{n = 'MB'; e = { [math]::Round($_.Length / 1MB, 2) } }, Length
