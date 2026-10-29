"""Tests for scripts/backlog.py (E00-S09). Standard library only, like the script.

Each test runs the real script from a throwaway copy of `scripts/` and `backlog/`, so the
repository's own backlog is never touched.

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


def copy_backlog(dest: Path) -> None:
    """Copies the script, the backlog markdown and .gitignore into `dest`, leaving out generated files."""
    shutil.copytree(REPO / "scripts", dest / "scripts", ignore=shutil.ignore_patterns("__pycache__", "test_*"))
    shutil.copytree(REPO / "backlog", dest / "backlog", ignore=shutil.ignore_patterns("backlog.json", "board.html"))
    shutil.copy(REPO / ".gitignore", dest / ".gitignore")


def run(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(root / "scripts" / "backlog.py"), *args],
        cwd=root, capture_output=True, text=True,
    )


def git(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-c", "user.name=test", "-c", "user.email=test@example.com", *args],
        cwd=root, capture_output=True, text=True,
    )


def two_stories(root: Path) -> tuple[str, str]:
    """IDs of two different stories."""
    stories = sorted((root / "backlog" / "stories").glob("*.md"))
    return stories[0].stem, stories[1].stem


def linked_stories(root: Path) -> tuple[str, str]:
    """A story that isn't done yet and another story that depends on it."""
    status, deps = {}, {}
    for path in sorted((root / "backlog" / "stories").glob("*.md")):
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.startswith("status: "):
                status[path.stem] = line.removeprefix("status: ").strip()
            elif line.startswith("depends_on: "):
                deps[path.stem] = [d.strip() for d in line.split("[", 1)[1].rstrip("]").split(",") if d.strip()]
    for story, needs in deps.items():
        for dep in needs:
            if status.get(dep) not in (None, "done"):
                return dep, story
    raise unittest.SkipTest("no unfinished story with a dependent")


class CheckTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        copy_backlog(self.root)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_check_passes_on_a_valid_backlog_and_writes_nothing(self) -> None:
        result = run(self.root, "--check")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.root / "backlog" / "backlog.json").exists())
        self.assertFalse((self.root / "backlog" / "board.html").exists())

    def test_check_fails_on_an_invalid_story(self) -> None:
        story, _ = two_stories(self.root)
        path = self.root / "backlog" / "stories" / f"{story}.md"
        path.write_text(path.read_text(encoding="utf-8").replace("\nstatus: ", "\nstatus: nonsense-", 1), encoding="utf-8")
        result = run(self.root, "--check")
        self.assertEqual(result.returncode, 1)
        self.assertIn(story, result.stderr)


@unittest.skipUnless(shutil.which("git"), "needs git")
class ParallelBranchesTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        copy_backlog(self.root)
        for args in (("init", "-q", "-b", "main"), ("add", "-A"), ("commit", "-q", "-m", "base")):
            result = git(self.root, *args)
            self.assertEqual(result.returncode, 0, result.stderr)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_parallel_branches_touching_linked_stories_merge_cleanly(self) -> None:
        # The case that used to conflict (seen merging main into PR #38): one branch marks a story
        # done, which changes the generated `waiting_on`/`ready` of a story that depends on it,
        # while another branch links a PR to that dependent story, on the lines right next to them.
        done, dependent = linked_stories(self.root)
        git(self.root, "checkout", "-q", "-b", "a", "main")
        self.assert_ok(run(self.root, "move", done, "done", "merged"))
        self.assertTrue((self.root / "backlog" / "backlog.json").exists(), "commands still write it locally")
        git(self.root, "commit", "-q", "-am", f"{done} done")
        git(self.root, "checkout", "-q", "-b", "b", "main")
        self.assert_ok(run(self.root, "pr", dependent, "https://github.com/owner/repo/pull/9"))
        git(self.root, "commit", "-q", "-am", f"{dependent} PR")

        git(self.root, "checkout", "-q", "main")
        for branch in ("a", "b"):
            merged = git(self.root, "merge", "-q", "--no-edit", branch)
            self.assertEqual(merged.returncode, 0, merged.stdout + merged.stderr)

        tracked = git(self.root, "ls-files", "backlog/backlog.json", "backlog/board.html").stdout
        self.assertEqual(tracked, "", "generated files must never be committed")
        self.assert_ok(run(self.root, "--check"))
        stories = self.root / "backlog" / "stories"
        self.assertIn("status: done", (stories / f"{done}.md").read_text(encoding="utf-8"))
        self.assertIn("pull/9", (stories / f"{dependent}.md").read_text(encoding="utf-8"))

    def assert_ok(self, result: subprocess.CompletedProcess) -> None:
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
