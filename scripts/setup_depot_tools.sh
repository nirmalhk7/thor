#!/usr/bin/env bash
set -euo pipefail

echo "Disabled: Thor now ships as an embedded CEF application; depot_tools is not used."
echo "Build the CEF product with: cargo build --locked --release --features cef-bundle --bin Thor --bin thor-labs"
exit 1
