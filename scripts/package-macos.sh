#!/usr/bin/env bash
#
# package-macos.sh - Create a macOS .app bundle from the built binary
#
# USAGE:
#   ./scripts/package-macos.sh <path-to-binary> <output-dir>
#
# EXAMPLE:
#   ./scripts/package-macos.sh target/release/claude-usage dist/

set -euo pipefail

BINARY="$1"
OUTPUT_DIR="$2"

APP_NAME="Claude Usage"
APP_BUNDLE="${OUTPUT_DIR}/${APP_NAME}.app"
BUNDLE_ID="co.smartapps.claude-usage"

# Clean previous bundle
rm -rf "$APP_BUNDLE"

# Create bundle structure
mkdir -p "${APP_BUNDLE}/Contents/MacOS"
mkdir -p "${APP_BUNDLE}/Contents/Resources"

# Copy binary
cp "$BINARY" "${APP_BUNDLE}/Contents/MacOS/claude-usage"
chmod +x "${APP_BUNDLE}/Contents/MacOS/claude-usage"

# Convert PNG icon to icns
ICONSET=$(mktemp -d)/icon.iconset
mkdir -p "$ICONSET"
sips -z 16 16     images/icon.png --out "${ICONSET}/icon_16x16.png"      > /dev/null 2>&1
sips -z 32 32     images/icon.png --out "${ICONSET}/icon_16x16@2x.png"   > /dev/null 2>&1
sips -z 32 32     images/icon.png --out "${ICONSET}/icon_32x32.png"      > /dev/null 2>&1
sips -z 64 64     images/icon.png --out "${ICONSET}/icon_32x32@2x.png"   > /dev/null 2>&1
sips -z 128 128   images/icon.png --out "${ICONSET}/icon_128x128.png"    > /dev/null 2>&1
sips -z 256 256   images/icon.png --out "${ICONSET}/icon_128x128@2x.png" > /dev/null 2>&1
sips -z 256 256   images/icon.png --out "${ICONSET}/icon_256x256.png"    > /dev/null 2>&1
iconutil -c icns -o "${APP_BUNDLE}/Contents/Resources/icon.icns" "$ICONSET"

# Create Info.plist
cat > "${APP_BUNDLE}/Contents/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleVersion</key>
    <string>1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>claude-usage</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSUIElement</key>
    <false/>
</dict>
</plist>
PLIST

echo "Created ${APP_BUNDLE}"
