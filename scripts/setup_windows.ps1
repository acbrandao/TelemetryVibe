# One-shot Windows setup: installs the MSVC build tools, Rust and FFmpeg with winget,
# then builds dist\TelemetryVibe\TelemetryVibe-<x64|arm64>.exe. Safe to re-run; anything already installed is skipped.
#
#   powershell -ExecutionPolicy Bypass -File scripts\setup_windows.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\setup_windows.ps1 -Target aarch64-pc-windows-msvc
param([string]$Target = "x86_64-pc-windows-msvc")
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    Write-Error "winget not found. Install 'App Installer' from the Microsoft Store, then re-run."
}

function Install-Package($Id, $Extra = @()) {
    winget list --id $Id -e --accept-source-agreements | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "==> Installing $Id..."
        winget install --id $Id -e --accept-source-agreements --accept-package-agreements @Extra
    } else {
        Write-Host "==> $Id already installed"
    }
}

# 1. C++ build tools (the MSVC linker and Windows SDK that Rust needs).
Install-Package "Microsoft.VisualStudio.2022.BuildTools" @(
    "--override", "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended")
# 2. Rust.
Install-Package "Rustlang.Rustup"
# 3. FFmpeg (the app runs ffmpeg/ffprobe for decoding and encoding).
Install-Package "Gyan.FFmpeg"

# Pick up PATH changes from the installers without reopening the terminal.
$env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
            [Environment]::GetEnvironmentVariable("Path", "User") + ";" +
            "$env:USERPROFILE\.cargo\bin"

rustup default stable
rustup update stable

# 4. Build.
Write-Host "==> Building TelemetryVibe..."
& (Join-Path $PSScriptRoot "build_windows.ps1") -Target $Target
Write-Host ""
$arch = if ($Target -like "aarch64*") { "arm64" } else { "x64" }
Write-Host "Done. Launch it with:  dist\TelemetryVibe\TelemetryVibe-$arch.exe"
