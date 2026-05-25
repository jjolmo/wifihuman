#!/bin/bash
# WifiHuman macOS Updater
# Downloads the latest version, replaces the current app, and clears quarantine.

REPO="jjolmo/wifihuman"
APP_NAME="WifiHuman.app"
INSTALL_DIR="/Applications"

echo "=== WifiHuman Updater ==="
echo ""

echo "Checking latest version..."
RELEASE_JSON=$(curl -sL "https://api.github.com/repos/$REPO/releases/latest")

if [ -z "$RELEASE_JSON" ]; then
    echo "Error: Could not reach GitHub API."
    echo "Press Enter to close." ; read ; exit 1
fi

PARSED=$(echo "$RELEASE_JSON" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    tag = data.get('tag_name', '')
    dmg_url = ''
    for asset in data.get('assets', []):
        if asset['name'].endswith('.dmg'):
            dmg_url = asset['browser_download_url']
            break
    print(f'{tag}|{dmg_url}')
except Exception as e:
    print(f'|', file=sys.stderr)
    sys.exit(1)
" 2>&1)

TAG=$(echo "$PARSED" | cut -d'|' -f1)
DMG_URL=$(echo "$PARSED" | cut -d'|' -f2)

if [ -z "$TAG" ]; then
    echo "Error: Could not parse release info."
    echo "Press Enter to close." ; read ; exit 1
fi

echo "Latest version: $TAG"

if [ -z "$DMG_URL" ]; then
    echo "Error: No DMG found in release $TAG."
    echo "Press Enter to close." ; read ; exit 1
fi

DMG_FILE=$(basename "$DMG_URL")
TMP_DIR=$(mktemp -d)
TMP_DMG="$TMP_DIR/$DMG_FILE"
MOUNT_POINT="$TMP_DIR/mount"

echo "Downloading $DMG_FILE..."
if ! curl -L --progress-bar --fail -o "$TMP_DMG" "$DMG_URL"; then
    echo "Error: Download failed."
    rm -rf "$TMP_DIR"
    echo "Press Enter to close." ; read ; exit 1
fi

FILE_SIZE=$(stat -f%z "$TMP_DMG" 2>/dev/null || echo "0")
if [ "$FILE_SIZE" -lt 1000 ]; then
    echo "Error: Downloaded file is too small (${FILE_SIZE} bytes)."
    rm -rf "$TMP_DIR"
    echo "Press Enter to close." ; read ; exit 1
fi

echo "Downloaded $(( FILE_SIZE / 1024 / 1024 )) MB"

echo "Mounting disk image..."
mkdir -p "$MOUNT_POINT"
if ! hdiutil attach "$TMP_DMG" -mountpoint "$MOUNT_POINT" -nobrowse -quiet; then
    echo "Error: Could not mount DMG."
    rm -rf "$TMP_DIR"
    echo "Press Enter to close." ; read ; exit 1
fi

if [ ! -d "$MOUNT_POINT/$APP_NAME" ]; then
    echo "Error: $APP_NAME not found in disk image."
    hdiutil detach "$MOUNT_POINT" 2>/dev/null || true
    rm -rf "$TMP_DIR"
    echo "Press Enter to close." ; read ; exit 1
fi

if pgrep -f "WifiHuman" > /dev/null 2>&1; then
    echo "Closing WifiHuman..."
    osascript -e 'quit app "WifiHuman"' 2>/dev/null || true
    for i in $(seq 1 10); do
        pgrep -f "WifiHuman" > /dev/null 2>&1 || break
        sleep 1
    done
    pkill -f "WifiHuman" 2>/dev/null || true
    sleep 1
fi

echo "Installing to $INSTALL_DIR/$APP_NAME..."
rm -rf "$INSTALL_DIR/$APP_NAME"
cp -R "$MOUNT_POINT/$APP_NAME" "$INSTALL_DIR/$APP_NAME"

if [ ! -d "$INSTALL_DIR/$APP_NAME" ]; then
    echo "Error: Failed to copy app."
    hdiutil detach "$MOUNT_POINT" 2>/dev/null || true
    rm -rf "$TMP_DIR"
    echo "Press Enter to close." ; read ; exit 1
fi

echo "Clearing quarantine..."
xattr -cr "$INSTALL_DIR/$APP_NAME"

echo "Cleaning up..."
hdiutil detach "$MOUNT_POINT" 2>/dev/null || true
rm -rf "$TMP_DIR"

echo ""
echo "Done! WifiHuman $TAG installed."
echo "Launching WifiHuman..."
open "$INSTALL_DIR/$APP_NAME"
