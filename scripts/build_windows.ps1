# Builds TelemetryVibe for Windows (x64 by default; pass -Target aarch64-pc-windows-msvc for ARM64).
# Output is named after the architecture: dist\TelemetryVibe\TelemetryVibe-x64.exe or TelemetryVibe-arm64.exe.
# Place ffmpeg.exe and ffprobe.exe next to the exe (dist\TelemetryVibe) to bundle FFmpeg;
# otherwise the app finds FFmpeg on PATH or via Settings.
param([string]$Target = "x86_64-pc-windows-msvc")
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
$arch = switch ($Target) {
    "x86_64-pc-windows-msvc"  { "x64" }
    "aarch64-pc-windows-msvc" { "arm64" }
    default { throw "Unsupported target '$Target'. Use x86_64-pc-windows-msvc or aarch64-pc-windows-msvc." }
}
rustup target add $Target | Out-Null
cargo build --release --target $Target
if ($LASTEXITCODE -ne 0) { throw "cargo build failed for $Target" }
$out = "dist\TelemetryVibe"
$exe = "TelemetryVibe-$arch.exe"
New-Item -ItemType Directory -Force -Path $out | Out-Null
Copy-Item "target\$Target\release\telemetryvibe.exe" (Join-Path $out $exe) -Force
Write-Host "Built $out\$exe"
