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
URL="${1:-about:blank}"
PORT="${THOR_REMOTE_DEBUGGING_PORT:-0}"
SHIELD_DIR="${THOR_SHIELD_EXTENSION_PATH:-$ROOT/browser/extensions/thor-shield}"
DOH_MODE="${THOR_DOH_MODE:-secure}"
DOH_TEMPLATE="${THOR_DOH_TEMPLATE:-https://cloudflare-dns.com/dns-query}"
WEBRTC_POLICY="${THOR_WEBRTC_IP_POLICY:-disable_non_proxied_udp}"
PRIVACY_PROFILE="${THOR_PRIVACY_PROFILE:-stealth}"
case "$PRIVACY_PROFILE" in
  devhost|stealth|hardened) ;;
  *)
    echo "Unknown THOR_PRIVACY_PROFILE '$PRIVACY_PROFILE'; use devhost, stealth, or hardened." >&2
    exit 1
    ;;
esac

# Use only a Thor-owned browser bundle; never Content Shell or a stock snapshot.
CANDIDATES=(
  "$SRC/$OUT/Thor.app/Contents/MacOS/Thor"
  "$SRC/$OUT/Chromium.app/Contents/MacOS/Chromium"
  "$ROOT/dist/Thor.app/Contents/Frameworks/Chromium.app/Contents/MacOS/Chromium"
  "$ROOT/dist/Thor.app/Contents/Frameworks/Chromium.app/Contents/MacOS/Thor"
)

EXE=""
for c in "${CANDIDATES[@]}"; do
  if [[ "$c" == "$SRC/$OUT/"* ]]; then
    MARKER="$SRC/$OUT/ThorOwnedChromium.json"
  else
    MARKER="$ROOT/dist/Thor.app/Contents/Resources/ThorOwnedChromium.json"
  fi
  if [[ -x "$c" && -f "$MARKER" ]] && \
     python3 "$ROOT/scripts/generate_thor_provenance.py" \
       --shield-dir "$SHIELD_DIR" --browser-executable "$c" \
       --check "$MARKER" >/dev/null; then
    EXE="$c"
    break
  fi
done

if [[ -z "$EXE" ]]; then
  echo "No Thor-owned Chromium binary and provenance marker found under $SRC/$OUT"
  echo "Build with: ./scripts/fetch_chromium.sh && ./scripts/apply_thor_branding.sh && ./scripts/build_chromium.sh"
  exit 1
fi
SHIELD_ASSETS=(manifest.json service_worker.js fingerprint.js rules.json filter_bundle.json options.html options.js)
for asset in "${SHIELD_ASSETS[@]}"; do
  if [[ ! -s "$SHIELD_DIR/$asset" ]]; then
    echo "Thor Shield is mandatory; required asset missing or empty: $SHIELD_DIR/$asset"
    exit 1
  fi
done
if ! grep -q '"manifest_version"[[:space:]]*:[[:space:]]*3' "$SHIELD_DIR/manifest.json" ||
   ! grep -q '"run_at"[[:space:]]*:[[:space:]]*"document_start"' "$SHIELD_DIR/manifest.json" ||
   ! grep -q '"world"[[:space:]]*:[[:space:]]*"MAIN"' "$SHIELD_DIR/manifest.json" ||
   ! grep -q '"id"[[:space:]]*:[[:space:]]*"thor_shield_rules"' "$SHIELD_DIR/manifest.json" ||
   ! grep -q '"enabled"[[:space:]]*:[[:space:]]*true' "$SHIELD_DIR/manifest.json"; then
  echo "Thor Shield manifest is invalid or does not enforce its ruleset and MAIN-world document_start protection"
  exit 1
fi

USER_DATA="${THOR_USER_DATA_DIR:-$HOME/Library/Application Support/Thor/ChromiumProfile}"
mkdir -p "$USER_DATA"

echo "⚡ Thor Chromium: $EXE"
echo "   user-data: $USER_DATA"
if [[ "$PORT" == "0" ]]; then
  echo "   CDP port:  ephemeral (see $USER_DATA/DevToolsActivePort)"
else
  echo "   CDP port:  $PORT"
fi
echo "   url:       $URL"

exec "$EXE" \
  --user-data-dir="$USER_DATA" \
  --remote-debugging-port="$PORT" \
  --no-first-run \
  --no-default-browser-check \
  --no-pings \
  --thor-privacy-profile="$PRIVACY_PROFILE" \
  --force-webrtc-ip-handling-policy="$WEBRTC_POLICY" \
  --dns-over-https-mode="$DOH_MODE" \
  --dns-over-https-templates="$DOH_TEMPLATE" \
  --load-extension="$SHIELD_DIR" \
  "$URL"
