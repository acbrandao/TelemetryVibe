#!/usr/bin/env bash
# Builds TelemetryVibe for Linux into dist/TelemetryVibe (binary + icon + .desktop file).
#
#   scripts/build_linux.sh              # build only
#   INSTALL=1 scripts/build_linux.sh    # also install into ~/.local (app menu, .gpsvideo association)
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release
OUT="dist/TelemetryVibe"
rm -rf "$OUT"
mkdir -p "$OUT"
cp target/release/telemetryvibe "$OUT/"

# Icon: rendered by the app itself.
ICONS=$(mktemp -d)
"$OUT/telemetryvibe" --write-icons "$ICONS"
cp "$ICONS/icon_256.png" "$OUT/telemetryvibe.png"

cat > "$OUT/telemetryvibe.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=TelemetryVibe
Comment=GPS telemetry gauge overlays for video
Exec=telemetryvibe %f
Icon=telemetryvibe
Terminal=false
Categories=AudioVideo;Video;
MimeType=application/x-telemetryvibe-project;
EOF

echo "Built $OUT/telemetryvibe"

if [[ "${INSTALL:-0}" == "1" ]]; then
    BIN="$HOME/.local/bin"
    SHARE="$HOME/.local/share"
    mkdir -p "$BIN" "$SHARE/applications" "$SHARE/mime/packages"
    install -m 755 "$OUT/telemetryvibe" "$BIN/telemetryvibe"
    for s in 16 32 64 128 256 512; do
        mkdir -p "$SHARE/icons/hicolor/${s}x${s}/apps"
        cp "$ICONS/icon_${s}.png" "$SHARE/icons/hicolor/${s}x${s}/apps/telemetryvibe.png"
    done
    sed "s|^Exec=telemetryvibe|Exec=$BIN/telemetryvibe|" "$OUT/telemetryvibe.desktop" > "$SHARE/applications/telemetryvibe.desktop"
    cat > "$SHARE/mime/packages/telemetryvibe.xml" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/x-telemetryvibe-project">
    <comment>TelemetryVibe Project</comment>
    <glob pattern="*.gpsvideo"/>
  </mime-type>
</mime-info>
EOF
    command -v update-mime-database >/dev/null && update-mime-database "$SHARE/mime" || true
    command -v update-desktop-database >/dev/null && update-desktop-database "$SHARE/applications" || true
    echo "Installed to $BIN/telemetryvibe (TelemetryVibe now appears in your app menu)."
fi
