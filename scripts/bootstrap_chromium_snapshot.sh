#!/usr/bin/env bash
# Bootstrap: download pinned Chromium snapshot into Thor's cache / Frameworks layout.
# This is NOT the long-term product (source build is). It unblocks self-owned binary
# testing without a 50GB fetch.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MANIFEST="$ROOT/chromium_manifest.json"
CACHE="${THOR_CHROMIUM_CACHE:-$HOME/Library/Application Support/Thor/ChromiumCache}"
ARCH="$(uname -m)"

if [[ "$ARCH" == "arm64" ]]; then
  KEY="aarch64-apple-darwin"
else
  KEY="x86_64-apple-darwin"
fi

URL="$(python3 -c "import json; print(json.load(open('$MANIFEST'))['artifacts']['$KEY']['url'])")"
REL_EXE="$(python3 -c "import json; print(json.load(open('$MANIFEST'))['artifacts']['$KEY']['executable'])")"

# If pinned ZIP 404s, fall back to platform LAST_CHANGE
if ! curl -sfI "$URL" >/dev/null 2>&1; then
  if [[ "$ARCH" == "arm64" ]]; then
    LAST="$(curl -sfL https://storage.googleapis.com/chromium-browser-snapshots/Mac_Arm/LAST_CHANGE)"
    URL="https://storage.googleapis.com/chromium-browser-snapshots/Mac_Arm/${LAST}/chrome-mac.zip"
  else
    LAST="$(curl -sfL https://storage.googleapis.com/chromium-browser-snapshots/Mac/LAST_CHANGE)"
    URL="https://storage.googleapis.com/chromium-browser-snapshots/Mac/${LAST}/chrome-mac.zip"
  fi
  echo "⚠ Pinned snapshot missing; using LAST_CHANGE=$LAST"
fi

echo "→ Downloading Chromium snapshot for $KEY"
echo "  $URL"
mkdir -p "$CACHE"
TMP="$(mktemp -d)"
ZIP="$TMP/chrome-mac.zip"
curl -L --fail --progress-bar -o "$ZIP" "$URL"
unzip -q "$ZIP" -d "$TMP"
# Snapshot zips usually contain chrome-mac/Chromium.app
if [[ -d "$TMP/chrome-mac/Chromium.app" ]]; then
  rm -rf "$CACHE/Chromium.app"
  mv "$TMP/chrome-mac/Chromium.app" "$CACHE/Chromium.app"
elif [[ -d "$TMP/Chromium.app" ]]; then
  rm -rf "$CACHE/Chromium.app"
  mv "$TMP/Chromium.app" "$CACHE/Chromium.app"
else
  echo "Unexpected zip layout:"; find "$TMP" -maxdepth 3 -type d | head -40
  exit 1
fi
rm -rf "$TMP"

EXE="$CACHE/Chromium.app/Contents/MacOS/Chromium"
if [[ ! -x "$EXE" ]]; then
  echo "Executable missing after extract: $EXE (manifest said $REL_EXE)"
  exit 1
fi

# Also stage into dist/Thor.app Frameworks for release-shaped path
FRAMEWORKS="$ROOT/dist/Thor.app/Contents/Frameworks"
mkdir -p "$FRAMEWORKS"
rm -rf "$FRAMEWORKS/Chromium.app"
cp -R "$CACHE/Chromium.app" "$FRAMEWORKS/Chromium.app"

echo "✓ Snapshot Chromium at $EXE"
echo "  Frameworks copy: $FRAMEWORKS/Chromium.app"
echo "  Run: ./scripts/run_thor_chromium.sh"
