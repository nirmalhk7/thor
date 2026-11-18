#!/usr/bin/env python3
"""Embed Sparkle and its public verification key in a staged Thor bundle."""

import argparse
import base64
import binascii
import plistlib
import shutil
from pathlib import Path


FEED_URL = "https://github.com/nirmalhk7/thor/releases/latest/download/appcast.xml"


def configure(app: Path, sparkle: Path, public_key: str) -> None:
    try:
        decoded = base64.b64decode(public_key, validate=True)
    except (ValueError, binascii.Error) as error:
        raise ValueError("Update public key must be base64-encoded Ed25519") from error
    if len(decoded) != 32:
        raise ValueError("Update public key must contain exactly 32 bytes")
    framework = sparkle / "Sparkle.framework"
    if not (framework / "Sparkle").is_file():
        raise FileNotFoundError("Sparkle framework executable missing")
    plist = app / "Contents" / "Info.plist"
    info = plistlib.loads(plist.read_bytes())
    destination = app / "Contents" / "Frameworks" / "Sparkle.framework"
    if destination.exists():
        shutil.rmtree(destination)
    shutil.copytree(framework, destination, symlinks=True)
    info.update({
        "SUFeedURL": FEED_URL,
        "SUPublicEDKey": public_key,
        "SURequireSignedFeed": True,
        "SUVerifyUpdateBeforeExtraction": True,
        "SUEnableAutomaticChecks": False,
        "SUAllowsAutomaticUpdates": False,
        "SUShowReleaseNotes": False,
    })
    plist.write_bytes(plistlib.dumps(info))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--sparkle-dir", type=Path, required=True)
    parser.add_argument("--public-key", required=True)
    args = parser.parse_args()
    configure(args.app, args.sparkle_dir, args.public_key)


if __name__ == "__main__":
    main()
