#!/bin/bash
set -euo pipefail

echo "THOR_DOWNSTREAM_UNSUPPORTED: Linux packaging is gated until it bundles and verifies a Thor-owned Chromium build."
exit 2

VERSION="${1:-0.1.0}"
ARCH="${2:-x86_64}"
echo "============================================================"
echo "📦 Packaging Thor for Linux (${ARCH}, v${VERSION})"
echo "============================================================"

echo "🔨 Compiling release binaries..."
cargo build --release --bin Thor --bin thor-labs

DIST_DIR="dist"
mkdir -p "${DIST_DIR}"

STAGING_DIR="${DIST_DIR}/thor-linux-${ARCH}"
rm -rf "${STAGING_DIR}"
mkdir -p "${STAGING_DIR}/usr/bin" "${STAGING_DIR}/usr/lib/thor" "${STAGING_DIR}/usr/share/applications" "${STAGING_DIR}/usr/share/pixmaps"

# Copy binaries
cp "target/release/Thor" "${STAGING_DIR}/usr/bin/Thor"
chmod +x "${STAGING_DIR}/usr/bin/Thor"
if [[ -f "target/release/thor-labs" ]]; then
  cp "target/release/thor-labs" "${STAGING_DIR}/usr/bin/thor-labs"
  chmod +x "${STAGING_DIR}/usr/bin/thor-labs"
fi

# Create Desktop Entry
cat <<EOF > "${STAGING_DIR}/usr/share/applications/com.thor.browser.desktop"
[Desktop Entry]
Version=1.0
Type=Application
Name=Thor
GenericName=Web Browser
Comment=High-performance, ephemeral developer browser
Exec=Thor %U
Icon=com.thor.browser
Categories=Development;Network;WebBrowser;
MimeType=text/html;text/xml;application/xhtml+xml;x-scheme-handler/http;x-scheme-handler/https;
StartupWMClass=Thor
Terminal=false
EOF

# 1. Package .tar.xz Archive
TARBALL_NAME="Thor-v${VERSION}-linux-${ARCH}.tar.xz"
echo "🤐 Creating ${TARBALL_NAME}..."
tar -cJf "${DIST_DIR}/${TARBALL_NAME}" -C "${DIST_DIR}" "thor-linux-${ARCH}"

# 2. Package .deb (Debian/Ubuntu) Package
DEB_NAME="Thor-v${VERSION}-linux-${ARCH}.deb"
echo "📦 Creating ${DEB_NAME}..."
DEB_DIR="${DIST_DIR}/deb-staging"
rm -rf "${DEB_DIR}"
mkdir -p "${DEB_DIR}/DEBIAN"
cp -r "${STAGING_DIR}/usr" "${DEB_DIR}/"

DEB_ARCH="${ARCH}"
if [[ "${ARCH}" == "x86_64" ]]; then DEB_ARCH="amd64"; fi
if [[ "${ARCH}" == "aarch64" ]]; then DEB_ARCH="arm64"; fi

cat <<EOF > "${DEB_DIR}/DEBIAN/control"
Package: thor-browser
Version: ${VERSION}
Section: web
Priority: optional
Architecture: ${DEB_ARCH}
Maintainer: Thor Contributors <dev@thor-browser.org>
Description: High-performance, ephemeral developer browser
 Thor is a lighter/faster-than-Chrome developer browser where every tab
 is an isolated experiment and every interaction can become a test, trace,
 benchmark, finding, or reproducible artifact.
EOF

if command -v dpkg-deb &> /dev/null; then
  dpkg-deb --build "${DEB_DIR}" "${DIST_DIR}/${DEB_NAME}"
  echo "✅ DEB package created."
else
  echo "⚠️ dpkg-deb not found. Creating tar-based deb archive fallback."
  tar -czf "${DIST_DIR}/${DEB_NAME}" -C "${DEB_DIR}" .
fi
rm -rf "${DEB_DIR}"

