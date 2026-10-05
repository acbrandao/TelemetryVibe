#!/usr/bin/env bash
# One-shot macOS setup: installs the build tools, Rust and FFmpeg, then builds dist/TelemetryVibe.app.
# Safe to re-run; anything already installed is skipped.
#
#   scripts/setup_macos.sh
set -euo pipefail
cd "$(dirname "$0")/.."

# 1. Apple's command-line tools (compiler, linker, codesign, iconutil).
if ! xcode-select -p >/dev/null 2>&1; then
    echo "==> Installing Xcode Command Line Tools (a dialog will open)..."
    xcode-select --install || true
    echo "Finish the installer dialog, then run this script again."
    exit 1
fi

# 2. Homebrew (used to install FFmpeg).
if ! command -v brew >/dev/null 2>&1; then
    for b in /opt/homebrew/bin/brew /usr/local/bin/brew; do
        if [[ -x "$b" ]]; then eval "$("$b" shellenv)"; fi
    done
fi
if ! command -v brew >/dev/null 2>&1; then
    echo "==> Installing Homebrew..."
    NONINTERACTIVE=1 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    for b in /opt/homebrew/bin/brew /usr/local/bin/brew; do
        if [[ -x "$b" ]]; then eval "$("$b" shellenv)"; fi
    done
fi

# 3. Rust (via rustup).
[[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env"
if ! command -v rustup >/dev/null 2>&1; then
    echo "==> Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi
rustup update stable

# 4. FFmpeg (the app runs ffmpeg/ffprobe for decoding and encoding).
if ! command -v ffmpeg >/dev/null 2>&1; then
    echo "==> Installing FFmpeg..."
    brew install ffmpeg
fi

# 5. Build the app bundle.
echo "==> Building TelemetryVibe..."
scripts/bundle_macos.sh
echo
echo "Done. Launch it with:  open dist/TelemetryVibe.app"
echo "Or drag dist/TelemetryVibe.app into /Applications."
