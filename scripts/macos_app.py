#!/usr/bin/env python3
"""Wrap a built Leaf binary in a macOS app bundle, `Leaf.app`.

macOS takes an app's name, icon and identity from its bundle, so a bare binary shows a generic icon in the
Dock and is called "leaf-app" in the menu bar. `just run` uses this to launch Leaf from a bundle, and
`just install` to put Leaf in /Applications (E08-S07); release packaging (E08-S04) can reuse it.

Usage: python3 scripts/macos_app.py <binary> <output-dir>                 # writes <output-dir>/Leaf.app
       python3 scripts/macos_app.py install <binary> [<apps-dir>]         # also copies it into <apps-dir>
                                                                          # (default /Applications)
"""

from __future__ import annotations

import plistlib
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RESOURCES = ROOT / "crates" / "leaf-app" / "resources"
# Keep in step with APP_ID in crates/leaf-app/src/main.rs.
APP_ID = "io.github.milinpaul.Leaf"


def workspace_version() -> str:
    match = re.search(r'^version\s*=\s*"([^"]+)"', (ROOT / "Cargo.toml").read_text(), re.MULTILINE)
    return match.group(1) if match else "0.0.0"


def make_bundle(binary: Path, out_dir: Path) -> Path:
    """Writes `<out_dir>/Leaf.app` around `binary` and returns its path."""
    app = out_dir / "Leaf.app"
    contents = app / "Contents"
    (contents / "MacOS").mkdir(parents=True, exist_ok=True)
    (contents / "Resources").mkdir(exist_ok=True)

    version = workspace_version()
    info = {
        "CFBundleName": "Leaf",
        "CFBundleDisplayName": "Leaf",
        "CFBundleIdentifier": APP_ID,
        "CFBundleExecutable": "Leaf",
        "CFBundleIconFile": "Leaf",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": version,
        "CFBundleVersion": version,
        "CFBundleDevelopmentRegion": "en",
        "LSApplicationCategoryType": "public.app-category.productivity",
        "NSHighResolutionCapable": True,
    }
    with open(contents / "Info.plist", "wb") as f:
        plistlib.dump(info, f)
    shutil.copyfile(RESOURCES / "Leaf.icns", contents / "Resources" / "Leaf.icns")

    # A clone (copy-on-write on APFS) is instant and takes no extra space; plain copy elsewhere.
    executable = contents / "MacOS" / "Leaf"
    executable.unlink(missing_ok=True)
    if subprocess.run(["cp", "-c", str(binary), str(executable)], stderr=subprocess.DEVNULL).returncode != 0:
        shutil.copy2(binary, executable)
    return app


def bundle_id(app: Path) -> str | None:
    """The bundle ID of an installed app, or None if its Info.plist is missing or unreadable."""
    try:
        with open(app / "Contents" / "Info.plist", "rb") as f:
            value = plistlib.load(f).get("CFBundleIdentifier")
    except (OSError, plistlib.InvalidFileException, ValueError):
        return None
    return value if isinstance(value, str) else None


def install(app: Path, apps_dir: Path) -> Path:
    """Copies `app` into `apps_dir`, replacing an older Leaf there.

    An existing `Leaf.app` is deleted only when its bundle ID is ours, so another program that happens to be
    called Leaf is never removed. Raises `RuntimeError` when it refuses.
    """
    target = apps_dir / app.name
    if target.exists() or target.is_symlink():
        found = bundle_id(target)
        if found != APP_ID:
            raise RuntimeError(
                f"{target} is not Leaf (bundle ID {found or 'unknown'}, expected {APP_ID}); "
                "move it away and run `just install` again"
            )
        if target.is_symlink():
            target.unlink()
        else:
            shutil.rmtree(target)
    shutil.copytree(app, target, symlinks=True)
    return target


def main() -> int:
    args = sys.argv[1:]
    if len(args) in (2, 3) and args[0] == "install":
        apps_dir = Path(args[2]) if len(args) == 3 else Path("/Applications")
        binary = Path(args[1])
        try:
            target = install(make_bundle(binary, binary.parent), apps_dir)
        except RuntimeError as err:
            print(f"error: {err}", file=sys.stderr)
            return 1
        print(f"Installed {target}")
        return 0
    if len(args) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    make_bundle(Path(args[0]), Path(args[1]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
