#!/bin/bash
# Package the binary as Motrix.app.
#
# Usage:
#   ./scripts/bundle-macos.sh [debug|release]
#
# Env:
#   TARGET         optional Rust target triple (e.g. x86_64-apple-darwin)
#   SIGN_IDENTITY  codesign identity; defaults to "-" (ad-hoc).
#                  A real Developer ID identity enables hardened runtime +
#                  secure timestamp, as required for notarization.
#   SKIP_BUILD=1   bundle an already-built binary
set -euo pipefail
cd "$(dirname "$0")/.."

PROFILE="${1:-debug}"
TARGET="${TARGET:-}"
SIGN_IDENTITY="${SIGN_IDENTITY:--}"

if [ "${SKIP_BUILD:-0}" != "1" ]; then
    FLAGS=()
    [ "$PROFILE" = "release" ] && FLAGS+=(--release)
    [ -n "$TARGET" ] && FLAGS+=(--target "$TARGET")
    cargo build "${FLAGS[@]}"
fi

if [ -n "$TARGET" ]; then
    BIN="target/$TARGET/$PROFILE/motrix"
else
    BIN="target/$PROFILE/motrix"
fi

APP=target/Motrix.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$BIN" "$APP/Contents/MacOS/Motrix"
cp assets/icon.icns "$APP/Contents/Resources/icon.icns"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key><string>en</string>
    <key>CFBundleExecutable</key><string>Motrix</string>
    <key>CFBundleIconFile</key><string>icon</string>
    <key>CFBundleIdentifier</key><string>com.vincent.motrix-gpui</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>CFBundleName</key><string>Motrix</string>
    <key>CFBundleDisplayName</key><string>Motrix</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>0.1.0</string>
    <key>CFBundleVersion</key><string>1</string>
    <key>LSMinimumSystemVersion</key><string>12.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
PLIST

if [ "$SIGN_IDENTITY" = "-" ]; then
    codesign --force --deep --sign - "$APP"
else
    codesign --force --deep --options runtime --timestamp \
        --sign "$SIGN_IDENTITY" "$APP"
fi

echo "Bundled: $APP (signed as: $SIGN_IDENTITY)"
