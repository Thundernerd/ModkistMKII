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
# Win32 COLOR_BTNFACE / stock WixUI dialog surface (rgb 240, 240, 240).
WIX_UI_BACKGROUND = (240, 240, 240)


def paste_icon(canvas: Image.Image, icon: Image.Image, size: int, x: int, y: int) -> None:
    resized = icon.resize((size, size), Image.Resampling.LANCZOS)
    # Flatten alpha onto the installer background so icon edges do not darken the panel.
    flattened = Image.new("RGB", (size, size), WIX_UI_BACKGROUND)
    flattened.paste(resized, (0, 0), resized)
    canvas.paste(flattened, (x, y))


def main() -> None:
    icon = Image.open(ICON).convert("RGBA")

    banner = Image.new("RGB", BANNER_SIZE, WIX_UI_BACKGROUND)
    paste_icon(banner, icon, 32, 16, 13)
    banner.save(ROOT / "installer-banner.bmp", format="BMP")

    dialog = Image.new("RGB", DIALOG_SIZE, WIX_UI_BACKGROUND)
    paste_icon(dialog, icon, 72, 32, 32)
    dialog.save(ROOT / "installer-dialog.bmp", format="BMP")

    print(f"Wrote {ROOT / 'installer-banner.bmp'}")
    print(f"Wrote {ROOT / 'installer-dialog.bmp'}")


if __name__ == "__main__":
    main()
