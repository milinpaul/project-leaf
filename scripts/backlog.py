#!/usr/bin/env python3
"""Backlog tool: validates backlog/*.md and generates backlog/backlog.json.

The markdown files are the source of truth. backlog.json is generated for the
control panel and must never be edited by hand.

Usage:
  python3 scripts/backlog.py            # validate + write backlog.json
  python3 scripts/backlog.py --check    # validate + fail if backlog.json is stale (CI)
  python3 scripts/backlog.py next       # print the next ready story for an agent
  python3 scripts/backlog.py summary    # counts per status
  python3 scripts/backlog.py session    # context summary (used by the SessionStart hook)
  python3 scripts/backlog.py serve [port] [--open]   # live board at http://127.0.0.1:4317
  python3 scripts/backlog.py claim <id> <assignee>
  python3 scripts/backlog.py move <id> <status> [note]
  python3 scripts/backlog.py pr <id> <url>          # link a pull request to a story

No third-party dependencies (standard library only).
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "backlog"
OUT = ROOT / "backlog.json"
BOARD = ROOT / "board.html"
TEMPLATE = Path(__file__).resolve().parent / "board_template.html"

STATUSES = ["backlog", "todo", "planning", "in_progress", "review", "done", "blocked"]
PRIORITIES = ["P0", "P1", "P2"]
SIZES = ["S", "M", "L"]
STORY_REQUIRED = ["id", "title", "epic", "status", "priority", "size", "depends_on", "labels", "assignee", "updated"]
EPIC_REQUIRED = ["id", "title", "phase", "status"]
CHECKBOX = re.compile(r"^\s*- \[( |x|X)\] (.+)$")
PR_URL = re.compile(r"^https://github\.com/[\w.-]+/[\w.-]+/pull/(\d+)$")


def pr_list(value) -> list[str]:
    """Optional `pr:` field: one URL or a [list], since a story can need several PRs."""
    if not value:
        return []
    return value if isinstance(value, list) else [value]


class BacklogError(Exception):
    pass


def parse_value(raw: str):
    raw = raw.strip()
    if raw.startswith("[") and raw.endswith("]"):
        inner = raw[1:-1].strip()
        return [v.strip().strip("\"'") for v in inner.split(",") if v.strip()] if inner else []
    return raw.strip("\"'")


def parse_file(path: Path) -> tuple[dict, str]:
    text = path.read_text(encoding="utf-8")
    if not text.startswith("---\n"):
        raise BacklogError(f"{path}: missing frontmatter")
    try:
        _, front, body = text.split("---\n", 2)
    except ValueError as exc:
        raise BacklogError(f"{path}: unterminated frontmatter") from exc
    meta = {}
    for line in front.splitlines():
        if not line.strip():
            continue
        if ":" not in line:
            raise BacklogError(f"{path}: bad frontmatter line: {line!r}")
        key, value = line.split(":", 1)
        meta[key.strip()] = parse_value(value)
    return meta, body


def sections(body: str) -> dict[str, str]:
    out, current, buf = {}, None, []
    for line in body.splitlines():
        if line.startswith("## "):
            if current:
                out[current] = "\n".join(buf).strip()
            current, buf = line[3:].strip().lower(), []
        elif current:
            buf.append(line)
    if current:
        out[current] = "\n".join(buf).strip()
    return out


def load() -> dict:
    errors: list[str] = []
    epics, stories = [], []

    for path in sorted((ROOT / "epics").glob("*.md")):
        meta, body = parse_file(path)
        missing = [k for k in EPIC_REQUIRED if k not in meta]
        if missing:
            errors.append(f"{path.name}: missing {missing}")
            continue
        if meta["id"] != path.stem:
            errors.append(f"{path.name}: id {meta['id']} does not match filename")
        if meta["status"] not in STATUSES:
            errors.append(f"{path.name}: bad status {meta['status']}")
        epics.append({
            "id": meta["id"], "title": meta["title"], "phase": int(meta["phase"]),
            "status": meta["status"], "goal": sections(body).get("goal", ""),
            "path": f"backlog/epics/{path.name}",
        })

    epic_ids = {e["id"] for e in epics}
    for path in sorted((ROOT / "stories").glob("*.md")):
        meta, body = parse_file(path)
        missing = [k for k in STORY_REQUIRED if k not in meta]
        if missing:
            errors.append(f"{path.name}: missing {missing}")
            continue
        sid = meta["id"]
        if sid != path.stem:
            errors.append(f"{path.name}: id {sid} does not match filename")
        for field, allowed in (("status", STATUSES), ("priority", PRIORITIES), ("size", SIZES)):
            if meta[field] not in allowed:
                errors.append(f"{sid}: bad {field} {meta[field]!r}")
        if meta["epic"] not in epic_ids:
            errors.append(f"{sid}: unknown epic {meta['epic']}")
        sec = sections(body)
        criteria = []
        for line in sec.get("acceptance criteria", "").splitlines():
            m = CHECKBOX.match(line)
            if m:
                criteria.append({"text": m.group(2).strip(), "done": m.group(1).lower() == "x"})
        if not criteria:
            errors.append(f"{sid}: no acceptance criteria checkboxes")
        prs = pr_list(meta.get("pr"))
        errors += [f"{sid}: bad pr URL {url!r}" for url in prs if not PR_URL.match(url)]
        stories.append({
            "id": sid, "title": meta["title"], "epic": meta["epic"], "status": meta["status"],
            "priority": meta["priority"], "size": meta["size"], "depends_on": meta["depends_on"],
            "labels": meta["labels"], "assignee": meta["assignee"] or None, "updated": meta["updated"],
            "context": sec.get("context", ""), "acceptance": criteria,
            "plan": sec.get("plan", ""), "path": f"backlog/stories/{path.name}",
            "prs": [{"url": url, "number": int(m.group(1))} for url in prs if (m := PR_URL.match(url))],
        })

    by_id = {s["id"]: s for s in stories}
    for s in stories:
        for dep in s["depends_on"]:
            if dep not in by_id:
                errors.append(f"{s['id']}: unknown dependency {dep}")
    errors += find_cycles(by_id)
    if errors:
        raise BacklogError("Backlog invalid:\n  " + "\n  ".join(errors))

    for s in stories:
        waiting = [d for d in s["depends_on"] if by_id[d]["status"] != "done"]
        s["waiting_on"] = waiting
        s["ready"] = s["status"] == "todo" and not waiting
        total = len(s["acceptance"])
        s["progress"] = round(sum(c["done"] for c in s["acceptance"]) / total, 2) if total else 0.0

    for e in epics:
        own = [s for s in stories if s["epic"] == e["id"]]
        e["story_count"] = len(own)
        e["done_count"] = sum(s["status"] == "done" for s in own)
        e["progress"] = round(e["done_count"] / len(own), 2) if own else 0.0

    return {"schema_version": 1, "statuses": STATUSES, "epics": epics, "stories": stories}


def find_cycles(by_id: dict) -> list[str]:
    errors, state = [], {}

    def visit(node: str, stack: list[str]):
        state[node] = "visiting"
        for dep in by_id[node]["depends_on"]:
            if dep not in by_id:
                continue
            if state.get(dep) == "visiting":
                errors.append("dependency cycle: " + " -> ".join(stack + [node, dep]))
            elif dep not in state:
                visit(dep, stack + [node])
        state[node] = "done"

    for sid in by_id:
        if sid not in state:
            visit(sid, [])
    return errors


def render(data: dict) -> str:
    return json.dumps(data, indent=2, ensure_ascii=False) + "\n"


def build_board(data: dict, version: int = 0, error: str | None = None) -> str:
    """Render the self-contained control panel HTML (no network, no external files)."""
    from datetime import datetime
    meta = {"generated_at": datetime.now().strftime("%d %b %Y, %H:%M:%S"), "version": version}
    if error:
        meta["error"] = error
    blob = json.dumps(dict(data, meta=meta), ensure_ascii=False).replace("</", "<\\/")
    return TEMPLATE.read_text(encoding="utf-8").replace("__BACKLOG_DATA__", blob)


def write_board(data: dict) -> None:
    if TEMPLATE.exists():
        BOARD.write_text(build_board(data), encoding="utf-8")


def write_all(data: dict) -> None:
    OUT.write_text(render(data), encoding="utf-8")
    write_board(data)


def set_fields(story_id: str, fields: dict, note: str) -> None:
    """Edit a story's frontmatter in place and append a dated Log line."""
    from datetime import date
    path = ROOT / "stories" / f"{story_id}.md"
    if not path.exists():
        raise BacklogError(f"no story {story_id}")
    head, front, body = path.read_text(encoding="utf-8").split("---\n", 2)
    fields = dict(fields, updated=date.today().isoformat())
    lines = []
    for line in front.splitlines():
        key = line.split(":", 1)[0].strip()
        lines.append(f"{key}: {fields.pop(key)}" if key in fields else line)
    lines += [f"{k}: {v}" for k, v in fields.items()]
    body = body.rstrip() + f"\n- {date.today().isoformat()}: {note}\n"
    path.write_text(f"{head}---\n" + "\n".join(lines) + "\n---\n" + body, encoding="utf-8")


