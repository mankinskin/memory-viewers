# Install script for doc-viewer
# Builds the server and copies it to the agents directory

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path "$ScriptDir\..\.."
$TargetExe = "$ScriptDir\target\release\doc-viewer.exe"
$InstallDir = "$RepoRoot\agents"
$InstalledExe = "$InstallDir\doc-viewer.exe"

Write-Host "Building doc-viewer..." -ForegroundColor Cyan
Push-Location $ScriptDir
try {
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        throw "Build failed"
    }
} finally {
    Pop-Location
}

Write-Host "Copying to agents directory..." -ForegroundColor Cyan
Copy-Item -Path $TargetExe -Destination $InstalledExe -Force

Write-Host "Restarting VS Code MCP server..." -ForegroundColor Cyan
# Send command to VS Code to restart MCP servers
# This uses the VS Code CLI if available
$vscodeCmd = Get-Command code -ErrorAction SilentlyContinue
if ($vscodeCmd) {
    # Unfortunately there's no direct CLI command to restart MCP servers
    # The user will need to manually restart or reload the window
    Write-Host "Please restart the MCP server in VS Code:" -ForegroundColor Yellow
    Write-Host "  1. Open Command Palette (Ctrl+Shift+P)" -ForegroundColor Yellow
    Write-Host "  2. Run 'MCP: List Servers'" -ForegroundColor Yellow  
    Write-Host "  3. Click the refresh/restart button for 'docs'" -ForegroundColor Yellow
} else {
    Write-Host "VS Code CLI not found. Please restart VS Code or the MCP server manually." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Installation complete!" -ForegroundColor Green
Write-Host "Installed to: $InstalledExe" -ForegroundColor Green
