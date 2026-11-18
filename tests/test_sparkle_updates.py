"""Exercise updater packaging against real temporary application bundles."""

import base64
import importlib.util
import plistlib
import tempfile
import unittest
from pathlib import Path


MODULE = Path(__file__).resolve().parents[1] / "scripts" / "sparkle_updates.py"


class SparklePackagingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.app = self.root / "Thor.app"
        contents = self.app / "Contents"
        contents.mkdir(parents=True)
        self.plist = contents / "Info.plist"
        self.original = {"CFBundleIdentifier": "com.nirmalhk7.thor", "CFBundleVersion": "0.3.0"}
        self.plist.write_bytes(plistlib.dumps(self.original))
        self.sparkle = self.root / "sparkle"
        version = self.sparkle / "Sparkle.framework" / "Versions" / "B"
        version.mkdir(parents=True)
        (version / "Sparkle").write_bytes(b"framework fixture")
        (version.parent / "Current").symlink_to("B")
        (version.parent.parent / "Sparkle").symlink_to("Versions/Current/Sparkle")
        self.key = base64.b64encode(bytes(range(32))).decode()

    def configure(self, key=None):
        self.assertTrue(MODULE.exists(), "Updater packaging implementation missing")
        spec = importlib.util.spec_from_file_location("sparkle_updates", MODULE)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        module.configure(self.app, self.sparkle, self.key if key is None else key)

    def test_bundle_requires_signed_feed_and_preserves_framework_symlinks(self):
        self.configure()
        configured = plistlib.loads(self.plist.read_bytes())
        self.assertEqual(configured["SUPublicEDKey"], self.key)
        self.assertEqual(
            configured["SUFeedURL"],
            "https://github.com/nirmalhk7/thor/releases/latest/download/appcast.xml",
        )
        self.assertIs(configured["SURequireSignedFeed"], True)
        self.assertIs(configured["SUVerifyUpdateBeforeExtraction"], True)
        self.assertIs(configured["SUEnableAutomaticChecks"], False)
        self.assertIs(configured["SUAllowsAutomaticUpdates"], False)
        self.assertEqual(configured["CFBundleVersion"], "0.3.0")
        binary = self.app / "Contents/Frameworks/Sparkle.framework/Sparkle"
        self.assertTrue(binary.is_symlink())
        self.assertEqual(binary.read_bytes(), b"framework fixture")

    def test_invalid_public_keys_leave_bundle_untouched(self):
        for key in ("", "not base64!", base64.b64encode(b"short").decode()):
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.configure(key)
            self.assertEqual(plistlib.loads(self.plist.read_bytes()), self.original)
            self.assertFalse((self.app / "Contents/Frameworks").exists())

    def test_missing_framework_leaves_bundle_untouched(self):
        (self.sparkle / "Sparkle.framework/Versions/B/Sparkle").unlink()
        with self.assertRaises(FileNotFoundError):
            self.configure()
        self.assertEqual(plistlib.loads(self.plist.read_bytes()), self.original)

    def test_repackaging_refreshes_existing_framework(self):
        self.configure()
        (self.sparkle / "Sparkle.framework/Versions/B/Sparkle").write_bytes(b"new framework")
        self.configure()
        self.assertEqual(
            (self.app / "Contents/Frameworks/Sparkle.framework/Sparkle").read_bytes(),
            b"new framework",
        )


if __name__ == "__main__":
    unittest.main()