def session_summary(data: dict) -> str:
    out = ["# Backlog status (auto-generated at session start)"]
    for st in ("in_progress", "planning", "review", "blocked"):
        items = [s for s in data["stories"] if s["status"] == st]
        if items:
            out.append(f"\n{st}:")
            out += [f"- {s['id']} {s['title']} (assignee: {s['assignee'] or 'none'})" for s in items]
    nxt = next_story(data)
    done = sum(s["status"] == "done" for s in data["stories"])
    out.append(f"\n{done}/{len(data['stories'])} done, {sum(s['ready'] for s in data['stories'])} ready.")
    out.append(f"Next ready story: {nxt['id']} {nxt['title']} ({nxt['path']})" if nxt else "No ready stories.")
    out.append("Use the backlog skill for all backlog housekeeping.")
    return "\n".join(out)


def next_story(data: dict):
    order = {p: i for i, p in enumerate(PRIORITIES)}
    ready = [s for s in data["stories"] if s["ready"]]
    ready.sort(key=lambda s: (order[s["priority"]], s["id"]))
    return ready[0] if ready else None


def watched_files() -> dict[Path, float]:
    files = list((ROOT / "epics").glob("*.md")) + list((ROOT / "stories").glob("*.md")) + [TEMPLATE]
    return {f: f.stat().st_mtime for f in files if f.exists()}


