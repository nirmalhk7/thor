#!/usr/bin/env bash
# Launch the Thor-owned Chromium build (never system Google Chrome).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHROMIUM_ROOT="${THOR_CHROMIUM_SRC:-}"
if [[ -z "$CHROMIUM_ROOT" && -f "$ROOT/.thor_chromium_src_path" ]]; then
  CHROMIUM_ROOT="$(cat "$ROOT/.thor_chromium_src_path")"
fi
CHROMIUM_ROOT="${CHROMIUM_ROOT:-$HOME/thor-chromium}"
OUT="${THOR_CHROMIUM_OUT:-out/Thor}"
SRC="$CHROMIUM_ROOT/src"
URL="${1:-https://example.com}"
PORT="${THOR_REMOTE_DEBUGGING_PORT:-9222}"

# Prefer branded Chromium.app from out/Thor; fall back to Content Shell; then cache snapshot.
CANDIDATES=(
  "$SRC/$OUT/Thor.app/Contents/MacOS/Thor"
  "$SRC/$OUT/Chromium.app/Contents/MacOS/Chromium"
  "$SRC/$OUT/Content Shell.app/Contents/MacOS/Content Shell"
  "$ROOT/dist/Thor.app/Contents/Frameworks/Chromium.app/Contents/MacOS/Chromium"
  "${HOME}/Library/Application Support/Thor/ChromiumCache/Chromium.app/Contents/MacOS/Chromium"
)

EXE=""
for c in "${CANDIDATES[@]}"; do
  if [[ -x "$c" ]]; then
    EXE="$c"
    break
  fi
done

if [[ -z "$EXE" ]]; then
  echo "No Thor/Chromium binary found under $SRC/$OUT"
  echo "Build with: ./scripts/setup_depot_tools.sh && ./scripts/fetch_chromium.sh && ./scripts/build_chromium.sh"
  echo "Or bootstrap a snapshot: ./scripts/bootstrap_chromium_snapshot.sh"
  exit 1
fi

USER_DATA="${THOR_USER_DATA_DIR:-$HOME/Library/Application Support/Thor/ChromiumProfile}"
mkdir -p "$USER_DATA"

echo "⚡ Thor Chromium: $EXE"
echo "   user-data: $USER_DATA"
echo "   CDP port:  $PORT"
echo "   url:       $URL"

exec "$EXE" \
  --user-data-dir="$USER_DATA" \
  --remote-debugging-port="$PORT" \
  --no-first-run \
  --no-default-browser-check \
  "$URL"
