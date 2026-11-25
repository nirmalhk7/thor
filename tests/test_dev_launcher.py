import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class DevLauncherTests(unittest.TestCase):
    def setUp(self):
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.root = Path(scratch.name)
        (self.root / "scripts").mkdir()
        (self.root / "bin").mkdir()
        shutil.copy2(ROOT / "Makefile", self.root)
        shutil.copy2(ROOT / "scripts/run_dev.sh", self.root / "scripts")
        self.log = self.root / "calls"
        self.env = {
            **os.environ,
            "PATH": f"{self.root / 'bin'}:{os.defpath}",
            "CALL_LOG": str(self.log),
            "THOR_DEV_SIGNING_IDENTITY": "Local Thor Signer",
        }
        self.executable("bin/cargo", """
if [ "$1" = run ]; then
    for output do :; done
    mkdir -p "$(dirname "$output")"
    cp -R fixture "$output"
fi
""")
        self.executable("bin/codesign", """
printf 'codesign %s\\n' "$*" >> "$CALL_LOG"
if [ "${FAIL_VERIFY:-}" = 1 ] && [ "$1" = --verify ]; then exit 1; fi
""")
        frameworks = self.root / "fixture/Thor.app/Contents/Frameworks"
        (frameworks / "Chromium Embedded Framework.framework").mkdir(parents=True)
        (frameworks / "Thor Helper (Renderer).app").mkdir()
        (self.root / "fixture/Thor.app/Contents/MacOS").mkdir()
        self.executable("fixture/Thor.app/Contents/MacOS/Thor", """
printf 'Thor %s\\n' "$*" >> "$CALL_LOG"
""")

    def executable(self, path, body):
        script = self.root / path
        script.write_text("#!/bin/sh\nset -e\n" + body)
        script.chmod(0o755)

    def make_dev(self):
        result = subprocess.run(
            ["make", "dev"], cwd=self.root, env=self.env, capture_output=True, text=True
        )
        calls = self.log.read_text().splitlines() if self.log.exists() else []
        return result, calls

    def test_dev_signs_bundle_and_helpers_before_launch(self):
        result, calls = self.make_dev()
        self.assertEqual(result.returncode, 0, result.stderr)
        signatures = calls[:-2]
        self.assertEqual(len(signatures), 3, calls)
        for signature, path in zip(signatures, (
            "Chromium Embedded Framework.framework",
            "Thor Helper (Renderer).app",
            "Thor.app",
        )):
            self.assertIn("--sign Local Thor Signer", signature)
            self.assertTrue(signature.endswith("/" + path), signature)
        self.assertIn("codesign --verify --deep --strict", calls[-2])
        self.assertEqual(calls[-1], "Thor --developer-mode")

    def test_dev_refuses_ad_hoc_signing(self):
        self.env["THOR_DEV_SIGNING_IDENTITY"] = "-"
        result, calls = self.make_dev()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(calls, [])
        self.assertIn("certificate", result.stderr)

    def test_dev_does_not_launch_if_signature_verification_fails(self):
        self.env["FAIL_VERIFY"] = "1"
        result, calls = self.make_dev()
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(any("--verify" in call for call in calls), calls)
        self.assertFalse(any(call.startswith("Thor ") for call in calls), calls)


if __name__ == "__main__":
    unittest.main()