def serve(port: int, open_browser: bool) -> int:
    """Live board: serves the panel on localhost and reloads it when backlog files change."""
    import threading
    import time
    import webbrowser
    from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

    state = {"version": 0, "data": None, "html": ""}
    lock = threading.Lock()

    def rebuild() -> None:
        with lock:
            state["version"] += 1
            try:
                data = load()
                write_all(data)
                state["data"] = data
                state["html"] = build_board(data, state["version"])
                print(f"[{time.strftime('%H:%M:%S')}] board updated")
            except BacklogError as exc:
                last = state["data"] or {"schema_version": 1, "statuses": STATUSES, "epics": [], "stories": []}
                state["html"] = build_board(last, state["version"], error=str(exc))
                print(f"[{time.strftime('%H:%M:%S')}] backlog invalid:\n{exc}", file=sys.stderr)

    def watch() -> None:
        seen = watched_files()
        while True:
            time.sleep(1)
            now = watched_files()
            if now != seen:
                seen = now
                rebuild()

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):  # noqa: N802 (stdlib naming)
            path = self.path.split("?", 1)[0]
            with lock:
                if path in ("/", "/index.html"):
                    body, kind = state["html"].encode("utf-8"), "text/html; charset=utf-8"
                elif path == "/version":
                    body, kind = str(state["version"]).encode(), "text/plain"
                elif path == "/backlog.json" and state["data"]:
                    body, kind = render(state["data"]).encode("utf-8"), "application/json"
                else:
                    self.send_error(404)
                    return
            self.send_response(200)
            self.send_header("Content-Type", kind)
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *args):  # keep the terminal quiet
            pass

    rebuild()
    threading.Thread(target=watch, daemon=True).start()
    server = ThreadingHTTPServer(("127.0.0.1", port), Handler)
    url = f"http://127.0.0.1:{port}"
    print(f"Live board at {url}  (Ctrl+C to stop)")
    if open_browser:
        webbrowser.open(url)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    return 0


def main(argv: list[str]) -> int:
    if argv[:1] == ["serve"]:
        nums = [a for a in argv[1:] if a.isdigit()]
        return serve(int(nums[0]) if nums else 4317, "--open" in argv)
    try:
        if argv[:1] == ["claim"] and len(argv) >= 3:
            data = load()
            s = next((x for x in data["stories"] if x["id"] == argv[1]), None)
            if s is None:
                raise BacklogError(f"no story {argv[1]}")
            if not s["ready"] or s["assignee"]:
                raise BacklogError(f"{argv[1]} is not claimable (status={s['status']}, waiting_on={s['waiting_on']}, assignee={s['assignee']})")
            set_fields(argv[1], {"status": "planning", "assignee": argv[2]}, f"claimed by {argv[2]}")
        elif argv[:1] == ["move"] and len(argv) >= 3:
            if argv[2] not in STATUSES:
                raise BacklogError(f"bad status {argv[2]}; use one of {STATUSES}")
            note = " ".join(argv[3:]) or f"moved to {argv[2]}"
            set_fields(argv[1], {"status": argv[2]}, note)
        elif argv[:1] == ["pr"] and len(argv) >= 3:
            if not PR_URL.match(argv[2]):
                raise BacklogError(f"not a GitHub pull request URL: {argv[2]}")
            path = ROOT / "stories" / f"{argv[1]}.md"
            if not path.exists():
                raise BacklogError(f"no story {argv[1]}")
            prs = pr_list(parse_file(path)[0].get("pr"))
            if argv[2] not in prs:
                set_fields(argv[1], {"pr": "[" + ", ".join(prs + [argv[2]]) + "]"}, f"PR {argv[2]}")
        data = load()
    except BacklogError as exc:
        print(exc, file=sys.stderr)
        return 1
    if argv[:1] in (["claim"], ["move"], ["pr"]):
        write_all(data)
        print(f"{argv[1]} updated; backlog.json and board.html regenerated")
        return 0
    if "session" in argv:
        write_all(data)
        print(session_summary(data))
        return 0
    if "--check" in argv:
        current = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        if current != render(data):
            print("backlog.json is stale: run `just backlog`", file=sys.stderr)
            return 1
        print("backlog OK")
        return 0
    if "next" in argv:
        s = next_story(data)
        print(f"{s['id']}  {s['title']}  ({s['priority']}, {s['size']})  {s['path']}" if s else "No ready stories.")
        return 0
    if "summary" in argv:
        for status in STATUSES:
            print(f"{status:12} {sum(s['status'] == status for s in data['stories'])}")
        print(f"{'ready':12} {sum(s['ready'] for s in data['stories'])}")
        return 0
    write_all(data)
    print(f"wrote {OUT.relative_to(ROOT.parent)} ({len(data['stories'])} stories)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
