import unittest

from scripts.semantic_version import (
    bump_level,
    next_version,
    package_version,
    release_plan,
    replace_lock_version,
    replace_package_version,
)


class SemanticVersionTests(unittest.TestCase):
    def test_commit_priority(self):
        self.assertEqual(bump_level(["fix: repair", "feat(browser): add tabs"]), "minor")
        self.assertEqual(bump_level(["feat(browser)!: replace engine"]), "major")
        self.assertEqual(bump_level(["fix: change\n\nBREAKING CHANGE: API changed"]), "major")
        self.assertEqual(bump_level(["new features"]), "patch")
        self.assertIsNone(bump_level(["chore(release): bump version to v0.1.4 [skip ci]"]))

    def test_semantic_increments(self):
        self.assertEqual(next_version("0.1.4", "patch"), "0.1.5")
        self.assertEqual(next_version("0.1.4", "minor"), "0.2.0")
        self.assertEqual(next_version("0.1.4", "major"), "1.0.0")

    def test_release_plan_recovers_unpublished_version(self):
        self.assertEqual(
            release_plan("0.2.0", "0.1.4", [], "chore(release): bump version to v0.2.0 [skip ci]"),
            "0.2.0",
        )
        with self.assertRaises(ValueError):
            release_plan("0.2.0", "0.1.4", [], "feat: other work")

    def test_package_files_change_only_thor_version(self):
        manifest = '[package]\nname = "thor"\nversion = "0.1.4"\n\n[dependencies]\nversion = "9"\n'
        lock = '[[package]]\nname = "other"\nversion = "1.0.0"\n\n[[package]]\nname = "thor"\nversion = "0.1.4"\n'
        self.assertEqual(package_version(manifest), "0.1.4")
        self.assertIn('[dependencies]\nversion = "9"', replace_package_version(manifest, "0.2.0"))
        self.assertIn('name = "thor"\nversion = "0.2.0"', replace_lock_version(lock, "0.2.0"))
        self.assertIn('name = "other"\nversion = "1.0.0"', replace_lock_version(lock, "0.2.0"))


if __name__ == "__main__":
    unittest.main()
