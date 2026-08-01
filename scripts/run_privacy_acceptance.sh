#!/usr/bin/env bash
# Launch the ignored, browser-backed Thor privacy acceptance harness.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${THOR_CHROMIUM_OUT:-out/Thor}"

usage() {
  cat <<'EOF'
Usage: scripts/run_privacy_acceptance.sh [THOR_OWNED_BROWSER_EXECUTABLE]

Runs tests/privacy_browser_acceptance.rs against a real Thor-owned Chromium.
Accepted inputs are:
  * a source build under Chromium src/out/... with ThorOwnedChromium.json; or
  * Chromium.app embedded in a packaged Thor.app with the packaged marker.

The harness verifies provenance before launch and fails (never skips) when its
selected ignored test lacks a valid binary. It uses an isolated temporary
profile and loopback-only test pages. Ordinary cargo test keeps it ignored.

Coverage: BrowserContext cookie/localStorage/sessionStorage/IndexedDB/
CacheStorage/service-worker/permission isolation, object-URL non-sharing,
failed-navigation, disposable final-tab, and coordinator-detach cleanup,
JavaScript GPC, Sec-GPC and DNT request headers, JavaScript DNT, strict WebRTC
process policy/candidates, native top-level tracking-query cleanup, and clean
about:blank startup without unsolicited page-target URLs.

Explicitly unverified: HSTS state, HTTP authentication cache, socket/network
connection pools, startup orphan BrowserContext cleanup, Safe Browsing/component
readiness, HTTPS-Only/mixed-content interstitials, third-party-cookie/history
persistence, DoH health/browser-process egress, filter/CNAME effect, native
identity menu commands, profile API/JIT/WebGL behavior, and site-isolation/
compiler/CA/download enforcement. The suite records these gaps explicitly; it
does not convert requested configuration into runtime proof.

Equivalent explicit command:
  THOR_PRIVACY_ACCEPTANCE_BINARY=/absolute/path/to/Chromium \
    cargo test --test privacy_browser_acceptance \
      thor_browser_privacy_acceptance -- --ignored --exact --nocapture
EOF
}

case "${1:-}" in
  -h|--help)
    usage
    exit 0
    ;;
esac
if (( $# > 1 )); then
  usage >&2
  exit 2
fi

BINARY="${1:-${THOR_PRIVACY_ACCEPTANCE_BINARY:-}}"
if [[ -z "$BINARY" ]]; then
  CHROMIUM_ROOT="${THOR_CHROMIUM_SRC:-}"
  if [[ -z "$CHROMIUM_ROOT" && -f "$ROOT/.thor_chromium_src_path" ]]; then
    CHROMIUM_ROOT="$(<"$ROOT/.thor_chromium_src_path")"
  fi
  CHROMIUM_ROOT="${CHROMIUM_ROOT:-$HOME/thor-chromium}"
  CANDIDATES=(
    "$CHROMIUM_ROOT/src/$OUT/Thor.app/Contents/MacOS/Thor"
    "$CHROMIUM_ROOT/src/$OUT/Thor.app/Contents/MacOS/Chromium"
    "$CHROMIUM_ROOT/src/$OUT/Chromium.app/Contents/MacOS/Chromium"
    "$CHROMIUM_ROOT/src/$OUT/Chromium.app/Contents/MacOS/Thor"
    "$ROOT/dist/Thor.app/Contents/Frameworks/Chromium.app/Contents/MacOS/Chromium"
    "$ROOT/dist/Thor.app/Contents/Frameworks/Chromium.app/Contents/MacOS/Thor"
  )
  for candidate in "${CANDIDATES[@]}"; do
    if [[ ! -x "$candidate" ]]; then
      continue
    fi
    if [[ "$candidate" == "$CHROMIUM_ROOT/src/$OUT/"* ]]; then
      candidate_marker="$CHROMIUM_ROOT/src/$OUT/ThorOwnedChromium.json"
      candidate_shield="$CHROMIUM_ROOT/src/$OUT/ThorShield"
    else
      candidate_marker="$ROOT/dist/Thor.app/Contents/Resources/ThorOwnedChromium.json"
      candidate_shield="$ROOT/dist/Thor.app/Contents/Resources/ThorShield"
    fi
    if [[ -f "$candidate_marker" && -d "$candidate_shield" ]] && \
       python3 "$ROOT/scripts/generate_thor_provenance.py" \
         --shield-dir "$candidate_shield" --browser-executable "$candidate" \
         --check "$candidate_marker" >/dev/null 2>&1; then
      BINARY="$candidate"
      break
    fi
  done
fi

if [[ -z "$BINARY" || ! -x "$BINARY" ]]; then
  echo "No executable Thor-owned source/package browser found." >&2
  echo "Pass its absolute executable path or build/package Thor first." >&2
  exit 1
fi
if [[ "$BINARY" != /* ]]; then
  BINARY="$(cd "$(dirname "$BINARY")" && pwd)/$(basename "$BINARY")"
fi

cd "$ROOT"
THOR_PRIVACY_ACCEPTANCE_BINARY="$BINARY" \
  cargo test --test privacy_browser_acceptance \
    thor_browser_privacy_acceptance -- --ignored --exact --nocapture
