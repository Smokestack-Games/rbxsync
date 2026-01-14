# RbxSync Installer for Windows
# Run with: irm https://raw.githubusercontent.com/devmarissa/rbxsync/master/scripts/install.ps1 | iex

$ErrorActionPreference = "Stop"

# ASCII Art Header
Write-Host ""
Write-Host "  ____  _          ____                   " -ForegroundColor Cyan
Write-Host " |  _ \| |____  __/ ___| _   _ _ __   ___ " -ForegroundColor Cyan
Write-Host " | |_) | '_ \ \/ /\___ \| | | | '_ \ / __|" -ForegroundColor Cyan
Write-Host " |  _ <| |_) >  <  ___) | |_| | | | | (__ " -ForegroundColor Cyan
Write-Host " |_| \_\_.__/_/\_\|____/ \__, |_| |_|\___|" -ForegroundColor Cyan
Write-Host "                         |___/            " -ForegroundColor Cyan
Write-Host ""
Write-Host "RbxSync Installer for Windows" -ForegroundColor White
Write-Host ""

# Check if running on Windows
if ($env:OS -ne "Windows_NT") {
    Write-Host "Error: This installer is for Windows only." -ForegroundColor Red
    Write-Host "For macOS, use:"
    Write-Host "  curl -fsSL https://raw.githubusercontent.com/devmarissa/rbxsync/master/scripts/install.sh | sh"
    exit 1
}

# Get latest version from GitHub
Write-Host "Fetching latest version..." -ForegroundColor Blue
try {
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/devmarissa/rbxsync/releases/latest"
    $VERSION = $release.tag_name
} catch {
    Write-Host "Error: Could not fetch latest version from GitHub." -ForegroundColor Red
    Write-Host "Please check your internet connection or try again later."
    exit 1
}

Write-Host "Latest version: $VERSION" -ForegroundColor Green
Write-Host ""

# Download URL
$BINARY = "rbxsync-windows-x86_64.exe"
$DOWNLOAD_URL = "https://github.com/devmarissa/rbxsync/releases/download/$VERSION/$BINARY"

# Install directory
$INSTALL_DIR = "$env:LOCALAPPDATA\rbxsync"

# Check for existing installations and clean them up
Write-Host "Checking for existing installations..." -ForegroundColor Blue
$existingPaths = @()
try {
    $whereOutput = & where.exe rbxsync 2>$null
    if ($whereOutput) {
        $existingPaths = $whereOutput -split "`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne "" }
    }
} catch {
    # where.exe not found or no existing installations
}

if ($existingPaths.Count -gt 0) {
    Write-Host ""
    Write-Host "Found existing RbxSync installation(s):" -ForegroundColor Yellow
    foreach ($path in $existingPaths) {
        Write-Host "  $path" -ForegroundColor Yellow
    }
    Write-Host ""

    foreach ($oldPath in $existingPaths) {
        # Don't remove if it's our target install location
        if ($oldPath -ne "$INSTALL_DIR\rbxsync.exe") {
            Write-Host "Removing old version: $oldPath" -ForegroundColor Blue
            try {
                Remove-Item $oldPath -Force -ErrorAction SilentlyContinue
                Write-Host "  Removed!" -ForegroundColor Green
            } catch {
                Write-Host "  Could not remove (may need admin). Delete manually: $oldPath" -ForegroundColor Yellow
            }
        }
    }
    Write-Host ""
}

# Create install directory if it doesn't exist
if (-not (Test-Path $INSTALL_DIR)) {
    Write-Host "Creating install directory: $INSTALL_DIR" -ForegroundColor Blue
    New-Item -ItemType Directory -Force -Path $INSTALL_DIR | Out-Null
}

# Download binary
Write-Host "Downloading $BINARY..." -ForegroundColor Blue
$DEST = "$INSTALL_DIR\rbxsync.exe"

try {
    Invoke-WebRequest -Uri $DOWNLOAD_URL -OutFile $DEST -UseBasicParsing
} catch {
    Write-Host "Error: Failed to download binary." -ForegroundColor Red
    Write-Host "URL: $DOWNLOAD_URL"
    exit 1
}

# Clean up PATH - remove old rbxsync directories and add new one at the START
$UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
$pathParts = $UserPath -split ";" | Where-Object { $_ -ne "" }

# Remove any old rbxsync-related paths (except our new one)
$cleanedPaths = $pathParts | Where-Object {
    $_ -ne $INSTALL_DIR -and
    $_ -notlike "*\.rbxsync\bin*" -and
    -not (Test-Path "$_\rbxsync.exe" -ErrorAction SilentlyContinue)
}

# Prepend our install dir (so it takes priority)
if ($cleanedPaths -notcontains $INSTALL_DIR) {
    Write-Host "Adding to PATH (with priority)..." -ForegroundColor Blue
    $newPath = @($INSTALL_DIR) + $cleanedPaths -join ";"
    [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
    # Also update current session
    $env:PATH = "$INSTALL_DIR;$env:PATH"
} else {
    Write-Host "Already in PATH." -ForegroundColor Blue
}

# Verify installation
Write-Host ""
Write-Host "RbxSync installed successfully!" -ForegroundColor Green
Write-Host ""

# Try to run version command
try {
    & $DEST version
} catch {
    Write-Host "Installed to: $DEST" -ForegroundColor White
}

Write-Host ""
Write-Host "Get started:" -ForegroundColor White
Write-Host "  rbxsync init      - Initialize a new project" -ForegroundColor Gray
Write-Host "  rbxsync serve     - Start the sync server" -ForegroundColor Gray
Write-Host "  rbxsync --help    - Show all commands" -ForegroundColor Gray
Write-Host ""
Write-Host "Documentation: https://rbxsync.dev" -ForegroundColor Blue
Write-Host ""
Write-Host "NOTE: Restart your terminal for PATH changes to take effect." -ForegroundColor Yellow
