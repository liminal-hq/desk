#!/usr/bin/env python3
# Validates the emoji files with third-party tools: fontTools reads the colour fonts and checks COLR, CPAL and the
# cmaps, OTS sanitises them, FreeType renders every colour emoji and every monochrome emoji glyph in the faces and
# compares the pixels with the .DEM files, the vector emoji are checked in colour and in the outline faces, and the
# era TrueType files are checked to carry no emoji beyond their own.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 validate_emoji.py <generated-dir> [proof-dir]    (run inside the container fonts/validate.sh starts)
import glob
import os
import struct
import subprocess
import sys
import zlib

import freetype
from fontTools.ttLib import TTFont

PALETTE = ["#050507", "#0c0c10", "#111116", "#1b1e28", "#333847", "#7c8796", "#b4bfce", "#e0e0e0",
           "#ffaa40", "#7d5422", "#372612", "#f43f5e", "#a78bfa", "#22d3ee", "#2ec66a", "#60a5fa"]
RGB = [tuple(int(h[i:i + 2], 16) for i in (1, 3, 5)) for h in PALETTE]
# Where each face's emoji sit: the .DEM size, the face's file, its pixel size, its ascent in pixels and the top
# row and left column of the emoji in the cell (fonts/src/emoji/faces.ts).
FACES = {
    "fixed": (16, "desk-fixed", 16, 13, 0, 0),
    "ui": (12, "desk-ui", 12, 11, 1, 0),
    "title": (12, "desk-title", 12, 12, 1, 0),
    "small": (8, "desk-small", 8, 7, 0, 2),
}
MONO = freetype.FT_LOAD_RENDER | freetype.FT_LOAD_TARGET_MONO | freetype.FT_LOAD_NO_HINTING

failures = []


def fail(msg):
    failures.append(msg)
    print("FAIL", msg)


def ok(msg):
    print("ok  ", msg)


def read_dem(path):
    """An independent reader for the .DEM layout in docs/FORMATS.md."""
    data = open(path, "rb").read()
    assert data[:4] == b"DEM1", path
    size, count = data[4], struct.unpack_from("<H", data, 6)[0]
    nb = (size + 7) // 8

    def bitmap(at):
        rows = []
        for y in range(size):
            row = data[at + y * nb: at + (y + 1) * nb]
            rows.append([(row[x >> 3] >> (7 - (x & 7))) & 1 for x in range(size)])
        return rows, at + size * nb

    emoji = {}
    for i in range(count):
        cp, off = struct.unpack_from("<II", data, 8 + 8 * i)
        n = data[off]
        at = off + 1
        layers = []
        for _ in range(n):
            slot = data[at]
            grid, at = bitmap(at + 1)
            layers.append((slot, grid))
        mono, at = bitmap(at)
        colour = [[-1] * size for _ in range(size)]
        for slot, grid in layers:
            for y in range(size):
                for x in range(size):
                    if grid[y][x]:
                        colour[y][x] = slot
        emoji[cp] = (colour, mono)
    return size, emoji


