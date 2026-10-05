#!/usr/bin/env bash
# Builds TelemetryVibe.app (optionally universal arm64 + x86_64) with icon and .gpsvideo association.
#
#   scripts/bundle_macos.sh                 # native architecture
#   UNIVERSAL=1 scripts/bundle_macos.sh     # arm64 + x86_64 (needs both rustup targets)
#   FFMPEG_DIR=/path/to/static/ffmpeg scripts/bundle_macos.sh   # bundle ffmpeg + ffprobe
#
# Bundled FFmpeg must be a self-contained (static) build; Homebrew's FFmpeg links against
# Homebrew dylibs and is not relocatable. Check the FFmpeg build's license before distributing.
set -euo pipefail

cd "$(dirname "$0")/.."
APP_NAME="TelemetryVibe"
VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')
OUT="dist/${APP_NAME}.app"

if [[ "${UNIVERSAL:-0}" == "1" ]]; then
    rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
    cargo build --release --target aarch64-apple-darwin
    cargo build --release --target x86_64-apple-darwin
    mkdir -p target/universal
    lipo -create -output target/universal/telemetryvibe \
        target/aarch64-apple-darwin/release/telemetryvibe \
        target/x86_64-apple-darwin/release/telemetryvibe
    BIN=target/universal/telemetryvibe
else
    cargo build --release
    BIN=target/release/telemetryvibe
fi

rm -rf "$OUT"
mkdir -p "$OUT/Contents/MacOS" "$OUT/Contents/Resources"
cp "$BIN" "$OUT/Contents/MacOS/telemetryvibe"

# Icon: rendered by the app itself, converted to .icns.
ICONSET=$(mktemp -d)/AppIcon.iconset
mkdir -p "$ICONSET"
PNGS=$(mktemp -d)
"$BIN" --write-icons "$PNGS"
for s in 16 32 128 256 512; do
    cp "$PNGS/icon_${s}.png" "$ICONSET/icon_${s}x${s}.png"
    d=$((s * 2))
    cp "$PNGS/icon_${d}.png" "$ICONSET/icon_${s}x${s}@2x.png" 2>/dev/null || sips -z $d $d "$PNGS/icon_1024.png" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$OUT/Contents/Resources/AppIcon.icns"

if [[ -n "${FFMPEG_DIR:-}" ]]; then
    cp "$FFMPEG_DIR/ffmpeg" "$FFMPEG_DIR/ffprobe" "$OUT/Contents/Resources/"
    chmod +x "$OUT/Contents/Resources/ffmpeg" "$OUT/Contents/Resources/ffprobe"
fi

cat > "$OUT/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key><string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key><string>net.telemetryvibe.TelemetryVibe</string>
    <key>CFBundleVersion</key><string>${VERSION}</string>
    <key>CFBundleShortVersionString</key><string>${VERSION}</string>
    <key>CFBundleExecutable</key><string>telemetryvibe</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>LSApplicationCategoryType</key><string>public.app-category.video</string>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key><string>TelemetryVibe Project</string>
            <key>CFBundleTypeRole</key><string>Editor</string>
            <key>LSHandlerRank</key><string>Owner</string>
            <key>CFBundleTypeIconFile</key><string>AppIcon</string>
            <key>LSItemContentTypes</key><array><string>net.telemetryvibe.project</string></array>
        </dict>
    </array>
    <key>UTExportedTypeDeclarations</key>
    <array>
        <dict>
            <key>UTTypeIdentifier</key><string>net.telemetryvibe.project</string>
            <key>UTTypeDescription</key><string>TelemetryVibe Project</string>
            <key>UTTypeConformsTo</key><array><string>public.json</string></array>
            <key>UTTypeTagSpecification</key>
            <dict><key>public.filename-extension</key><array><string>gpsvideo</string></array></dict>
        </dict>
    </array>
</dict>
</plist>
EOF

# Ad-hoc signature so Gatekeeper on Apple Silicon accepts the local build.
codesign --force --deep --sign - "$OUT" >/dev/null 2>&1 || true
echo "Built $OUT"
