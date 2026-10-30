"""Tests for scripts/macos_app.py (E08-S05, E08-S07). Standard library only, like the script.

Everything happens in a temporary folder with a fake binary, so the tests run on any OS and never touch the real
/Applications.

Run: python3 -m unittest discover -s scripts -p 'test_*.py'
"""
from __future__ import annotations

import plistlib
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "macos_app.py"
sys.path.insert(0, str(SCRIPT.parent))

import macos_app  # noqa: E402


def run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(SCRIPT), *args], capture_output=True, text=True)


def write_app(path: Path, bundle_id: str) -> None:
    """Writes a minimal bundle at `path` with the given bundle ID and one marker file."""
    (path / "Contents").mkdir(parents=True)
    with open(path / "Contents" / "Info.plist", "wb") as f:
        plistlib.dump({"CFBundleIdentifier": bundle_id}, f)
    (path / "Contents" / "marker").write_text("old")


class MacosAppTest(unittest.TestCase):
    def setUp(self) -> None:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.build = self.root / "target" / "release"
        self.build.mkdir(parents=True)
        self.binary = self.build / "leaf-app"
        self.binary.write_text("new binary")
        self.apps = self.root / "Applications"
        self.apps.mkdir()

    def test_bundle_mode_writes_leaf_app(self) -> None:
        out = self.root / "out"
        result = run(str(self.binary), str(out))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(macos_app.bundle_id(out / "Leaf.app"), macos_app.APP_ID)
        self.assertEqual((out / "Leaf.app" / "Contents" / "MacOS" / "Leaf").read_text(), "new binary")
        self.assertTrue((out / "Leaf.app" / "Contents" / "Resources" / "Leaf.icns").is_file())

    def test_install_copies_bundle_into_apps_dir(self) -> None:
        result = run("install", str(self.binary), str(self.apps))
        self.assertEqual(result.returncode, 0, result.stderr)
        installed = self.apps / "Leaf.app"
        self.assertEqual(macos_app.bundle_id(installed), macos_app.APP_ID)
        self.assertEqual((installed / "Contents" / "MacOS" / "Leaf").read_text(), "new binary")
        self.assertIn(str(installed), result.stdout)

    def test_install_replaces_older_leaf(self) -> None:
        write_app(self.apps / "Leaf.app", macos_app.APP_ID)
        result = run("install", str(self.binary), str(self.apps))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.apps / "Leaf.app" / "Contents" / "marker").exists())
        self.assertEqual((self.apps / "Leaf.app" / "Contents" / "MacOS" / "Leaf").read_text(), "new binary")

    def test_install_refuses_other_apps_called_leaf(self) -> None:
        write_app(self.apps / "Leaf.app", "com.example.SomeOtherLeaf")
        result = run("install", str(self.binary), str(self.apps))
        self.assertEqual(result.returncode, 1)
        self.assertIn("com.example.SomeOtherLeaf", result.stderr)
        self.assertEqual((self.apps / "Leaf.app" / "Contents" / "marker").read_text(), "old")

    def test_install_refuses_unreadable_bundle(self) -> None:
        broken = self.apps / "Leaf.app" / "Contents"
        broken.mkdir(parents=True)
        (broken / "Info.plist").write_text("not a plist")
        result = run("install", str(self.binary), str(self.apps))
        self.assertEqual(result.returncode, 1)
        self.assertIn("unknown", result.stderr)
        self.assertTrue((broken / "Info.plist").exists())

    def test_install_replaces_symlink_to_leaf(self) -> None:
        elsewhere = self.root / "elsewhere" / "Leaf.app"
        write_app(elsewhere, macos_app.APP_ID)
        (self.apps / "Leaf.app").symlink_to(elsewhere)
        result = run("install", str(self.binary), str(self.apps))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.apps / "Leaf.app").is_symlink())
        self.assertEqual((elsewhere / "Contents" / "marker").read_text(), "old")

    def test_wrong_arguments_print_usage(self) -> None:
        result = run("only-one")
        self.assertEqual(result.returncode, 2)
        self.assertIn("Usage", result.stderr)


if __name__ == "__main__":
    unittest.main()
