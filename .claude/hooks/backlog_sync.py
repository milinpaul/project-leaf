#!/usr/bin/env python3
"""PostToolUse hook: after any edit to a backlog markdown file, validate the backlog
and regenerate backlog.json + board.html. On validation errors, exit 2 so Claude
sees the message and fixes the file."""
import json
import os
import subprocess
import sys
from pathlib import Path

try:
    event = json.load(sys.stdin)
except json.JSONDecodeError:
    sys.exit(0)

path = Path(str(event.get("tool_input", {}).get("file_path", "")))
parts = path.parts
if path.suffix != ".md" or "backlog" not in parts or path.name == "README.md":
    sys.exit(0)

root = Path(os.environ.get("CLAUDE_PROJECT_DIR", "."))
result = subprocess.run(
    [sys.executable, str(root / "scripts" / "backlog.py")],
    capture_output=True, text=True, cwd=root,
)
if result.returncode != 0:
    print(result.stderr or result.stdout, file=sys.stderr)
    sys.exit(2)
sys.exit(0)
