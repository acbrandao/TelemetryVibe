# Builds TelemetryVibe for Windows (x64 by default; pass -Target aarch64-pc-windows-msvc for ARM64).
# Place ffmpeg.exe and ffprobe.exe next to telemetryvibe.exe (dist\TelemetryVibe) to bundle FFmpeg;
# otherwise the app finds FFmpeg on PATH or via Settings.
param([string]$Target = "x86_64-pc-windows-msvc")
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
rustup target add $Target | Out-Null
cargo build --release --target $Target
$out = "dist\TelemetryVibe"
New-Item -ItemType Directory -Force -Path $out | Out-Null
Copy-Item "target\$Target\release\telemetryvibe.exe" $out -Force
Write-Host "Built $out\telemetryvibe.exe"
