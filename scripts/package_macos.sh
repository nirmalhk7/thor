#!/bin/bash
set -e

VERSION="${1:-0.1.0}"
echo "============================================================"
echo "📦 Packaging Thor for macOS (v${VERSION})"
echo "============================================================"

# Ensure release binary is compiled
echo "🔨 Compiling release binary..."
cargo build --release --bin Thor

DIST_DIR="dist"
APP_DIR="${DIST_DIR}/Thor.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"

rm -rf "${DIST_DIR}"
mkdir -p "${MACOS_DIR}" "${RESOURCES_DIR}"

# Copy binary
cp "target/release/Thor" "${MACOS_DIR}/Thor"
chmod +x "${MACOS_DIR}/Thor"

# Create Info.plist
cat <<EOF > "${CONTENTS_DIR}/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>Thor</string>
    <key>CFBundleIdentifier</key>
    <string>com.nirmalhk7.thor</string>
    <key>CFBundleName</key>
    <string>Thor</string>
    <key>CFBundleDisplayName</key>
    <string>Thor Browser</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

echo "✅ Thor.app bundle created."

# Package ZIP archive
ZIP_NAME="Thor-v${VERSION}-macos.zip"
echo "🤐 Creating ${ZIP_NAME}..."
(cd "${DIST_DIR}" && zip -r "${ZIP_NAME}" "Thor.app" > /dev/null)

# Package DMG installer if hdiutil is available
DMG_NAME="Thor-v${VERSION}-macos.dmg"
if command -v hdiutil &> /dev/null; then
    echo "💿 Creating ${DMG_NAME} using hdiutil..."
    hdiutil create -volname "Thor Browser" -srcfolder "${APP_DIR}" -ov -format UDZO "${DIST_DIR}/${DMG_NAME}" > /dev/null
    echo "✅ DMG created successfully."
else
    echo "⚠️ hdiutil not found, skipping DMG creation."
fi

# Calculate SHA256 checksums
echo "🔒 Computing SHA256 checksums..."
(cd "${DIST_DIR}" && shasum -a 256 *.* > checksums.txt 2>/dev/null || sha256sum *.* > checksums.txt 2>/dev/null || true)

echo "============================================================"
echo "🎉 Thor macOS Packaging Complete!"
echo "Artifacts generated in ./${DIST_DIR}/:"
ls -la "${DIST_DIR}"
echo "============================================================"
