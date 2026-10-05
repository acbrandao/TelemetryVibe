#!/usr/bin/env bash
# One-shot Linux setup: installs system packages, Rust and FFmpeg, then builds dist/TelemetryVibe.
# Supports Debian/Ubuntu (apt), Fedora (dnf) and Arch (pacman). Safe to re-run.
#
#   scripts/setup_linux.sh              # build only
#   INSTALL=1 scripts/setup_linux.sh    # build and add TelemetryVibe to your app menu
set -euo pipefail
cd "$(dirname "$0")/.."

SUDO=""
[[ $EUID -ne 0 ]] && SUDO="sudo"

# 1. System packages: compiler, windowing/graphics libraries, file-dialog portal, FFmpeg.
echo "==> Installing system packages..."
if command -v apt-get >/dev/null 2>&1; then
    $SUDO apt-get update
    $SUDO apt-get install -y build-essential pkg-config curl \
        libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
        libvulkan1 mesa-vulkan-drivers xdg-desktop-portal xdg-desktop-portal-gtk ffmpeg
elif command -v dnf >/dev/null 2>&1; then
    $SUDO dnf install -y gcc gcc-c++ make pkgconf-pkg-config curl \
        libxkbcommon-devel wayland-devel libX11-devel libXcursor-devel libXrandr-devel libXi-devel \
        vulkan-loader mesa-vulkan-drivers xdg-desktop-portal xdg-desktop-portal-gtk
    # Full FFmpeg (with H.264/H.265 encoders) comes from RPM Fusion; fall back to Fedora's ffmpeg-free.
    if ! command -v ffmpeg >/dev/null 2>&1; then
        $SUDO dnf install -y ffmpeg || $SUDO dnf install -y ffmpeg-free || true
    fi
elif command -v pacman >/dev/null 2>&1; then
    $SUDO pacman -Sy --needed --noconfirm base-devel curl \
        libxkbcommon wayland libx11 libxcursor libxrandr libxi \
        vulkan-icd-loader xdg-desktop-portal xdg-desktop-portal-gtk ffmpeg
else
    echo "Unsupported package manager. Install a C compiler, pkg-config, the X11/Wayland/xkbcommon"
    echo "development packages, Vulkan or OpenGL drivers, and ffmpeg, then run scripts/build_linux.sh."
    exit 1
fi

# 2. Rust (via rustup).
[[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env"
if ! command -v rustup >/dev/null 2>&1; then
    echo "==> Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi
rustup update stable

command -v ffmpeg >/dev/null 2>&1 || echo "WARNING: ffmpeg not found; install it before rendering videos."

# 3. Build.
echo "==> Building TelemetryVibe..."
scripts/build_linux.sh
