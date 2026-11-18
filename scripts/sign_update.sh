#!/usr/bin/env bash
# Generate the signed feed only after the release archive is complete.
set -euo pipefail

VERSION="${1:?Pass the release version}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
: "${THOR_UPDATE_PRIVATE_KEY:?Configure the private update key in repository secrets}"
SPARKLE_DIR="${SPARKLE_DIR:-target/sparkle-2.10.1}"
UPDATES="$(mktemp -d)"
trap 'rm -rf "$UPDATES"' EXIT
cp "dist/Thor-v${VERSION}-macos.zip" "$UPDATES/"
unzip -p "$UPDATES/Thor-v${VERSION}-macos.zip" Thor.app/Contents/Info.plist > "$UPDATES/Info.plist"
PUBLIC_KEY="$(/usr/libexec/PlistBuddy -c 'Print SUPublicEDKey' "$UPDATES/Info.plist")"
printf '%s' "$THOR_UPDATE_PRIVATE_KEY" | /usr/bin/swift "$SCRIPT_DIR/verify_update_key.swift" "$PUBLIC_KEY"
printf '%s' "$THOR_UPDATE_PRIVATE_KEY" | "$SPARKLE_DIR/bin/generate_appcast" \
  --ed-key-file - --maximum-deltas 0 \
  --download-url-prefix "https://github.com/nirmalhk7/thor/releases/download/v${VERSION}/" \
  --link https://github.com/nirmalhk7/thor/releases \
  "$UPDATES"
printf '%s' "$THOR_UPDATE_PRIVATE_KEY" | "$SPARKLE_DIR/bin/sign_update" \
  --verify --ed-key-file - "$UPDATES/appcast.xml"
mv "$UPDATES/appcast.xml" dist/appcast.xml
