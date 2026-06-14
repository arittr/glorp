#!/usr/bin/env python3
"""Render a dev-preview frame's .cells.json to a PNG so the colored output can
be inspected as an image (the .txt frames drop color; index.html can't be
viewed headless). Run with Pillow available, e.g.:

    uv run --with pillow python3 scripts/render_preview_frame.py \
        target/glorp-preview/frames/pet-species-stage.cells.json out.png

Optional 3rd/4th args: cell width, cell height (px).
"""
import json
import sys

from PIL import Image, ImageDraw, ImageFont

inp = sys.argv[1]
outp = sys.argv[2]
CW = int(sys.argv[3]) if len(sys.argv) > 3 else 14
CH = int(sys.argv[4]) if len(sys.argv) > 4 else 28
FS = int(CH * 0.86)
DEFAULT_BG = (0x13, 0x11, 0x0F)  # tokenpet theme bg
DEFAULT_FG = (0xE8, 0xE3, 0xDA)
FONT = "/System/Library/Fonts/Menlo.ttc"


def hexrgb(s):
    if not s or not isinstance(s, str) or not s.startswith("#") or len(s) != 7:
        return None
    return tuple(int(s[i : i + 2], 16) for i in (1, 3, 5))


def main():
    d = json.load(open(inp))
    w, h, cells = d["width"], d["height"], d["cells"]
    img = Image.new("RGB", (w * CW, h * CH), DEFAULT_BG)
    draw = ImageDraw.Draw(img)
    font = ImageFont.truetype(FONT, FS, index=0)

    # Pass 1: per-cell backgrounds (e.g. the biome wash).
    for c in cells:
        bg = hexrgb(c.get("bg"))
        if bg:
            x, y = c["x"], c["y"]
            dw = c.get("display_width", 1) or 1
            draw.rectangle(
                [x * CW, y * CH, (x + dw) * CW - 1, (y + 1) * CH - 1], fill=bg
            )

    # Pass 2: glyphs.
    for c in cells:
        if c.get("continuation"):
            continue
        sym = c["symbol"]
        if sym.strip() == "":
            continue
        fg = hexrgb(c.get("fg")) or DEFAULT_FG
        mods = c.get("modifiers", [])
        if "dim" in mods:
            fg = tuple(int(v * 0.6) for v in fg)
        x, y = c["x"], c["y"]
        draw.text((x * CW + 1, y * CH - 1), sym, fill=fg, font=font)

    img.save(outp)
    print("wrote", outp, img.size)


if __name__ == "__main__":
    main()
