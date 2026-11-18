"""Exercise repository conventions through the actual checker CLI."""

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


CHECKER = Path(__file__).resolve().parents[1] / "scripts/check_structure.py"


class RepositoryStructureTests(unittest.TestCase):
    def check_tree(self, files):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, content in files.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            return subprocess.run(
                [sys.executable, str(CHECKER), str(root)],
                capture_output=True, text=True, check=False,
            )

    def test_accepts_directory_owned_modules_and_leaf_modules(self):
        result = self.check_tree({
            "src/lib.rs": "pub mod browser;",
            "src/browser/mod.rs": "mod session;",
            "src/browser/session.rs": "",
            "crates/helper/src/lib.rs": "",
        })
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_module_entry_outside_its_implementation_directory(self):
        result = self.check_tree({
            "src/lib.rs": "mod browser;",
            "src/browser.rs": "mod session;",
            "src/browser/session.rs": "",
        })
        self.assertEqual(result.returncode, 1)
        self.assertIn("src/browser.rs: move module entry to src/browser/mod.rs", result.stdout)

    def test_rejects_rust_filename_that_needs_a_path_override(self):
        result = self.check_tree({"src/dx/3d_dom_snapshot.rs": ""})
        self.assertEqual(result.returncode, 1)
        self.assertIn("3d_dom_snapshot.rs: use a snake_case Rust filename", result.stdout)

    def test_checks_workspace_crates_without_scanning_build_artifacts(self):
        result = self.check_tree({
            "crates/helper/src/BadName.rs": "",
            "target/generated/BadName.rs": "",
            "legacy/shell/BadName.rs": "",
        })
        self.assertEqual(result.returncode, 1)
        self.assertIn("crates/helper/src/BadName.rs", result.stdout)
        self.assertNotIn("target/", result.stdout)
        self.assertNotIn("legacy/", result.stdout)

    def test_rejects_root_scratch_rust_but_accepts_build_script(self):
        result = self.check_tree({"scratch.rs": "", "build.rs": ""})
        self.assertEqual(result.returncode, 1)
        self.assertIn("scratch.rs: place Rust code in src/, tests/, benches/, or examples/", result.stdout)
        self.assertNotIn("build.rs:", result.stdout)

    def test_rejects_source_that_no_parent_module_declares(self):
        result = self.check_tree({
            "src/lib.rs": "// mod retired;\n",
            "src/retired/mod.rs": "mod child;",
            "src/retired/child.rs": "",
        })
        self.assertEqual(result.returncode, 1)
        self.assertIn("src/retired/mod.rs: no parent declares mod retired", result.stdout)

    def test_accepts_cfg_modules_visibility_and_binary_entry_points(self):
        result = self.check_tree({
            "src/main.rs": "#[cfg(unix)]\npub(crate) mod platform;",
            "src/platform.rs": "",
            "src/bin/labs.rs": "",
            "src/bin/packager/main.rs": "mod assets;",
            "src/bin/packager/assets.rs": "",
        })
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_commented_declarations_do_not_register_orphan_sources(self):
        for source in [
            "/*\nmod retired;\n*/\n",
            "/* outer /* nested */\nmod retired;\n*/\n",
            'const EXAMPLE: &str = "\nmod retired;\n";\n',
            'const EXAMPLE: &str = r##"\nmod retired;\n"##;\n',
        ]:
            with self.subTest(source=source):
                result = self.check_tree({
                    "src/lib.rs": source,
                    "src/retired.rs": "",
                })
                self.assertEqual(result.returncode, 1, result.stdout)
                self.assertIn("src/retired.rs: no parent declares mod retired", result.stdout)

    def test_rust_literals_and_comments_leave_real_module_declarations_visible(self):
        result = self.check_tree({
            "src/lib.rs": (
                "const QUOTE: char = '\"';\n"
                "const ESCAPED: char = '\\\\';\n"
                "const RAW: &str = r##\"a \" quoted value\"##;\n"
                "// An ignored declaration: mod other;\n"
                "/* outer /* nested */ comment */\n"
                "pub mod browser;\n"
            ),
            "src/browser.rs": "",
        })
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_diagnostic_programs_are_separate_from_product_binaries(self):
        result = self.check_tree({
            "src/main.rs": "fn main() {}",
            "src/bin/test_bookmarks.rs": "fn main() {}",
            "examples/bookmark_storage_probe.rs": "fn main() {}",
        })
        self.assertEqual(result.returncode, 1)
        self.assertIn("src/bin/test_bookmarks.rs: place diagnostic programs in examples/", result.stdout)


if __name__ == "__main__":
    unittest.main()
