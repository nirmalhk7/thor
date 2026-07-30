#!/usr/bin/env bash
# Build content_shell (fast smoke) then chrome into out/Thor.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEPOT_TOOLS="${THOR_DEPOT_TOOLS:-$HOME/depot_tools}"
CHROMIUM_ROOT="${THOR_CHROMIUM_SRC:-}"
if [[ -z "$CHROMIUM_ROOT" && -f "$ROOT/.thor_chromium_src_path" ]]; then
  CHROMIUM_ROOT="$(cat "$ROOT/.thor_chromium_src_path")"
fi
CHROMIUM_ROOT="${CHROMIUM_ROOT:-$HOME/thor-chromium}"
SRC="$CHROMIUM_ROOT/src"
OUT="${THOR_CHROMIUM_OUT:-out/Thor}"
TARGET="${1:-all}" # content_shell | chrome | all

export PATH="$DEPOT_TOOLS:$PATH"

if [[ ! -d "$SRC" ]]; then
  echo "Missing $SRC — run ./scripts/fetch_chromium.sh"
  exit 1
fi

if [[ ! -f "$SRC/$OUT/args.gn" ]]; then
  "$ROOT/scripts/apply_thor_branding.sh"
fi

cd "$SRC"

if ! command -v gn >/dev/null 2>&1; then
  echo "gn not on PATH — run ./scripts/setup_depot_tools.sh and ensure PATH includes depot_tools"
  exit 1
fi

echo "→ gn gen $OUT"
gn gen "$OUT"

build_one() {
  local t="$1"
  echo "→ autoninja -C $OUT $t"
  if command -v autoninja >/dev/null 2>&1; then
    autoninja -C "$OUT" "$t"
  else
    ninja -C "$OUT" "$t"
  fi
}

case "$TARGET" in
  content_shell)
    build_one content_shell
    ;;
  chrome)
    build_one chrome
    ;;
  all)
    build_one content_shell
    build_one chrome
    ;;
  *)
    build_one "$TARGET"
    ;;
esac

APP="$SRC/$OUT/Chromium.app"
if [[ -d "$APP" ]]; then
  echo "✓ Built: $APP"
elif [[ -d "$SRC/$OUT/Content Shell.app" ]]; then
  echo "✓ Built Content Shell: $SRC/$OUT/Content Shell.app"
fi
echo "  Run: ./scripts/run_thor_chromium.sh"
