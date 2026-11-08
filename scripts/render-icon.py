#!/usr/bin/env python3
"""assets/icons/bing-wallpaper-manager.svg から各サイズの PNG を作る（GdkPixbuf + librsvg）。

  python3 scripts/render-icon.py
"""
from pathlib import Path

import gi

gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
SVG = ROOT / "assets/icons/bing-wallpaper-manager.svg"
SIZES = [16, 24, 32, 48, 64, 128, 256, 512]

for size in SIZES:
    out = ROOT / f"assets/icons/hicolor/{size}x{size}/apps/bing-wallpaper-manager.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    GdkPixbuf.Pixbuf.new_from_file_at_size(str(SVG), size, size).savev(str(out), "png", [], [])
    print(out.relative_to(ROOT))
