#!/usr/bin/env bash
# Fetch the pinned upstream updater; never execute an unverified download.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/target/sparkle-2.10.1"
if [[ ! -x "$DEST/bin/generate_appcast" ]]; then
  ARCHIVE="$(mktemp -t thor-sparkle)"
  trap 'rm -f "$ARCHIVE"' EXIT
  curl --fail --location --proto '=https' --proto-redir '=https' --tlsv1.2 \
    https://github.com/sparkle-project/Sparkle/releases/download/2.10.1/Sparkle-2.10.1.tar.xz \
    --output "$ARCHIVE"
  printf '%s  %s\n' '1a856d2a9edb8e628de88121d7be04cf0e2167f8641650b78fca00b14244cf27' "$ARCHIVE" \
    | shasum -a 256 -c - >&2
  mkdir -p "$DEST"
  tar -xJf "$ARCHIVE" -C "$DEST"
fi
printf '%s\n' "$DEST"
