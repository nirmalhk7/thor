#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ "${1:-}" != --once ]] && command -v cargo-watch >/dev/null; then
  exec cargo watch -w src -s './scripts/run_dev.sh --once'
fi

# Keychain permissions follow the signing certificate across rebuilds.
identity="${THOR_DEV_SIGNING_IDENTITY:-Thor Local Development}"
if [[ "$identity" == - ]]; then
  echo "Thor dev requires a code-signing certificate. Set THOR_DEV_SIGNING_IDENTITY to its name or hash." >&2
  exit 1
fi

destination=dist/cef-dev
app="$destination/Thor.app"
frameworks="$app/Contents/Frameworks"
cargo build --locked --release --features cef-bundle --bin Thor --bin thor-labs --bin thor-package
cargo run --locked --release --features cef-bundle --bin thor-package -- "$destination"

for component in "$frameworks/Chromium Embedded Framework.framework" "$frameworks"/Thor\ Helper*.app "$app"; do
  if ! codesign --force --sign "$identity" --timestamp=none "$component"; then
    echo "Use Keychain Access to create a Code Signing certificate named Thor Local Development, or set THOR_DEV_SIGNING_IDENTITY to an existing signing identity." >&2
    exit 1
  fi
done
codesign --verify --deep --strict "$app"

exec "$app/Contents/MacOS/Thor" --developer-mode
