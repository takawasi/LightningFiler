# LightningFiler Release Build Script
$ErrorActionPreference = "Stop"

Write-Host "=== LightningFiler Release Build ===" -ForegroundColor Cyan

# Environment check
Write-Host "Checking toolchains..."
$targetInstalled = rustup target list --installed | Select-String "x86_64-pc-windows-msvc"
if (-not $targetInstalled) {
    Write-Host "Error: Target x86_64-pc-windows-msvc is not installed." -ForegroundColor Red
    Write-Host "Run: rustup target add x86_64-pc-windows-msvc" -ForegroundColor Yellow
    exit 1
}
Write-Host "Target x86_64-pc-windows-msvc is installed." -ForegroundColor Green

# 64bit main app build
Write-Host "`n[1/2] Building 64-bit main app..." -ForegroundColor Yellow
cargo build --release --target x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw "64-bit build failed" }

# Create distribution package
Write-Host "`n[2/2] Creating distribution package..." -ForegroundColor Yellow
$installDir = "install"
if (Test-Path $installDir) { Remove-Item $installDir -Recurse -Force }
New-Item -ItemType Directory -Path $installDir | Out-Null

# Copy binary
Copy-Item "target\x86_64-pc-windows-msvc\release\lightning_filer.exe" $installDir

# Copy config files
if (Test-Path "config\default_keymap.toml") {
    Copy-Item "config\default_keymap.toml" $installDir
}
if (Test-Path "config\default_config.toml") {
    Copy-Item "config\default_config.toml" "$installDir\config.toml"
}

# Copy license/readme
if (Test-Path "LICENSE") { Copy-Item "LICENSE" $installDir }
if (Test-Path "README.md") { Copy-Item "README.md" $installDir }

# Create plugins directory
New-Item -ItemType Directory -Path "$installDir\plugins" -Force | Out-Null

Write-Host "`n=== Build Complete ===" -ForegroundColor Green
$mainSize = (Get-Item "$installDir\lightning_filer.exe").Length / 1MB
Write-Host "Main binary: $([math]::Round($mainSize, 2)) MB"
Write-Host "Output: $installDir\"
