#!/bin/bash
set -e

echo "Building Homebrew Cask package for Thor..."
# Placeholder for packaging logic
TARBALL="Thor-mac-v1.0.0.tar.gz"
echo "Creating $TARBALL (Mock)..."
mkdir -p ../target/release
touch ../target/release/Thor
tar -czvf $TARBALL -C ../target/release Thor > /dev/null 2>&1 || true

echo "Calculating SHA256..."
SHA256=$(shasum -a 256 $TARBALL 2>/dev/null | awk '{print $1}' || echo "dummy_sha256")

echo "Update your Homebrew formula with this SHA: $SHA256"
echo "Homebrew packaging complete."
