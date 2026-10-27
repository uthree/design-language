#!/usr/bin/env python3
"""Draw Atelier app marks: two letters on a dark element tile, after a periodic table cell.

    python3 tools/app-icon/make_icons.py            # every app with ship = true, and the sheets
    python3 tools/app-icon/make_icons.py --only auris-studio --no-sheets

The rules these numbers follow are in doc/app-icons.md. Needs fonttools, uharfbuzz, resvg-py
and Pillow, and Inter Display (Bold and SemiBold) installed or passed with --font-dir.
"""

from __future__ import annotations

import argparse
import colorsys
import io
import math
import subprocess
import tomllib
from dataclasses import dataclass
from pathlib import Path

import resvg_py
import uharfbuzz as hb
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[2]
ART = 1024

# Element colours: the glyph's seed. Taken from the backdrop palettes (src/backdrop.rs) so the
# icons and the welcome screens share hues. The tile is derived from the same hue.
ELEMENTS = {
    "gold": 0xFFD36B,
    "orange": 0xFF8A3D,
    "crimson": 0xE8455A,
    "rose": 0xFF6F91,
    "violet": 0x9B6BFF,
    "blue": 0x3F8CFF,
    "cyan": 0x4FD8FF,
    "seaglass": 0x9FF7E0,
    "green": 0x8FE08A,
    "lime": 0xE6F27A,
    "graphite": 0xB8BAC2,
}

# Construction, as fractions of the tile's side.
CAP_HEIGHT = 0.36
CAP_HEIGHT_SMALL = 0.44
MAX_SYMBOL_WIDTH = 0.70
OPTICAL_LIFT = 0.01
NUMBER_CAP = 0.075
NUMBER_INSET = 0.13
NAME_CAP = 0.058
NAME_BASELINE = 0.875  # from the tile's top
MAX_NAME_WIDTH = 0.74
CORNER_OPACITY = 0.72
RIM = 0.012
SQUIRCLE_EXPONENT = 5.0
TRACKING = -0.02  # of the em, between the two letters
GLYPH_CONTRAST = 7.0

# macOS draws its icons on an 824 tile inside the 1024 canvas, with a shadow.
MACOS_TILE = 824


# Colour ---------------------------------------------------------------------------------------


def rgb(value: int) -> tuple[float, float, float]:
    return ((value >> 16) & 255) / 255, ((value >> 8) & 255) / 255, (value & 255) / 255


def hex_of(color: tuple[float, float, float]) -> str:
    return "#" + "".join(f"{round(max(0, min(1, c)) * 255):02x}" for c in color)


