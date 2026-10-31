"""Tests for scripts/adr_number.py (E00-S12): new decision records get their number after merging.

Each test runs the real script in a throwaway git repository, so the real `docs/adr` is never touched.

Run: python3 -m unittest discover -s scripts -p 'test_*.py'
"""
from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


def git(root: Path, *args: str) -> None:
    subprocess.run(
        ["git", "-c", "user.name=test", "-c", "user.email=test@example.com", *args],
        cwd=root, capture_output=True, text=True, check=True,
    )


class AdrNumberTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        (self.root / "scripts").mkdir()
        shutil.copy(REPO / "scripts" / "adr_number.py", self.root / "scripts")
        self.adr = self.root / "docs" / "adr"
        self.adr.mkdir(parents=True)
        self.write("docs/adr/ADR-000-EXX-SYY-template.md", "# ADR-NNN. Title\n")
        self.write("docs/adr/ADR-074-E08-S13-old.md", "# ADR-074. Old\n")
        self.write("docs/adr/ADR-075-E22-S01-views.md", "# ADR-075. Views\n")
        git(self.root, "init", "-q")

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def write(self, name: str, text: str) -> None:
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def read(self, name: str) -> str:
        return (self.root / name).read_text(encoding="utf-8")

    def run_script(self) -> subprocess.CompletedProcess:
        git(self.root, "add", "-A")
        return subprocess.run(
            [sys.executable, str(self.root / "scripts" / "adr_number.py")],
            cwd=self.root, capture_output=True, text=True,
        )

    def test_new_records_get_the_next_numbers_and_every_mention_follows(self) -> None:
        self.write("docs/adr/ADR-XXX-E08-S08-bundled-code-font.md", "# ADR-XXX. Bundled code font\n")
        self.write("docs/adr/ADR-XXX-E11-search.md", "# ADR-XXX. Search\n\nSee ADR-XXX-E08-S08.\n")
        self.write(
            "docs/adr/README.md",
            "| [ADR-075](ADR-075-E22-S01-views.md) |\n"
            "| [ADR-XXX](ADR-XXX-E08-S08-bundled-code-font.md) |\n"
            "| [ADR-XXX](ADR-XXX-E11-search.md) |\n",
        )
        self.write("crates/theme.rs", "/// The code face (E08-S08, ADR-XXX-E08-S08). Not ADR-XXX-E11-S02.\n")
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.adr / "ADR-076-E08-S08-bundled-code-font.md").exists())
        self.assertTrue((self.adr / "ADR-077-E11-search.md").exists())
        self.assertFalse(list(self.adr.glob("ADR-XXX-*")))
        self.assertEqual(self.read("docs/adr/ADR-076-E08-S08-bundled-code-font.md"), "# ADR-076. Bundled code font\n")
        self.assertEqual(self.read("docs/adr/ADR-077-E11-search.md"), "# ADR-077. Search\n\nSee ADR-076.\n")
        self.assertIn("| [ADR-076](ADR-076-E08-S08-bundled-code-font.md) |", self.read("docs/adr/README.md"))
        self.assertIn("| [ADR-077](ADR-077-E11-search.md) |", self.read("docs/adr/README.md"))
        # An epic's mention doesn't swallow a longer story ID that only starts the same way.
        self.assertEqual(
            self.read("crates/theme.rs"),
            "/// The code face (E08-S08, ADR-076). Not ADR-XXX-E11-S02.\n",
        )

    def test_nothing_waiting_is_not_an_error(self) -> None:
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("no ADRs waiting", result.stdout)

    def test_two_new_records_for_one_story_are_refused(self) -> None:
        self.write("docs/adr/ADR-XXX-E09-S05-tabs.md", "# ADR-XXX. Tabs\n")
        self.write("docs/adr/ADR-XXX-E09-S05-board.md", "# ADR-XXX. Board\n")
        result = self.run_script()
        self.assertEqual(result.returncode, 1)
        self.assertIn("two new ADRs for E09-S05", result.stderr)
        self.assertTrue((self.adr / "ADR-XXX-E09-S05-tabs.md").exists(), "nothing is renamed")


if __name__ == "__main__":
    unittest.main()
