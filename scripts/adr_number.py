#!/usr/bin/env python3
"""Gives new decision records their number once they reach `main` (E00-S12).

On a branch a new record is `docs/adr/ADR-XXX-<story>-<title>.md`, mentioned elsewhere as `ADR-XXX-<story>`, so
branches never pick the same number. After a merge, the follow-up PR that marks the story done runs this on the newest
`main`: each `ADR-XXX` file gets the next number after the highest one, in name order, and every link and mention in
the repository's tracked text files follows. Its row in the index (`docs/adr/README.md`) is added here too, from the
record's own heading, Type and Status, so story branches never edit the index and never clash there (E00-S13). A row a
branch did add the old way is renamed instead. Nothing to do is not an error. Standard library only, like
`backlog.py`.

Usage: python3 scripts/adr_number.py   (from anywhere in the repository; prints what it renamed)
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ADR = ROOT / "docs" / "adr"
NUMBERED = re.compile(r"^ADR-(\d{3})-")
PENDING = re.compile(r"^ADR-XXX-(?:(E\d{2}(?:-S\d{2})?)-)?[a-z0-9-]+\.md$")
TEXT = (".md", ".rs", ".py", ".toml", ".yml", ".yaml", ".html", ".txt")


class AdrError(Exception):
    pass


# This script and its tests spell out `ADR-XXX` names as examples; they are never rewritten.
SKIP = {"scripts/adr_number.py", "scripts/test_adr_number.py"}


def tracked_text_files(root: Path) -> list[Path]:
    out = subprocess.run(["git", "ls-files"], cwd=root, capture_output=True, text=True, check=True).stdout
    return [root / name for name in out.splitlines() if name.endswith(TEXT) and name not in SKIP]


def plan(adr: Path) -> list[tuple[str, str, str | None]]:
    """(old name, new name, story) for each record waiting for a number, in name order."""
    highest = max((int(m.group(1)) for p in adr.glob("ADR-*.md") if (m := NUMBERED.match(p.name))), default=0)
    pending = sorted(p.name for p in adr.glob("ADR-XXX-*.md"))
    renames, stories = [], set()
    for offset, name in enumerate(pending, start=1):
        match = PENDING.match(name)
        if not match:
            raise AdrError(f"docs/adr/{name}: name it ADR-XXX-EXX-SYY-short-title.md")
        story = match.group(1)
        if story and story in stories:
            # Mentions are `ADR-XXX-<story>`; two new records from one story couldn't be told apart.
            raise AdrError(f"two new ADRs for {story}: mentions of ADR-XXX-{story} would be ambiguous")
        if story:
            stories.add(story)
        number = f"{highest + offset:03d}"
        if int(number) > 999:
            raise AdrError("ADR numbers have run past 999")
        renames.append((name, name.replace("ADR-XXX-", f"ADR-{number}-", 1), story))
    return renames


def apply(root: Path) -> list[tuple[str, str]]:
    adr = root / "docs" / "adr"
    renames = plan(adr)
    for old, new, _ in renames:
        subprocess.run(["git", "mv", str(adr / old), str(adr / new)], cwd=root, check=True)
    for path in tracked_text_files(root):
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        updated = text
        for old, new, story in renames:
            number = new[: len("ADR-000")]
            updated = updated.replace(f"[ADR-XXX]({old})", f"[{number}]({new})")
            updated = updated.replace(old, new)
            if path.name == new:
                updated = re.sub(r"^# ADR-XXX\. ", f"# {number}. ", updated, count=1, flags=re.M)
            if story:
                updated = re.sub(rf"\bADR-XXX-{story}(?![\w-])", number, updated)
        if updated != text:
            path.write_text(updated, encoding="utf-8")
    add_index_rows(adr, [new for _, new, _ in renames])
    return [(old, new) for old, new, _ in renames]


def field(record: str, name: str) -> str:
    """The value of a record's `- **<name>:** value` line, or "?" when it has none."""
    match = re.search(rf"^- \*\*{name}:\*\* *(.+?) *$", record, flags=re.M)
    return match.group(1) if match else "?"


def index_row(adr: Path, name: str) -> str:
    """The index row of numbered record `name`, from its heading, Type and Status."""
    record = (adr / name).read_text(encoding="utf-8")
    number = name[: len("ADR-000")]
    heading = re.search(r"^# ADR-\d{3}\. *(.+?) *$", record, flags=re.M)
    title = heading.group(1) if heading else name
    return f"| [{number}]({name}) | {title} | {field(record, 'Type')} | {field(record, 'Status')} |"


def add_index_rows(adr: Path, names: list[str]) -> None:
    """Adds a row for each of `names` the index doesn't list yet, and keeps the rows in number order."""
    index = adr / "README.md"
    if not index.is_file():
        return
    lines = index.read_text(encoding="utf-8").splitlines()
    rows = [i for i, line in enumerate(lines) if line.startswith("| [ADR-")]
    if not rows:
        return
    missing = [index_row(adr, name) for name in names if not any(f"]({name})" in line for line in lines)]
    at = rows[-1] + 1
    lines[at:at] = missing
    # Rows stay in number order, wherever a row added the old way was renamed.
    rows = [i for i, line in enumerate(lines) if line.startswith("| [ADR-")]
    for i, line in zip(rows, sorted((lines[i] for i in rows), key=lambda line: line[len("| [ADR-") :])):
        lines[i] = line
    index.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    # It takes no options: anything on the command line (`--help`, a typo) prints this help and changes nothing.
    if len(sys.argv) > 1:
        print(__doc__)
        return 0
    try:
        done = apply(ROOT)
    except AdrError as exc:
        print(exc, file=sys.stderr)
        return 1
    for old, new in done:
        print(f"{old} -> {new}")
    if not done:
        print("no ADRs waiting for a number")
    return 0


if __name__ == "__main__":
    sys.exit(main())