def luminance(color: tuple[float, float, float]) -> float:
    def lin(c: float) -> float:
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    r, g, b = map(lin, color)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a, b) -> float:
    la, lb = sorted((luminance(a), luminance(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


def hls(h: float, l: float, s: float):
    return colorsys.hls_to_rgb(h, max(0, min(1, l)), max(0, min(1, s)))


@dataclass
class Palette:
    glyph: tuple[float, float, float]
    tile_top: tuple[float, float, float]
    tile_bottom: tuple[float, float, float]
    rim: tuple[float, float, float]
    metal: list[tuple[float, tuple[float, float, float]]]


def palette(element: str) -> Palette:
    seed = rgb(ELEMENTS[element])
    h, l, s = colorsys.rgb_to_hls(*seed)
    tile_top = hls(h, 0.155, s * 0.62)
    tile_bottom = hls(h, 0.085, s * 0.70)
    glyph = seed
    while contrast(glyph, tile_top) < GLYPH_CONTRAST:
        gh, gl, gs = colorsys.rgb_to_hls(*glyph)
        glyph = hls(gh, gl + 0.01, gs)
    # The metal finish: a chrome ramp in the element's hue, bright sky above a dark horizon.
    metal = [
        (0.00, hls(h, 0.94, s * 0.55)),
        (0.36, hls(h, 0.78, s * 0.85)),
        (0.50, hls(h, 0.36, s * 0.70)),
        (0.56, hls(h, 0.60, s * 0.85)),
        (0.80, hls(h, 0.82, s * 0.75)),
        (1.00, hls(h, 0.64, s * 0.80)),
    ]
    return Palette(glyph, tile_top, tile_bottom, hls(h, 0.55, s * 0.8), metal)


# Type -----------------------------------------------------------------------------------------


class Face:
    def __init__(self, path: Path):
        self.path = path
        self.font = TTFont(path)
        self.glyphs = self.font.getGlyphSet()
        self.order = self.font.getGlyphOrder()
        self.upem = self.font["head"].unitsPerEm
        self.cap = self.font["OS/2"].sCapHeight
        blob = hb.Blob.from_file_path(str(path))
        self.hb = hb.Font(hb.Face(blob))

    def outline(self, text: str, cap_px: float, tracking: float = 0.0):
        """The text's outline as an SVG path, scaled so capitals are `cap_px` tall, with the
        origin on the baseline at the left of the first glyph; and its advance width."""
        buf = hb.Buffer()
        buf.add_str(text)
        buf.guess_segment_properties()
        hb.shape(self.hb, buf, {"kern": True})
        scale = cap_px / self.cap
        pen = SVGPathPen(self.glyphs)
        x = 0.0
        ink_left = None
        ink_right = 0.0
        for index, (info, pos) in enumerate(zip(buf.glyph_infos, buf.glyph_positions)):
            name = self.order[info.codepoint]
            glyph = self.glyphs[name]
            # Flip y: font units go up, SVG goes down.
            t = TransformPen(pen, (scale, 0, 0, -scale, (x + pos.x_offset) * scale, 0))
            glyph.draw(t)
            bounds = self._bounds(name)
            if bounds:
                left = (x + pos.x_offset + bounds[0]) * scale
                right = (x + pos.x_offset + bounds[2]) * scale
                ink_left = left if ink_left is None else min(ink_left, left)
                ink_right = max(ink_right, right)
            x += pos.x_advance
            if index < len(text) - 1:
                x += tracking * self.upem
        return pen.getCommands(), (ink_left or 0.0), ink_right

    def _bounds(self, name: str):
        from fontTools.pens.boundsPen import BoundsPen

        pen = BoundsPen(self.glyphs)
        self.glyphs[name].draw(pen)
        return pen.bounds


def find_font(font_dir: Path | None, style: str) -> Path:
    file = f"InterDisplay-{style}.otf"
    if font_dir:
        return font_dir / file
    out = subprocess.run(
        ["fc-match", "-f", "%{file}", f"Inter Display:style={style}"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    if "InterDisplay" not in out:
        raise SystemExit(f"Inter Display {style} not found; pass --font-dir")
    return Path(out)


# Drawing --------------------------------------------------------------------------------------


def squircle(x: float, y: float, size: float, steps: int = 256) -> str:
    """A superellipse (continuous corners, as macOS and Glass capsules draw them)."""
    r = size / 2
    cx, cy = x + r, y + r
    n = SQUIRCLE_EXPONENT
    points = []
    for i in range(steps):
        t = 2 * math.pi * i / steps
        c, s = math.cos(t), math.sin(t)
        px = cx + r * math.copysign(abs(c) ** (2 / n), c)
        py = cy + r * math.copysign(abs(s) ** (2 / n), s)
        points.append(f"{px:.2f},{py:.2f}")
    return "M" + " L".join(points) + " Z"


@dataclass
class Mark:
    symbol: str
    number: int | None
    element: str
    version: str | None = None
    name: str | None = None


def edition(version: str | None) -> str | None:
    """What the cell shows for an app's version: the major alone, or β before 1.0, so the icon
    changes only at a major release."""
    if not version:
        return None
    major = int(version.split(".")[0])
    return str(major) if major >= 1 else "β"


def mark_of(app: dict) -> Mark:
    return Mark(app["symbol"], app.get("number"), app["element"], edition(app.get("version")), app.get("name"))


def draw(
    mark: Mark,
    faces: dict[str, Face],
    *,
    form: str = "standard",  # standard | macos
    # full (>= 256 px) | corners (128 px) | medium (48-64 px) | small (<= 32 px)
    detail: str = "full",
    finish: str = "flat",  # flat | metal
    guides: bool = False,
) -> str:
    p = palette(mark.element)
    if form == "macos":
        size = MACOS_TILE
        x0 = y0 = (ART - MACOS_TILE) / 2
        y0 -= 10  # leave room for the shadow below, as Apple's grid does
    else:
        size = ART
        x0 = y0 = 0
    tile = squircle(x0, y0, size)
    cap = (CAP_HEIGHT_SMALL if detail == "small" else CAP_HEIGHT) * size
    bold = faces["Bold"]
    path, left, right = bold.outline(mark.symbol, cap, TRACKING)
    width = right - left
    limit = MAX_SYMBOL_WIDTH * size
    if width > limit:
        scale = limit / width
        cap *= scale
        path, left, right = bold.outline(mark.symbol, cap, TRACKING)
        width = right - left
    gx = x0 + (size - width) / 2 - left
    baseline = y0 + size / 2 + cap / 2 - OPTICAL_LIFT * size

    defs = [
        f'<linearGradient id="fill" x1="0" y1="0" x2="0" y2="1">'
        f'<stop offset="0" stop-color="{hex_of(p.tile_top)}"/>'
        f'<stop offset="1" stop-color="{hex_of(p.tile_bottom)}"/></linearGradient>',
        f'<clipPath id="tile"><path d="{tile}"/></clipPath>',
    ]
    body = []
    if form == "macos":
        defs.append(
            '<filter id="shadow" x="-20%" y="-20%" width="140%" height="140%">'
            '<feDropShadow dx="0" dy="10" stdDeviation="12" flood-color="#000" '
            'flood-opacity="0.32"/></filter>'
        )
        body.append(f'<path d="{tile}" fill="url(#fill)" filter="url(#shadow)"/>')
    else:
        body.append(f'<path d="{tile}" fill="url(#fill)"/>')

    if detail != "small":
        # Glass: a soft sheen from the top and a lit upper edge, as `highlight` lights bezels.
        defs.append(
            '<linearGradient id="sheen" x1="0" y1="0" x2="0" y2="1">'
            '<stop offset="0" stop-color="#fff" stop-opacity="0.10"/>'
            '<stop offset="0.5" stop-color="#fff" stop-opacity="0"/></linearGradient>'
        )
        defs.append(
            '<linearGradient id="edge" x1="0" y1="0" x2="0" y2="1">'
            '<stop offset="0" stop-color="#fff" stop-opacity="0.28"/>'
            '<stop offset="0.35" stop-color="#fff" stop-opacity="0"/></linearGradient>'
        )
        rim = RIM * size
        body.append(f'<rect x="{x0}" y="{y0}" width="{size}" height="{size}" fill="url(#sheen)" clip-path="url(#tile)"/>')
        body.append(
            f'<path d="{tile}" fill="none" stroke="{hex_of(p.rim)}" stroke-opacity="0.34" '
            f'stroke-width="{rim * 2:.1f}" clip-path="url(#tile)"/>'
        )
        body.append(
            f'<path d="{tile}" fill="none" stroke="url(#edge)" stroke-width="{rim * 1.2:.1f}" '
            f'clip-path="url(#tile)"/>'
        )

    if finish == "metal":
        stops = "".join(
            f'<stop offset="{o}" stop-color="{hex_of(c)}"/>' for o, c in p.metal
        )
        defs.append(
            f'<linearGradient id="metal" gradientUnits="userSpaceOnUse" x1="0" y1="{-cap:.1f}" '
            f'x2="0" y2="0">{stops}</linearGradient>'
        )
        glyph_fill = "url(#metal)"  # in the glyphs' own space: the baseline is y = 0
    else:
        glyph_fill = hex_of(p.glyph)
    body.append(f'<path transform="translate({gx:.2f} {baseline:.2f})" d="{path}" fill="{glyph_fill}"/>')

    # The cell's readings, as a periodic table prints them: the number top left, the edition
    # top right (where a table puts the atomic weight), the full name along the bottom.
    semi = faces["SemiBold"]
    ink = f'fill="{hex_of(p.glyph)}" fill-opacity="{CORNER_OPACITY}"'
    if detail in ("full", "corners"):
        ncap = NUMBER_CAP * size
        top = y0 + NUMBER_INSET * size + ncap
        if mark.number is not None:
            npath, nleft, _ = semi.outline(str(mark.number), ncap)
            nx = x0 + NUMBER_INSET * size - nleft
            body.append(f'<path transform="translate({nx:.2f} {top:.2f})" d="{npath}" {ink}/>')
        if mark.version:
            vpath, _, vright = semi.outline(mark.version, ncap)
            vx = x0 + size - NUMBER_INSET * size - vright
            body.append(f'<path transform="translate({vx:.2f} {top:.2f})" d="{vpath}" {ink}/>')
    if detail == "full" and mark.name:
        ncap = NAME_CAP * size
        npath, nleft, nright = semi.outline(mark.name, ncap)
        limit = MAX_NAME_WIDTH * size
        if nright - nleft > limit:
            ncap *= limit / (nright - nleft)
            npath, nleft, nright = semi.outline(mark.name, ncap)
        nx = x0 + (size - (nright - nleft)) / 2 - nleft
        ny = y0 + NAME_BASELINE * size
        body.append(f'<path transform="translate({nx:.2f} {ny:.2f})" d="{npath}" {ink}/>')

    if guides:
        body.append(guide_lines(x0, y0, size, cap, baseline, gx + left, width))

    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {ART} {ART}" width="{ART}" height="{ART}">'
        f'<defs>{"".join(defs)}</defs>{"".join(body)}</svg>'
    )


def guide_lines(x0, y0, size, cap, baseline, ink_left, width) -> str:
    c = "#ff3b8d"
    lines = []

    def h(y, dash=False):
        d = ' stroke-dasharray="10 8"' if dash else ""
        lines.append(f'<line x1="{x0}" y1="{y:.1f}" x2="{x0 + size}" y2="{y:.1f}" stroke="{c}" stroke-width="3"{d}/>')

    def v(x, dash=False):
        d = ' stroke-dasharray="10 8"' if dash else ""
        lines.append(f'<line x1="{x:.1f}" y1="{y0}" x2="{x:.1f}" y2="{y0 + size}" stroke="{c}" stroke-width="3"{d}/>')

    h(baseline)
    h(baseline - cap)
    h(y0 + size / 2, dash=True)
    v(x0 + size / 2, dash=True)
    margin = (1 - MAX_SYMBOL_WIDTH) / 2 * size
    v(x0 + margin, dash=True)
    v(x0 + size - margin, dash=True)
    n = NUMBER_INSET * size
    lines.append(
        f'<rect x="{x0 + n:.1f}" y="{y0 + n:.1f}" width="{NUMBER_CAP * size * 2.2:.1f}" '
        f'height="{NUMBER_CAP * size:.1f}" fill="none" stroke="{c}" stroke-width="3"/>'
    )
    lines.append(
        f'<rect x="{x0 + size - n - NUMBER_CAP * size * 2.2:.1f}" y="{y0 + n:.1f}" '
        f'width="{NUMBER_CAP * size * 2.2:.1f}" height="{NUMBER_CAP * size:.1f}" '
        f'fill="none" stroke="{c}" stroke-width="3"/>'
    )
    name_margin = (1 - MAX_NAME_WIDTH) / 2 * size
    lines.append(
        f'<rect x="{x0 + name_margin:.1f}" y="{y0 + (NAME_BASELINE - NAME_CAP) * size:.1f}" '
        f'width="{MAX_NAME_WIDTH * size:.1f}" height="{NAME_CAP * size:.1f}" '
        f'fill="none" stroke="{c}" stroke-width="3"/>'
    )
    return "".join(lines)


def symbolic(mark: Mark, faces: dict[str, Face]) -> str:
    """Letters alone, black, for menu bar template images and Linux -symbolic icons."""
    side = 16
    cap = 0.62 * side
    path, left, right = faces["Bold"].outline(mark.symbol, cap, TRACKING)
    width = right - left
    if width > side * 0.96:
        cap *= side * 0.96 / width
        path, left, right = faces["Bold"].outline(mark.symbol, cap, TRACKING)
        width = right - left
    gx = (side - width) / 2 - left
    baseline = side / 2 + cap / 2
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {side} {side}" width="{side}" height="{side}">'
        f'<path transform="translate({gx:.3f} {baseline:.3f})" d="{path}" fill="#000"/></svg>'
    )


def raster(svg: str, px: int) -> Image.Image:
    data = resvg_py.svg_to_bytes(svg_string=svg, width=px, height=px)
    return Image.open(io.BytesIO(bytes(data))).convert("RGBA")


def detail_for(px: int) -> str:
    if px <= 32:
        return "small"
    if px <= 64:
        return "medium"
    if px < 256:
        return "corners"
    return "full"


# Output ---------------------------------------------------------------------------------------

PNG_SIZES = [16, 24, 32, 48, 64, 128, 256, 512, 1024]
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]
ICNS_SIZES = [16, 32, 64, 128, 256, 512, 1024]


def write_app(app: dict, faces, out: Path):
    mark = mark_of(app)
    out.mkdir(parents=True, exist_ok=True)
    (out / "png").mkdir(exist_ok=True)
    svgs = {
        "icon.svg": draw(mark, faces),
        "icon-corners.svg": draw(mark, faces, detail="corners"),
        "icon-medium.svg": draw(mark, faces, detail="medium"),
        "icon-small.svg": draw(mark, faces, detail="small"),
        "icon-macos.svg": draw(mark, faces, form="macos"),
        "symbolic.svg": symbolic(mark, faces),
    }
    if app.get("metal"):
        svgs["icon-metal.svg"] = draw(mark, faces, finish="metal")
    for name, svg in svgs.items():
        (out / name).write_text(svg + "\n")
    images = {}
    for px in PNG_SIZES:
        images[px] = raster(draw(mark, faces, detail=detail_for(px)), px)
        images[px].save(out / "png" / f"{px}.png", optimize=True)
    ico = [images[px] for px in ICO_SIZES]
    ico[-1].save(out / "icon.ico", sizes=[(px, px) for px in ICO_SIZES], append_images=ico[:-1])
    mac = [raster(draw(mark, faces, form="macos", detail=detail_for(px)), px) for px in ICNS_SIZES]
    mac[-1].save(out / "icon.icns", append_images=mac[:-1])
    print(f"wrote {out.relative_to(ROOT)}")


# Sheets ---------------------------------------------------------------------------------------

DESK_DARK = (30, 30, 34)
DESK_LIGHT = (236, 236, 240)


def label_font(face: Face, px: int):
    return ImageFont.truetype(str(face.path), px)


def sheet_suite(apps, faces, out: Path):
    apps = [a for a in apps if a["slug"] != "auris-studio-family"]
    cell, pad, icon = 320, 40, 256
    width = pad * 2 + cell * len(apps)
    height = 2 * (icon + 120)
    img = Image.new("RGB", (width, height), DESK_DARK)
    draw_ = ImageDraw.Draw(img)
    draw_.rectangle([0, height // 2, width, height], fill=DESK_LIGHT)
    name_font = label_font(faces["SemiBold"], 22)
    for row, (bg, fg) in enumerate([(DESK_DARK, (235, 235, 240)), (DESK_LIGHT, (40, 40, 46))]):
        for i, app in enumerate(apps):
            mark = mark_of(app)
            x = pad + i * cell + (cell - icon) // 2
            y = row * (height // 2) + 30
            img.paste(raster(draw(mark, faces), icon), (x, y), raster(draw(mark, faces), icon))
            text = app["name"]
            tw = draw_.textlength(text, font=name_font)
            draw_.text((pad + i * cell + (cell - tw) / 2, y + icon + 20), text, font=name_font, fill=fg)
    img.save(out / "app-icons-suite.png", optimize=True)


def sheet_auris(apps, faces, out: Path):
    by = {a["slug"]: a for a in apps}
    au = by["auris-studio"]
    au = mark_of(au)
    fam = by["auris-studio-family"]
    fam = mark_of(fam)
    items = [
        ("Au · standard", raster(draw(au, faces), 256)),
        ("Au · macOS form", raster(draw(au, faces, form="macos"), 256)),
        ("As · family form", raster(draw(fam, faces), 256)),
    ]
    sym = Image.new("RGBA", (256, 256), (0, 0, 0, 0))
    glyph = raster(symbolic(au, faces), 96)
    for dy in (0,):
        sym.paste(glyph, (80, 80 + dy), glyph)
    items.append(("Au · symbolic", sym))
    cell, pad = 300, 40
    width = pad * 2 + cell * len(items)
    img = Image.new("RGB", (width, 380), DESK_LIGHT)
    d = ImageDraw.Draw(img)
    f = label_font(faces["SemiBold"], 22)
    for i, (text, im) in enumerate(items):
        x = pad + i * cell + (cell - 256) // 2
        img.paste(im, (x, 30), im)
        tw = d.textlength(text, font=f)
        d.text((pad + i * cell + (cell - tw) / 2, 310), text, font=f, fill=(40, 40, 46))
    img.save(out / "app-icons-auris.png", optimize=True)


def sheet_sizes(apps, faces, out: Path):
    a = next(a for a in apps if a["slug"] == "auris-studio")
    mark = mark_of(a)
    sizes = [16, 24, 32, 48, 64, 128, 256]
    pad, gap = 40, 36
    width = pad * 2 + sum(sizes) + gap * (len(sizes) - 1)
    row = 256 + 70
    img = Image.new("RGB", (width, row * 2), DESK_DARK)
    d = ImageDraw.Draw(img)
    d.rectangle([0, row, width, row * 2], fill=DESK_LIGHT)
    f = label_font(faces["SemiBold"], 18)
    for r, fg in enumerate([(200, 200, 206), (60, 60, 66)]):
        x = pad
        for px in sizes:
            im = raster(draw(mark, faces, detail=detail_for(px)), px)
            y = r * row + 20 + (256 - px)
            img.paste(im, (x, y), im)
            text = f"{px}"
            tw = d.textlength(text, font=f)
            d.text((x + (px - tw) / 2, r * row + 20 + 256 + 12), text, font=f, fill=fg)
            x += px + gap
    img.save(out / "app-icons-sizes.png", optimize=True)


def sheet_anatomy(apps, faces, out: Path):
    a = next(a for a in apps if a["slug"] == "auris-studio")
    mark = mark_of(a)
    im = raster(draw(mark, faces, guides=True), 640)
    img = Image.new("RGB", (720, 720), DESK_LIGHT)
    img.paste(im, (40, 40), im)
    img.save(out / "app-icons-anatomy.png", optimize=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--apps", type=Path, default=Path(__file__).with_name("apps.toml"))
    parser.add_argument("--out", type=Path, default=ROOT / "assets" / "app-icons")
    parser.add_argument("--sheets", type=Path, default=ROOT / "doc" / "images")
    parser.add_argument("--only", help="slug of one app to write, ship or not")
    parser.add_argument("--no-sheets", action="store_true")
    parser.add_argument("--font-dir", type=Path)
    args = parser.parse_args()

    apps = tomllib.loads(args.apps.read_text())["app"]
    for app in apps:
        if len(app["symbol"]) != 2 or not (app["symbol"][0].isupper() and app["symbol"][1].islower()):
            raise SystemExit(f"{app['slug']}: a symbol is one capital and one lowercase letter")
        if app["element"] not in ELEMENTS:
            raise SystemExit(f"{app['slug']}: unknown element colour {app['element']!r}")
    faces = {style: Face(find_font(args.font_dir, style)) for style in ("Bold", "SemiBold")}

    for app in apps:
        if (args.only and app["slug"] == args.only) or (not args.only and app.get("ship")):
            write_app(app, faces, args.out / app["slug"])
    if not args.no_sheets:
        args.sheets.mkdir(parents=True, exist_ok=True)
        sheet_suite(apps, faces, args.sheets)
        sheet_auris(apps, faces, args.sheets)
        sheet_sizes(apps, faces, args.sheets)
        sheet_anatomy(apps, faces, args.sheets)
        print(f"wrote sheets to {args.sheets.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
