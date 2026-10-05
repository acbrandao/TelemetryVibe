# Builds TelemetryVibe for Windows. Targets this machine's architecture by default; pass
# -Target x86_64-pc-windows-msvc or -Target aarch64-pc-windows-msvc to cross-build.
# Output is named after the architecture: dist\TelemetryVibe\TelemetryVibe-x64.exe or TelemetryVibe-arm64.exe.
# Place ffmpeg.exe and ffprobe.exe next to the exe (dist\TelemetryVibe) to bundle FFmpeg;
# otherwise the app finds FFmpeg on PATH or via Settings.
param([string]$Target)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
if (-not $Target) {
    # The registry holds the native OS architecture, even when PowerShell runs under x64 emulation on ARM64.
    $native = (Get-ItemProperty "HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment").PROCESSOR_ARCHITECTURE
    $Target = if ($native -eq "ARM64") { "aarch64-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" }
    Write-Host "==> Detected $native Windows; building for $Target"
}
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