# 3. Package .rpm (RedHat/Fedora) Package
RPM_NAME="Thor-v${VERSION}-linux-${ARCH}.rpm"
echo "📦 Creating ${RPM_NAME}..."
if command -v rpmbuild &> /dev/null; then
  RPMBUILD_ROOT="${DIST_DIR}/rpmbuild"
  mkdir -p "${RPMBUILD_ROOT}/"{BUILD,RPMS,SOURCES,SPECS,BUILDROOT}
  cat <<EOF > "${RPMBUILD_ROOT}/SPECS/thor.spec"
Name: thor-browser
Version: ${VERSION}
Release: 1
Summary: High-performance, ephemeral developer browser
License: MIT
Group: Applications/Internet
BuildArch: ${ARCH}

%description
Thor is a lighter/faster-than-Chrome developer browser.

%install
mkdir -p %{buildroot}/usr/bin
cp -a ${PWD}/${STAGING_DIR}/usr/bin/* %{buildroot}/usr/bin/

%files
/usr/bin/Thor
/usr/bin/thor-labs
EOF
  rpmbuild -bb --define "_topdir ${PWD}/${RPMBUILD_ROOT}" "${RPMBUILD_ROOT}/SPECS/thor.spec"
  cp "${RPMBUILD_ROOT}/RPMS/${ARCH}/"* "${DIST_DIR}/${RPM_NAME}" 2>/dev/null || true
  rm -rf "${RPMBUILD_ROOT}"
  echo "✅ RPM package created via rpmbuild."
elif command -v fpm &> /dev/null; then
  fpm -s dir -t rpm -n thor-browser -v "${VERSION}" -C "${STAGING_DIR}" -p "${DIST_DIR}/${RPM_NAME}" usr
  echo "✅ RPM package created via fpm."
else
  echo "⚠️ rpmbuild/fpm not found, creating tar-based rpm archive fallback."
  tar -czf "${DIST_DIR}/${RPM_NAME}" -C "${STAGING_DIR}" .
fi

# 4. Package AppImage
APPIMAGE_NAME="Thor-v${VERSION}-linux-${ARCH}.AppImage"
echo "🖼️ Creating AppImage..."
APPDIR="${DIST_DIR}/AppDir"
rm -rf "${APPDIR}"
mkdir -p "${APPDIR}/usr"
cp -r "${STAGING_DIR}/usr/"* "${APPDIR}/usr/"
cp "${STAGING_DIR}/usr/share/applications/com.thor.browser.desktop" "${APPDIR}/"

cat <<EOF > "${APPDIR}/AppRun"
#!/bin/bash
HERE="\$(dirname "\$(readlink -f "\${0}")")"
export PATH="\${HERE}/usr/bin:\${PATH}"
exec "\${HERE}/usr/bin/Thor" "\$@"
EOF
chmod +x "${APPDIR}/AppRun"

if command -v appimagetool &> /dev/null; then
  ARCH="${ARCH}" appimagetool "${APPDIR}" "${DIST_DIR}/${APPIMAGE_NAME}"
  echo "✅ AppImage created."
else
  echo "⚠️ appimagetool not found, bundling portable AppDir archive fallback."
  tar -czf "${DIST_DIR}/${APPIMAGE_NAME}" -C "${APPDIR}" .
fi
rm -rf "${APPDIR}"

# Calculate SHA256 Checksums
echo "🔒 Computing SHA256 checksums..."
(cd "${DIST_DIR}" && (shasum -a 256 *.tar.xz *.deb *.rpm *.AppImage 2>/dev/null || sha256sum *.tar.xz *.deb *.rpm *.AppImage 2>/dev/null || true) >> checksums.txt)

echo "============================================================"
echo "🎉 Thor Linux Packaging Complete!"
echo "Artifacts in ./${DIST_DIR}/:"
ls -la "${DIST_DIR}"
echo "============================================================"
