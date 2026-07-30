#!/usr/bin/env bash
# Copy Thor branding overlays into the Chromium src tree and seed out/Thor args.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHROMIUM_ROOT="${THOR_CHROMIUM_SRC:-}"
if [[ -z "$CHROMIUM_ROOT" && -f "$ROOT/.thor_chromium_src_path" ]]; then
  CHROMIUM_ROOT="$(cat "$ROOT/.thor_chromium_src_path")"
fi
CHROMIUM_ROOT="${CHROMIUM_ROOT:-$HOME/thor-chromium}"
SRC="$CHROMIUM_ROOT/src"

if [[ ! -d "$SRC" ]]; then
  echo "Chromium src not found at $SRC"
  echo "Run ./scripts/fetch_chromium.sh first (or set THOR_CHROMIUM_SRC)."
  exit 1
fi

echo "→ Applying Thor branding into $SRC"

THEME_DIR="$SRC/chrome/app/theme/thor"
mkdir -p "$THEME_DIR"
cp "$ROOT/browser/branding/BRANDING" "$THEME_DIR/BRANDING"
cp "$ROOT/browser/branding/thor_resources.md" "$THEME_DIR/README.thor.md"

# Seed a PRODUCT_STRING override note for chrome branding
cat > "$THEME_DIR/thor_product.h.inc" <<'EOF'
// Thor product string overrides — included by branding patches once wired.
// PRODUCT_SHORTNAME Thor / MAC_BUNDLE_ID com.thor.browser
#pragma once
#define THOR_PRODUCT_NAME "Thor"
#define THOR_BUNDLE_ID "com.thor.browser"
EOF

mkdir -p "$SRC/out/Thor"
cp "$ROOT/browser/gn_args.thor.gn" "$SRC/out/Thor/args.gn"

# Native messaging host template for Labs sidecar (developer machine path filled at install)
NM_DIR="$SRC/chrome/browser/thor"
mkdir -p "$NM_DIR"
cp "$ROOT/browser/native_messaging/com.thor.labs.json" "$NM_DIR/com.thor.labs.json"
cp "$ROOT/browser/isolation/MAPPING.md" "$NM_DIR/ISOLATION.md"

# Apply real patches if present (skip .example)
shopt -s nullglob
for patch in "$ROOT/browser/patches/"*.patch; do
  echo "→ git apply $(basename "$patch")"
  git -C "$SRC" apply --whitespace=nowarn "$patch" || {
    echo "⚠ patch failed (tree may already include change): $patch"
  }
done
shopt -u nullglob

echo "✓ Branding applied. Configure with:"
echo "    cd $SRC && gn gen out/Thor"
echo "  Then: ./scripts/build_chromium.sh"
