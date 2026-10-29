#!/usr/bin/env python3
"""Wrap a built Leaf binary in a macOS app bundle, `Leaf.app`.

macOS takes an app's name, icon and identity from its bundle, so a bare binary shows a generic icon in the
Dock and is called "leaf-app" in the menu bar. `just run` uses this to launch Leaf from a bundle; release
packaging (E08-S04) can reuse it.

Usage: python3 scripts/macos_app.py <binary> <output-dir>    # writes <output-dir>/Leaf.app
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


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    binary, out_dir = Path(sys.argv[1]), Path(sys.argv[2])
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
    return 0


if __name__ == "__main__":
    sys.exit(main())
