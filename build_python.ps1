# build_python.ps1
# Builds the high-performance PyO3 extension and produces rusty_clicker.pyd

Write-Host "Building rusty_clicker release Python extension..." -ForegroundColor Cyan
cargo build --release -p rusty-clicker-py

if ($LASTEXITCODE -eq 0) {
    Copy-Item target\release\rusty_clicker.dll rusty_clicker.pyd -Force
    Write-Host "`nSUCCESS: Created rusty_clicker.pyd (size: $((Get-Item rusty_clicker.pyd).Length) bytes)" -ForegroundColor Green
    Write-Host "You can now drop rusty_clicker.pyd next to any Python script or package it into a Python .exe!" -ForegroundColor Yellow
} else {
    Write-Host "`nBuild failed!" -ForegroundColor Red
}

