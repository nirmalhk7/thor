"""Run the release signer with disposable keys and actual Sparkle tools."""

import os
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SIGNER = Path(__file__).resolve().parents[1] / "scripts/sign_update.sh"
SPARKLE = os.environ.get("SPARKLE_DIR")


@unittest.skipUnless(SPARKLE, "Set SPARKLE_DIR to run native signing integration tests")
class ReleaseSigningTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.version = "0.3.0"
        self.dist = self.root / "dist"
        self.dist.mkdir()
        self.private, public = self.keypair()
        self.app = self.root / "Thor.app"
        contents = self.app / "Contents"
        (contents / "MacOS").mkdir(parents=True)
        shutil.copy("/usr/bin/true", contents / "MacOS/Thor")
        info = {
            "CFBundleExecutable": "Thor",
            "CFBundleIdentifier": "com.nirmalhk7.thor",
            "CFBundleName": "Thor",
            "CFBundlePackageType": "APPL",
            "CFBundleVersion": self.version,
            "CFBundleShortVersionString": self.version,
            "LSMinimumSystemVersion": "12.0",
        }
        (contents / "Info.plist").write_bytes(plistlib.dumps(info))
        # Use the same packaging implementation as the real release pipeline.
        subprocess.run([
            sys.executable, str(SIGNER.with_name("sparkle_updates.py")),
            "--app", str(self.app), "--sparkle-dir", SPARKLE, "--public-key", public,
        ], check=True)
        subprocess.run(["codesign", "--force", "--sign", "-", str(self.app)], check=True, capture_output=True)
        self.archive = self.dist / f"Thor-v{self.version}-macos.zip"
        subprocess.run(["ditto", "-c", "-k", "--keepParent", self.app, self.archive], check=True)

    def keypair(self):
        output = subprocess.check_output(["/usr/bin/swift", "-e", """
import CryptoKit
import Foundation
let key = Curve25519.Signing.PrivateKey()
print(key.rawRepresentation.base64EncodedString())
print(key.publicKey.rawRepresentation.base64EncodedString())
"""], text=True)
        return output.splitlines()

    def sign(self, private=None):
        self.assertTrue(SIGNER.exists(), "Release signer implementation missing")
        return subprocess.run(
            ["bash", SIGNER, self.version], cwd=self.root,
            env={**os.environ, "THOR_UPDATE_PRIVATE_KEY": private or self.private},
            capture_output=True, text=True,
        )

    def test_release_feed_is_signed_and_detects_tampering(self):
        result = self.sign()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        feed = self.dist / "appcast.xml"
        original = feed.read_text()
        self.assertIn("https://github.com/nirmalhk7/thor/releases/download/v0.3.0/Thor-v0.3.0-macos.zip", original)
        # Check the signature, then change the download URL without resigning.
        verifier = [str(Path(SPARKLE) / "bin/sign_update"), "--verify", "--ed-key-file", "-", str(feed)]
        verified = subprocess.run(verifier, input=self.private, capture_output=True, text=True)
        self.assertEqual(verified.returncode, 0, verified.stdout + verified.stderr)
        feed.write_text(original.replace("Thor-v0.3.0-macos.zip", "untrusted.zip"))
        tampered = subprocess.run(verifier, input=self.private, capture_output=True, text=True)
        self.assertNotEqual(tampered.returncode, 0)

    def test_mismatched_signing_key_does_not_publish_feed(self):
        different_private, _ = self.keypair()
        result = self.sign(different_private)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.dist / "appcast.xml").exists())


if __name__ == "__main__":
    unittest.main()
