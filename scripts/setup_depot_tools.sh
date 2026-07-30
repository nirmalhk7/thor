#!/usr/bin/env bash
# Install Google depot_tools for Chromium fetch/build.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEPOT_TOOLS="${THOR_DEPOT_TOOLS:-$HOME/depot_tools}"

if [[ -d "$DEPOT_TOOLS/.git" ]]; then
  echo "✓ depot_tools already at $DEPOT_TOOLS"
  git -C "$DEPOT_TOOLS" pull --ff-only || true
else
  echo "→ Cloning depot_tools into $DEPOT_TOOLS"
  git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git "$DEPOT_TOOLS"
fi

# shellcheck disable=SC2016
MARKER='export PATH="$HOME/depot_tools:$PATH"'
if ! grep -q 'depot_tools' "${HOME}/.zprofile" 2>/dev/null && ! grep -q 'depot_tools' "${HOME}/.zshrc" 2>/dev/null; then
  echo "→ Add to your shell PATH: export PATH=\"$DEPOT_TOOLS:\$PATH\""
fi

export PATH="$DEPOT_TOOLS:$PATH"
echo "→ Ensuring cipd / gclient available..."
command -v gclient >/dev/null
command -v fetch >/dev/null
echo "✓ depot_tools ready"
echo "  PATH prefix: $DEPOT_TOOLS"
echo "  Next: ./scripts/fetch_chromium.sh"