def dfn_code_points(path):
    """The code points of a .DFN file: the characters a face draws itself."""
    data = open(path, "rb").read()
    height, count = data[4], struct.unpack_from("<H", data, 6)[0]
    at, cps = 8, set()
    for _ in range(count):
        cp, width = struct.unpack_from("<IB", data, at)
        cps.add(cp)
        at += 5 + height * ((width + 7) // 8)
    return cps


def inked(face, cp, flags, ascent):
    face.load_char(chr(cp), flags)
    g = face.glyph
    bm = g.bitmap
    out = set()
    for y in range(bm.rows):
        for x in range(bm.width):
            if (bm.buffer[y * bm.pitch + (x >> 3)] >> (7 - (x & 7))) & 1:
                out.add((g.bitmap_left + x, ascent - g.bitmap_top + y))
    return out


def colour_render(face, cp, ascent, scale=1):
    """Renders a colour glyph through COLR and returns {(x, y): (r, g, b, a)} for every visible pixel."""
    face.load_char(chr(cp), freetype.FT_LOAD_COLOR | freetype.FT_LOAD_RENDER | freetype.FT_LOAD_NO_HINTING)
    g = face.glyph
    bm = g.bitmap
    out = {}
    if bm.pixel_mode != freetype.FT_PIXEL_MODE_BGRA:
        return None
    for y in range(bm.rows):
        for x in range(bm.width):
            b, gg, r, a = bm.buffer[y * bm.pitch + 4 * x: y * bm.pitch + 4 * x + 4]
            if a:
                # BGRA is premultiplied; undo it for opaque pixels only.
                out[(g.bitmap_left + x, ascent * scale - g.bitmap_top + y)] = (r, gg, b, a)
    return out


def check_colour_font(gen, path, dem):
    name = os.path.basename(path)
    font = TTFont(path)
    for tag in font.keys():
        font[tag]
    font.save(os.devnull)
    tables = set(font.keys())
    for t in ("COLR", "CPAL", "cmap", "name", "OS/2"):
        if t not in tables:
            fail(f"{name}: no {t} table")
    colr, cpal = font["COLR"], font["CPAL"]
    if colr.version != 0:
        fail(f"{name}: COLR version {colr.version}")
    pixel = "pixel" in name
    if cpal.version != 0 or len(cpal.palettes) != 1 or (pixel and cpal.numPaletteEntries != 16):
        fail(f"{name}: CPAL should be version 0 with one palette (of 16 for the pixel font)")
    pal = ["#%02x%02x%02x" % (c.red, c.green, c.blue) for c in cpal.palettes[0]]
    if pixel and pal != PALETTE:
        fail(f"{name}: CPAL is not the Desk 16-colour set: {pal}")
    if any(c.alpha != 255 for c in cpal.palettes[0]):
        fail(f"{name}: CPAL has colours that are not opaque")
    order = font.getGlyphOrder()
    bad = []
    for base, layers in colr.ColorLayers.items():
        for layer in layers:
            if layer.name not in order or not (layer.colorID < cpal.numPaletteEntries or layer.colorID == 0xFFFF):
                bad.append(base)
    if bad:
        fail(f"{name}: bad layers in {bad[:10]}")
    subtables = {(t.platformID, t.platEncID): t.format for t in font["cmap"].tables}
    for want, fmt in (((0, 3), 4), ((3, 1), 4), ((0, 4), 12), ((3, 10), 12)):
        if subtables.get(want) != fmt:
            fail(f"{name}: cmap {want} should be format {fmt}, got {subtables.get(want)}")
    full = font["cmap"].getcmap(3, 10).cmap
    bmp = font["cmap"].getcmap(3, 1).cmap
    missing = [hex(cp) for cp in dem if cp not in full or (cp <= 0xFFFF and cp not in bmp)]
    if missing:
        fail(f"{name}: emoji not in the cmap: {missing[:10]}")
    no_colour = [hex(cp) for cp in dem if full.get(cp) not in colr.ColorLayers]
    if no_colour:
        fail(f"{name}: emoji without COLR layers: {no_colour[:10]}")
    family = font["name"].getDebugName(1)
    licence = font["name"].getDebugName(13) or ""
    if len(family) > 31 or "Open Font License" not in licence:
        fail(f"{name}: family '{family}' or licence '{licence[:40]}'")
    ok(f"{name}: fontTools reads it; {len(colr.ColorLayers)} colour glyphs, "
       f"{sum(len(v) for v in colr.ColorLayers.values())} layers, {len(order)} glyphs; CPAL of {cpal.numPaletteEntries}"
       f"{' (the Desk set)' if pixel else ''}; "
       f"cmap {sorted(subtables.items())}")


def main(gen, proof_dir=None):
    emoji_dir = os.path.join(gen, "emoji")
    dems = {}
    for size in (16, 12, 8):
        s, d = read_dem(os.path.join(emoji_dir, f"emoji{size}.dem"))
        assert s == size
        dems[size] = d
    print(f"== .DEM files: {len(dems[16])}, {len(dems[12])} and {len(dems[8])} emoji at 16, 12 and 8 px")

    colour_fonts = sorted(glob.glob(os.path.join(emoji_dir, "*.ttf")) + glob.glob(os.path.join(emoji_dir, "*.otf")))
    print("\n== fontTools: the colour fonts")
    for path in colour_fonts:
        try:
            check_colour_font(gen, path, dems[16] if "pixel" in path else {})
        except Exception as e:  # noqa: BLE001
            fail(f"{os.path.basename(path)}: fontTools: {e}")

    print("\n== fontTools: format 12 cmaps in the pixel faces' OpenType files, none in the TrueType files")
    for fid, (size, file, _, _, _, _) in FACES.items():
        for style in ("", "-round"):
            otf = TTFont(os.path.join(gen, f"{file}{style}.otf"))
            sub = {(t.platformID, t.platEncID): t.format for t in otf["cmap"].tables}
            full = otf["cmap"].getcmap(3, 10)
            if sub.get((3, 10)) != 12 or sub.get((0, 4)) != 12:
                fail(f"{file}{style}.otf: no format 12 on (3, 10) and (0, 4): {sub}")
                continue
            missing = [hex(cp) for cp in dems[size] if cp not in full.cmap]
            if missing:
                fail(f"{file}{style}.otf: emoji not reached: {missing[:10]}")
            ttf = TTFont(os.path.join(gen, f"{file}{style}.ttf"))
            tsub = {(t.platformID, t.platEncID): t.format for t in ttf["cmap"].tables}
            have = ttf.getBestCmap()
            own = dfn_code_points(os.path.join(gen, f"{fid}.dfn"))
            extra = [hex(cp) for cp in have if cp not in own and cp not in (0, 0x0D)]
            in_ttf = [cp for cp in dems[size] if cp in have]
            if any(f == 12 for f in tsub.values()) or extra:
                fail(f"{file}{style}.ttf: has a format 12 cmap or characters the face does not draw: {extra[:10]}")
            ok(f"{file}{style}.otf reaches all {len(dems[size])} emoji of {size} px; {file}{style}.ttf has no format 12, "
               f"maps only the face's own characters, and has {len(in_ttf)} emoji code points, all drawn by the face")

    print("\n== OTS (ots-sanitize): the colour fonts")
    for path in colour_fonts:
        r = subprocess.run([sys.executable, "-m", "ots", path, os.devnull], capture_output=True, text=True)
        out = (r.stdout + r.stderr).strip()
        if r.returncode != 0:
            fail(f"{os.path.basename(path)}: ots-sanitize: {out}")
        else:
            ok(f"{os.path.basename(path)}" + (f" ({out})" if out else ""))

    print("\n== FreeType: Desk Emoji Pixel in colour (COLR) and its monochrome base glyphs, against emoji16.dem")
    for path in [p for p in colour_fonts if "pixel" in p]:
        face = freetype.Face(path)
        for scale in (1, 2):
            face.set_pixel_sizes(0, 16 * scale)
            wrong_colour, wrong_mono, no_bgra = [], [], 0
            for cp, (colour, mono) in dems[16].items():
                got = colour_render(face, cp, 14, scale)
                if got is None:
                    no_bgra += 1
                    continue
                want = {}
                for y in range(16):
                    for x in range(16):
                        if colour[y][x] >= 0:
                            for i in range(scale):
                                for j in range(scale):
                                    want[(x * scale + i, y * scale + j)] = RGB[colour[y][x]]
                if set(got) != set(want) or any(got[k][:3] != want[k] or got[k][3] != 255 for k in want):
                    wrong_colour.append(chr(cp))
                if scale == 1:
                    m = inked(face, cp, MONO, 14)
                    if m != {(x, y) for y in range(16) for x in range(16) if mono[y][x]}:
                        wrong_mono.append(chr(cp))
            line = f"{os.path.basename(path)} at {16 * scale} px: {len(dems[16]) - len(wrong_colour) - no_bgra}/{len(dems[16])} colour emoji pixel-exact"
            if scale == 1:
                line += f", {len(dems[16]) - len(wrong_mono)}/{len(dems[16])} monochrome base glyphs exact"
            if wrong_colour or wrong_mono or no_bgra:
                fail(line + f"; not BGRA: {no_bgra}; colour differs: {''.join(wrong_colour[:20])}; mono differs: {''.join(wrong_mono[:20])}")
            else:
                ok(line)

    print("\n== FreeType: the monochrome emoji in the pixel faces' OpenType files, against the .DEM mono bitmaps")
    for fid, (size, file, px, ascent, top, left) in FACES.items():
        for style in ("", "-round"):
            path = os.path.join(gen, f"{file}{style}.otf")
            face = freetype.Face(path)
            face.set_pixel_sizes(0, px)
            own = dfn_code_points(os.path.join(gen, f"{fid}.dfn"))
            wrong, tried = [], 0
            for cp, (_, mono) in dems[size].items():
                # Characters the face draws itself keep the face's glyph.
                if cp in own:
                    continue
                want = {(x + left, y + top) for y in range(size) for x in range(size) if mono[y][x]}
                got = inked(face, cp, MONO, ascent)
                tried += 1
                if got != want and not style:
                    wrong.append(chr(cp))
            line = f"{file}{style}.otf at {px} px: {tried - len(wrong)}/{tried} emoji exact" + (" (Round: rendered, not compared)" if style else "")
            if wrong:
                fail(line + f"; differ: {''.join(wrong[:20])}")
            else:
                ok(line)

    vector = os.path.join(emoji_dir, "desk-emoji.ttf")
    if os.path.exists(vector):
        vfont = TTFont(vector)
        vcps = sorted(cp for cp, g in vfont.getBestCmap().items() if g in vfont["COLR"].ColorLayers)
        print(f"\n== Desk Emoji: {len(vcps)} vector emoji in colour, and in the outline faces' OpenType files")
        face = freetype.Face(vector)
        face.set_pixel_sizes(0, 48)
        grey = []
        for cp in vcps:
            got = colour_render(face, cp, 40)
            # A colour rendering has opaque pixels that are not grey.
            if not got or not any(a == 255 and not (r == g == b) for (r, g, b, a) in got.values()):
                grey.append(chr(cp))
        if grey:
            fail(f"desk-emoji.ttf: FreeType did not render in colour: {''.join(grey[:20])}")
        else:
            ok(f"desk-emoji.ttf at 48 px: {len(vcps)}/{len(vcps)} emoji render through COLR in colour")
        for base in ("desk-sans", "desk-sans-bold", "desk-mono", "desk-mono-bold", "desk-serif", "desk-serif-bold"):
            otf = TTFont(os.path.join(gen, f"{base}.otf"))
            sub = {(t.platformID, t.platEncID): t.format for t in otf["cmap"].tables}
            full = otf["cmap"].getcmap(3, 10)
            missing = [hex(cp) for cp in vcps if not full or cp not in full.cmap]
            ttf = TTFont(os.path.join(gen, f"{base}.ttf"))
            in_ttf = [hex(cp) for cp in vcps if cp in ttf.getBestCmap()]
            if sub.get((3, 10)) != 12 or sub.get((0, 4)) != 12 or missing or in_ttf:
                fail(f"{base}: cmap {sub}; missing from the .otf {missing[:10]}; in the .ttf {in_ttf[:10]}")
            else:
                ok(f"{base}.otf reaches all {len(vcps)} vector emoji through format 12; {base}.ttf has none")
            r = subprocess.run([sys.executable, "-m", "ots", os.path.join(gen, f"{base}.otf"), os.devnull], capture_output=True, text=True)
            if r.returncode != 0:
                fail(f"{base}.otf: ots-sanitize: {(r.stdout + r.stderr).strip()}")

    if proof_dir:
        for path in [p for p in colour_fonts if p.endswith(".ttf")]:
            out = os.path.join(proof_dir, os.path.basename(path).replace(".ttf", ".png"))
            sheet(path, sorted(dems[16]) if "pixel" in path else None, out)
        print("note FreeType colour proof sheets written to the proof directory")

    print(f"\n{len(failures)} failures")
    return 1 if failures else 0


def sheet(path, cps, out, px=48, per_row=20):
    """A FreeType rendering of every colour glyph at `px`, on white, as an RGB PNG."""
    face = freetype.Face(path)
    face.set_pixel_sizes(0, px)
    if cps is None:
        font = TTFont(path)
        cps = sorted(cp for cp, g in font.getBestCmap().items() if g in font["COLR"].ColorLayers)
    cell = px + px // 4
    w = per_row * cell + 8
    h = ((len(cps) + per_row - 1) // per_row) * cell + 8
    canvas = [[[255, 255, 255] for _ in range(w)] for _ in range(h)]
    for i, cp in enumerate(cps):
        ox, oy = 4 + (i % per_row) * cell, 4 + (i // per_row) * cell
        face.load_char(chr(cp), freetype.FT_LOAD_COLOR | freetype.FT_LOAD_RENDER)
        g = face.glyph
        bm = g.bitmap
        for y in range(bm.rows):
            for x in range(bm.width):
                b, gg, r, a = bm.buffer[y * bm.pitch + 4 * x: y * bm.pitch + 4 * x + 4]
                cx, cy = ox + g.bitmap_left + x, oy + int(px * 0.85) - g.bitmap_top + y
                if 0 <= cx < w and 0 <= cy < h and a:
                    p = canvas[cy][cx]
                    canvas[cy][cx] = [r + p[0] * (255 - a) // 255, gg + p[1] * (255 - a) // 255, b + p[2] * (255 - a) // 255]
    raw = b"".join(b"\x00" + bytes(v for px_ in row for v in px_) for row in canvas)

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    with open(out, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else None))
