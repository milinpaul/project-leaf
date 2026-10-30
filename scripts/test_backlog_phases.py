"""Tests for phases on the board and moving epics between them (E00-S10).

Like test_backlog.py, each test works on a throwaway copy of `scripts/` and `backlog/`.

Run: python3 -m unittest discover -s scripts -p 'test_*.py'
"""
from __future__ import annotations

import json
import socket
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.error
import urllib.request
from pathlib import Path

from test_backlog import copy_backlog, run


def backlog_json(root: Path) -> dict:
    result = run(root)
    assert result.returncode == 0, result.stderr
    return json.loads((root / "backlog" / "backlog.json").read_text(encoding="utf-8"))


def epic_phase(root: Path, epic: str) -> str:
    text = (root / "backlog" / "epics" / f"{epic}.md").read_text(encoding="utf-8")
    return next(line.split(":", 1)[1].strip() for line in text.splitlines() if line.startswith("phase:"))


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class PhasesTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        copy_backlog(self.root)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_every_epic_is_in_exactly_one_phase_with_its_stories_counted(self) -> None:
        data = backlog_json(self.root)
        listed = [e for p in data["phases"] for e in p["epics"]]
        self.assertEqual(sorted(listed), sorted(e["id"] for e in data["epics"]))
        self.assertEqual(sum(p["story_count"] for p in data["phases"]), len(data["stories"]))
        self.assertEqual(sum(p["done_count"] for p in data["phases"]),
                         sum(s["status"] == "done" for s in data["stories"]))

    def test_the_current_phase_is_the_first_with_unfinished_stories(self) -> None:
        phases = backlog_json(self.root)["phases"]
        current = [p for p in phases if p["current"]]
        self.assertEqual(len(current), 1)
        for p in phases[: phases.index(current[0])]:
            self.assertEqual(p["done_count"], p["story_count"], f"phase {p['id']} is unfinished")
        self.assertLess(current[0]["done_count"], current[0]["story_count"])

    def test_moving_an_epic_takes_its_stories_to_the_new_phase(self) -> None:
        before = backlog_json(self.root)
        epic = next(e for e in before["epics"] if e["phase"] != 0 and e["story_count"])
        result = run(self.root, "phase", epic["id"], "0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(epic_phase(self.root, epic["id"]), "0")
        after = backlog_json(self.root)
        self.assertIn(epic["id"], after["phases"][0]["epics"])
        self.assertEqual(after["phases"][0]["story_count"], before["phases"][0]["story_count"] + epic["story_count"])

    def test_an_unknown_phase_or_epic_changes_nothing(self) -> None:
        for args, message in ((("E00", "9"), "bad phase"), (("E99", "1"), "no epic"), (("../x", "1"), "no epic")):
            result = run(self.root, "phase", *args)
            self.assertEqual(result.returncode, 1)
            self.assertIn(message, result.stderr)
        self.assertEqual(epic_phase(self.root, "E00"), "0")

    def test_check_fails_on_an_epic_with_a_bad_phase(self) -> None:
        path = self.root / "backlog" / "epics" / "E00.md"
        path.write_text(path.read_text(encoding="utf-8").replace("\nphase: 0", "\nphase: soon", 1), encoding="utf-8")
        result = run(self.root, "--check")
        self.assertEqual(result.returncode, 1)
        self.assertIn("E00.md: bad phase", result.stderr)


class BoardServerTest(unittest.TestCase):
    """The live board's POST /epic-phase: only the board page itself may move an epic."""

    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        copy_backlog(self.root)
        self.port = free_port()
        self.url = f"http://127.0.0.1:{self.port}"
        self.server = subprocess.Popen(
            [sys.executable, str(self.root / "scripts" / "backlog.py"), "serve", str(self.port)],
            cwd=self.root, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        for _ in range(100):
            try:
                urllib.request.urlopen(f"{self.url}/version", timeout=1).close()
                break
            except OSError:
                time.sleep(0.05)
        else:
            self.fail("the board server didn't start")

    def tearDown(self) -> None:
        self.server.terminate()
        self.server.wait(timeout=5)
        self.tmp.cleanup()

    def post(self, body: bytes, origin: str | None = None, kind: str = "application/json") -> int:
        headers = {"Content-Type": kind, "Origin": origin if origin is not None else self.url}
        request = urllib.request.Request(f"{self.url}/epic-phase", data=body, headers=headers, method="POST")
        try:
            with urllib.request.urlopen(request, timeout=5) as response:
                return response.status
        except urllib.error.HTTPError as exc:
            return exc.code

    def test_the_board_page_can_move_an_epic(self) -> None:
        self.assertEqual(self.post(b'{"epic": "E14", "phase": 2}'), 204)
        self.assertEqual(epic_phase(self.root, "E14"), "2")
        served = json.loads(urllib.request.urlopen(f"{self.url}/backlog.json", timeout=5).read())
        self.assertIn("E14", served["phases"][2]["epics"])

    def test_other_sites_and_plain_forms_are_refused(self) -> None:
        self.assertEqual(self.post(b'{"epic": "E14", "phase": 2}', origin="http://example.com"), 403)
        self.assertEqual(self.post(b'{"epic": "E14", "phase": 2}', origin=""), 403)
        self.assertEqual(self.post(b"epic=E14&phase=2", kind="application/x-www-form-urlencoded"), 403)
        self.assertEqual(epic_phase(self.root, "E14"), "3")

    def test_bad_requests_change_nothing(self) -> None:
        for body in (b'{"epic": "E14", "phase": 7}', b'{"epic": "E99", "phase": 1}', b"not json"):
            self.assertEqual(self.post(body), 400, body)
        self.assertEqual(epic_phase(self.root, "E14"), "3")


if __name__ == "__main__":
    unittest.main()
