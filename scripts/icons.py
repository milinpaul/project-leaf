#!/usr/bin/env python3
"""Generate Leaf's app icons from the master SVG.

Run with `just icons` after changing `crates/leaf-app/resources/icon.svg`, and commit the results. Builds use
the committed files, so neither CI nor contributors need these tools; only whoever redraws the icon does.

Needs `resvg` (`cargo install resvg --locked`) to render the SVG, and macOS's `iconutil` for the `.icns`.
The Windows `.ico` is written here directly: it is a small header followed by PNG images.
"""

from __future__ import annotations

import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

RESOURCES = Path(__file__).resolve().parent.parent / "crates" / "leaf-app" / "resources"
SVG = RESOURCES / "icon.svg"
PNG_DIR = RESOURCES / "png"

# Linux icon themes (hicolor) and general use.
PNG_SIZES = [16, 32, 48, 64, 128, 256, 512, 1024]
# Windows picks the closest size for each place it shows the icon.
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]
# macOS iconset entries: (file name, pixel size).
ICONSET = [
    (f"icon_{size}x{size}{suffix}.png", size * scale)
    for size in (16, 32, 128, 256, 512)
    for suffix, scale in (("", 1), ("@2x", 2))
]


def render(size: int, out: Path) -> None:
    subprocess.run(["resvg", "-w", str(size), "-h", str(size), str(SVG), str(out)], check=True)


def write_ico(pngs: list[tuple[int, bytes]], out: Path) -> None:
    """An ICO file: a 6-byte header, a 16-byte entry per image, then the PNG data of each image."""
    header = struct.pack("<HHH", 0, 1, len(pngs))
    offset = len(header) + 16 * len(pngs)
    entries = b""
    for size, data in pngs:
        # Width and height 0 mean 256.
        side = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    out.write_bytes(header + entries + b"".join(data for _, data in pngs))


def main() -> int:
    if shutil.which("resvg") is None:
        print("icons: resvg not found; install it with `cargo install resvg --locked`", file=sys.stderr)
        return 1

    PNG_DIR.mkdir(exist_ok=True)
    for size in PNG_SIZES:
        render(size, PNG_DIR / f"leaf-{size}.png")

    with tempfile.TemporaryDirectory() as tmp:
        ico = []
        for size in ICO_SIZES:
            path = Path(tmp) / f"ico-{size}.png"
            render(size, path)
            ico.append((size, path.read_bytes()))
        write_ico(ico, RESOURCES / "leaf.ico")

        if shutil.which("iconutil") is None:
            print("icons: iconutil not found (macOS only); Leaf.icns left as it was", file=sys.stderr)
        else:
            iconset = Path(tmp) / "Leaf.iconset"
            iconset.mkdir()
            for name, size in ICONSET:
                render(size, iconset / name)
            subprocess.run(
                ["iconutil", "-c", "icns", str(iconset), "-o", str(RESOURCES / "Leaf.icns")], check=True
            )

    print(f"icons: written to {RESOURCES}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
