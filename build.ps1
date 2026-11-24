$ErrorActionPreference = "Stop"
$env:Path = "$HOME\.cargo\bin;" + $env:Path
Set-Location $PSScriptRoot

Write-Host "1/2 Running Rust unit tests & /// doctests..." -ForegroundColor Cyan
cargo test

Write-Host "2/2 Building EdgeBridge v1.0.0-rc.1 native Windows binary..." -ForegroundColor Cyan
cargo build --release

$Bin = Join-Path $PSScriptRoot "target\release\edgebridge.exe"
$Rel = Join-Path $PSScriptRoot "releases\edgebridge-v1.0.0-rc.1-windows-x86_64.exe"
Copy-Item -Force $Bin $Rel
Write-Host "BUILD COMPLETE: $Bin -> $Rel" -ForegroundColor Green
