#!/usr/bin/env python3
"""Generate WiX installer banner and dialog bitmaps from the app icon.

WiX draws black title text on the banner (493x58) on inner installer pages.
Both images use a light background so that text stays readable.
"""

from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent
ICON = ROOT.parent / "icons" / "128x128.png"
BANNER_SIZE = (493, 58)
DIALOG_SIZE = (493, 312)
# Match the light panels used by the stock WiX installer UI.
BACKGROUND = (0xF0, 0xF0, 0xF0)


def paste_icon(canvas: Image.Image, icon: Image.Image, size: int, x: int, y: int) -> None:
    resized = icon.resize((size, size), Image.Resampling.LANCZOS)
    canvas.paste(resized, (x, y), resized)


def main() -> None:
    icon = Image.open(ICON).convert("RGBA")

    banner = Image.new("RGB", BANNER_SIZE, BACKGROUND)
    paste_icon(banner, icon, 32, 16, 13)
    banner.save(ROOT / "installer-banner.bmp")

    dialog = Image.new("RGB", DIALOG_SIZE, BACKGROUND)
    paste_icon(dialog, icon, 72, 32, 32)
    dialog.save(ROOT / "installer-dialog.bmp")

    print(f"Wrote {ROOT / 'installer-banner.bmp'}")
    print(f"Wrote {ROOT / 'installer-dialog.bmp'}")


if __name__ == "__main__":
    main()
