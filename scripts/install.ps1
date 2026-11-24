# ==============================================================================
# MiniCI Portable Installer for Windows (`scripts/install.ps1`)
# Usage (no admin needed):
#   irm https://raw.githubusercontent.com/minici-dev/minici-cost-action/main/scripts/install.ps1 | iex
# Env overrides: MINICI_VERSION (default latest), MINICI_INSTALL_DIR
# ==============================================================================
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Repo = 'minici-dev/minici-cost-action'
$Version = if ($env:MINICI_VERSION) { $env:MINICI_VERSION } else { 'latest' }
$InstallDir = if ($env:MINICI_INSTALL_DIR) { $env:MINICI_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'minici\bin' }

switch ($env:PROCESSOR_ARCHITECTURE) {
  'AMD64' { $Arch = 'x86_64' }
  'ARM64' { $Arch = 'arm64' }
  default { throw "Unsupported architecture: $env:PROCESSOR_ARCHITECTURE" }
}

$Asset = "minici-windows-$Arch.exe"
if ($Version -eq 'latest') {
  $BaseUrl = "https://github.com/$Repo/releases/latest/download"
} else {
  $BaseUrl = "https://github.com/$Repo/releases/download/$Version"
}

$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("minici-" + [System.Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $TmpDir | Out-Null
try {
  $Binary = Join-Path $TmpDir 'minici.exe'
  $Sums = Join-Path $TmpDir 'SHA256SUMS.txt'
  Write-Host "Downloading $Asset from $Repo ($Version)..."
  Invoke-WebRequest -UseBasicParsing -Uri "$BaseUrl/$Asset" -OutFile $Binary
  Invoke-WebRequest -UseBasicParsing -Uri "$BaseUrl/SHA256SUMS.txt" -OutFile $Sums

  $Line = Get-Content $Sums | Where-Object { $_ -match "\s\*?$([regex]::Escape($Asset))$" } | Select-Object -First 1
  if (-not $Line) { throw "Release asset $Asset missing from SHA256SUMS.txt. Refusing unverified binary." }
  $Expected = ($Line -split '\s+')[0].ToLower()
  $Actual = (Get-FileHash -Algorithm SHA256 -Path $Binary).Hash.ToLower()
  if ($Expected -ne $Actual) { throw "Checksum mismatch for $Asset! Expected $Expected, got $Actual" }
  Write-Host "Verified SHA-256 checksum ($($Actual.Substring(0, 16))...)"

  New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
  $Target = Join-Path $InstallDir 'minici.exe'
  Move-Item -Force -Path $Binary -Destination $Target
} finally {
  Remove-Item -Recurse -Force -Path $TmpDir -ErrorAction SilentlyContinue
}

# Add the install dir to the user PATH (no admin); skip if already present.
$UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$Entries = @()
if ($UserPath) { $Entries = $UserPath -split ';' | Where-Object { $_ } }
if ($Entries -notcontains $InstallDir) {
  [Environment]::SetEnvironmentVariable('Path', (($Entries + $InstallDir) -join ';'), 'User')
  Write-Host "Added $InstallDir to your user PATH (open a new terminal to pick it up)."
}
$env:Path = "$InstallDir;$env:Path"

$Installed = & (Join-Path $InstallDir 'minici.exe') --version
if ($LASTEXITCODE -ne 0) { throw "minici failed to run after install." }
Write-Host "Installed $Installed -> $InstallDir\minici.exe"
