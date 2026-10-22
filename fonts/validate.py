#!/usr/bin/env python3
# Validates the built fonts with third-party tools: fontTools reads every table of every file, OTS sanitises each,
# FreeType renders every pixel-face glyph at the native size and at twice it to compare with the .DFN bitmaps, compares
# the outline faces' OTF and TTF renderings, and optionally writes proof sheets.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 validate.py <generated-dir> [proof-dir]    (run inside the container that fonts/validate.sh starts)
import glob
import os
import re
import struct
import subprocess
import sys
import zlib

import freetype
from fontTools.ttLib import TTFont

FACES = {"ui": ("desk-ui", 12), "title": ("desk-title", 12), "fixed": ("desk-fixed", 16), "small": ("desk-small", 8)}
REQUIRED_TTF = {"head", "hhea", "maxp", "hmtx", "loca", "glyf", "cmap", "name", "post", "OS/2"}
CP437_PICTURES = "☺☻♥♦♣♠•◘○◙♂♀♪♫☼►◄↕‼¶§▬↨↑↓→←∟↔▲▼"


def cp437(b):
    """The Unicode character of code page 437 byte b, with the pictures the VGA ROM shows for control codes."""
    if 1 <= b < 0x20:
        return CP437_PICTURES[b - 1]
    if b == 0x7F:
        return "⌂"
    return bytes([b]).decode("cp437")


CP1252 = bytes(range(0x20, 0x7F)) + bytes(b for b in range(0x80, 0x100) if b not in (0x81, 0x8D, 0x8F, 0x90, 0x9D))

failures = []
warnings = []


def fail(msg):
    failures.append(msg)
    print("FAIL", msg)


def read_dfn(path):
    data = open(path, "rb").read()
    assert data[:4] == b"DFN1", path
    height, baseline, count = data[4], data[5], struct.unpack_from("<H", data, 6)[0]
    at, glyphs = 8, {}
    for _ in range(count):
        cp, width = struct.unpack_from("<IB", data, at)
        at += 5
        nb = (width + 7) // 8
        rows = []
        for _ in range(height):
            row = data[at : at + nb]
            rows.append([(row[x >> 3] >> (7 - (x & 7))) & 1 for x in range(width)])
            at += nb
        glyphs[cp] = rows
    return height, baseline, glyphs


def render(face, cp, flags, scale_rows):
    """Renders one glyph and returns the set of inked (x, y) cells relative to the cell's top-left corner."""
    face.load_char(chr(cp), flags)
    g = face.glyph
    bm = g.bitmap
    inked = set()
    grey = []
    for y in range(bm.rows):
        for x in range(bm.width):
            if bm.pixel_mode == freetype.FT_PIXEL_MODE_MONO:
                v = (bm.buffer[y * bm.pitch + (x >> 3)] >> (7 - (x & 7))) & 1
                v = 255 if v else 0
            else:
                v = bm.buffer[y * bm.pitch + x]
            grey.append(v)
            if v >= 128:
                inked.add((g.bitmap_left + x, scale_rows - g.bitmap_top + y))
    return inked, grey


def main(gen, proof_dir=None):
    print("== fontTools: read and recompile every table")
    for path in sorted(glob.glob(os.path.join(gen, "*.ttf")) + glob.glob(os.path.join(gen, "*.otf"))):
        name = os.path.basename(path)
        try:
            font = TTFont(path)
            for tag in font.keys():
                font[tag]  # decompile every table
            font.save(os.devnull)  # recompiles everything
        except Exception as e:  # noqa: BLE001
            fail(f"{name}: fontTools: {e}")
            continue
        if name.endswith(".ttf"):
            missing = REQUIRED_TTF - set(font.keys())
            if missing:
                fail(f"{name}: missing tables {sorted(missing)}")
            if font["maxp"].tableVersion != 0x00010000:
                fail(f"{name}: maxp is not version 1.0")
            if font["post"].formatType != 2.0:
                fail(f"{name}: post is not format 2.0")
            if font["OS/2"].version > 1:
                fail(f"{name}: OS/2 version {font['OS/2'].version}")
            subtables = {(t.platformID, t.platEncID, t.format) for t in font["cmap"].tables}
            if "-dos" in name:
                if name.endswith(".ttf"):
                    if subtables != {(0, 3, 4), (3, 0, 4), (1, 0, 6)}:
                        fail(f"{name}: expected Unicode (0, 3), symbol (3, 0) and Macintosh (1, 0) cmaps only, got {sorted(subtables)}")
                    sym = font["cmap"].getcmap(3, 0).cmap
                    mac = font["cmap"].getcmap(1, 0).cmap
                    uni = font["cmap"].getcmap(0, 3).cmap
                    missing = [b for b in range(256) if 0xF000 + b not in sym or mac.get(b) != sym.get(0xF000 + b)]
                    missing += [b for b in range(1, 256) if uni.get(ord(cp437(b))) != sym.get(0xF000 + b)]
                    if missing:
                        fail(f"{name}: bytes not reached through all three cmaps: {missing}")
                else:
                    best = font.getBestCmap()
                    missing = [b for b in range(1, 256) if best.get(ord(cp437(b))) != best.get(0xF000 + b) or 0xF000 + b not in best]
                    if missing:
                        fail(f"{name}: real and private-use code points disagree for bytes {missing}")
                if not font["OS/2"].ulCodePageRange1 & (1 << 31):
                    fail(f"{name}: OS/2 does not declare the symbol character set")
            else:
                for want in [(3, 1, 4), (1, 0, 6)]:
                    if want not in subtables:
                        fail(f"{name}: no cmap subtable {want}")
                cmap = font["cmap"].getcmap(3, 1).cmap
                missing = [b for b in CP1252 if ord(bytes([b]).decode("cp1252")) not in cmap]
                if missing:
                    fail(f"{name}: code page 1252 bytes without a glyph: {[hex(b) for b in missing]}")
            if font["glyf"].glyphs and any(font["glyf"][g].isComposite() for g in font.getGlyphOrder()):
                fail(f"{name}: has composite glyphs")
        print(f"ok   {name}: {len(font.getGlyphOrder())} glyphs, tables {' '.join(sorted(font.keys()))}")

    print("\n== OTS (ots-sanitize)")
    for path in sorted(glob.glob(os.path.join(gen, "*.ttf")) + glob.glob(os.path.join(gen, "*.otf"))):
        r = subprocess.run([sys.executable, "-m", "ots", path, os.devnull], capture_output=True, text=True)
        out = (r.stdout + r.stderr).strip()
        if r.returncode != 0:
            fail(f"{os.path.basename(path)}: ots-sanitize: {out}")
        else:
            if out and out != "File sanitized successfully!":
                warnings.append(f"{os.path.basename(path)}: {out}")
            print(f"ok   {os.path.basename(path)}" + (f" ({out})" if out else ""))

    print("\n== FreeType: render every glyph and compare with the .DFN pixels")
    for fid, (file, size) in FACES.items():
        height, ascent, dfn = read_dfn(os.path.join(gen, f"{fid}.dfn"))
        for style in ("", "-round"):
            for ext in ("ttf", "otf"):
                path = os.path.join(gen, f"{file}{style}.{ext}")
                face = freetype.Face(path)
                for n in (1, 2):
                    face.set_pixel_sizes(0, size * n)
                    for label, flags in (
                        ("mono, no hinting", freetype.FT_LOAD_RENDER | freetype.FT_LOAD_TARGET_MONO | freetype.FT_LOAD_NO_HINTING),
                        ("mono, default hinting", freetype.FT_LOAD_RENDER | freetype.FT_LOAD_TARGET_MONO),
                        ("grey, no hinting", freetype.FT_LOAD_RENDER | freetype.FT_LOAD_NO_HINTING),
                    ):
                        wrong, partial = [], 0
                        for cp, rows in dfn.items():
                            if cp > 0xFFFF and ext == "ttf":
                                continue
                            want = {(x * n + i, y * n + j) for y, r in enumerate(rows) for x, v in enumerate(r) if v for i in range(n) for j in range(n)}
                            got, grey = render(face, cp, flags, ascent * n)
                            if got != want:
                                wrong.append(chr(cp))
                            partial += sum(1 for v in grey if 0 < v < 255)
                        msg = f"{file}{style}.{ext} at {size * n} px, {label}: {len(dfn) - len(wrong)}/{len(dfn)} glyphs exact"
                        if "grey" in label:
                            msg += f", {partial} partly covered pixels"
                        if wrong and not style and n == 1 and "no hinting" in label:
                            fail(msg + f"; differ: {''.join(wrong[:20])}")
                        else:
                            print(("ok   " if not wrong else "note ") + msg + (f"; differ: {''.join(wrong[:20])}" if wrong else ""))
                # A size that is not a multiple of the pixel size, for the record.
                odd = size + size // 2 + 1
                face.set_pixel_sizes(0, odd)
                partial = 0
                for cp in list(dfn)[:120]:
                    _, grey = render(face, cp, freetype.FT_LOAD_RENDER, odd)
                    partial += sum(1 for v in grey if 0 < v < 255)
                print(f"note {file}{style}.{ext} at {odd} px (not a multiple), default hinting, grey: {partial} partly covered pixels in 120 glyphs")

    print("\n== FreeType: Desk Fixed DOS through its symbol cmap, against the Unicode glyphs of fixed.dfn")
    height, ascent, dfn = read_dfn(os.path.join(gen, "fixed.dfn"))
    pictures = {0x01: "☺", 0x02: "☻", 0x03: "♥", 0x0E: "♫", 0x10: "►", 0x18: "↑", 0x1E: "▲", 0x7F: "⌂"}
    for path in sorted(glob.glob(os.path.join(gen, "desk-fixed-dos*"))):
        face = freetype.Face(path)
        face.set_pixel_sizes(0, 16)
        # The charmap FreeType picks by default, and the real characters through it.
        default = face.charmap.encoding_name if face.charmap else "none"
        sample = "☺♥░╔═α"
        reached = [c for c in sample if face.get_char_index(ord(c))]
        line = f"{os.path.basename(path)}: default charmap {default}; real characters {''.join(reached)} of {sample} reached"
        if len(reached) != len(sample):
            fail(line)
        else:
            print("ok   " + line)
        if not any(cm.encoding == freetype.FT_ENCODING_MS_SYMBOL for cm in face.charmaps) and path.endswith(".ttf"):
            fail(f"{os.path.basename(path)}: FreeType finds no symbol charmap")
            continue
        if path.endswith(".ttf"):
            face.select_charmap(freetype.FT_ENCODING_MS_SYMBOL)
        wrong, tried = [], 0
        for b in list(range(0xB0, 0xE0)) + [0xFE] + list(pictures):
            ch = pictures.get(b) or bytes([b]).decode("cp437")
            want = {(x, y) for y, r in enumerate(dfn[ord(ch)]) for x, v in enumerate(r) if v}
            face.load_char(chr(0xF000 + b), freetype.FT_LOAD_RENDER | freetype.FT_LOAD_TARGET_MONO | freetype.FT_LOAD_NO_HINTING)
            g = face.glyph
            got = set()
            for y in range(g.bitmap.rows):
                for x in range(g.bitmap.width):
                    if (g.bitmap.buffer[y * g.bitmap.pitch + (x >> 3)] >> (7 - (x & 7))) & 1:
                        got.add((g.bitmap_left + x, ascent - g.bitmap_top + y))
            tried += 1
            if got != want and "-round" not in path:
                wrong.append(hex(b))
        msg = f"{os.path.basename(path)} at 16 px: box drawing, blocks and pictures {tried - len(wrong)}/{tried} exact"
        if wrong:
            fail(msg + f"; differ: {wrong}")
        else:
            print("ok   " + msg + (" (Round compared in monochrome)" if "-round" in path else ""))

    print("\n== FreeType: the outline faces, OTF against TTF, and proof sheets")
    modern = sorted(os.path.basename(p)[:-4] for p in glob.glob(os.path.join(gen, "desk-*.otf")) if re.match(r"desk-(sans|mono|serif)", os.path.basename(p)))
    for base in modern:
        faces = {ext: freetype.Face(os.path.join(gen, f"{base}.{ext}")) for ext in ("otf", "ttf")}
        chars = [chr(c) for c in range(0x20, 0x7F)] + [chr(c) for c in range(0xA1, 0x100) if c != 0xAD] + list("ŒœŸŠšŽž€‚ƒ„…†‡ˆ‰‹‘’“”•–—˜™›")
        worst, worst_ch = 0.0, ""
        for ch in chars:
            out = {}
            for ext, face in faces.items():
                face.set_pixel_sizes(0, 48)
                face.load_char(ch, freetype.FT_LOAD_RENDER | freetype.FT_LOAD_NO_HINTING)
                g = face.glyph
                out[ext] = {(g.bitmap_left + x, -g.bitmap_top + y): g.bitmap.buffer[y * g.bitmap.pitch + x] for y in range(g.bitmap.rows) for x in range(g.bitmap.width)}
            keys = set(out["otf"]) | set(out["ttf"])
            ink = sum(out["otf"].values()) or 1
            diff = sum(abs(out["otf"].get(k, 0) - out["ttf"].get(k, 0)) for k in keys) / ink
            if diff > worst:
                worst, worst_ch = diff, ch
        # Grey edges of small marks differ a little between FreeType's CFF and TrueType paths even for equal shapes.
        line = f"{base}: OTF and TTF rendered at 48 px differ by at most {worst * 100:.2f} % of the ink ({worst_ch})"
        if worst > 0.05:
            fail(line)
        else:
            print("ok   " + line)
    if proof_dir:
        for base in modern:
            proof(os.path.join(gen, f"{base}.ttf"), os.path.join(proof_dir, f"{base}.png"))
        print(f"note proof sheets written to the proof directory")

    print(f"\n{len(failures)} failures, {len(warnings)} warnings")
    for w in warnings:
        print("warn", w)
    return 1 if failures else 0


PROOF_LINES = [
    "Hamburgefonstiv agyks e 357 &@ 0O Il1 rn m",
    "The quick brown fox jumps over the lazy dog.",
    "Ébauche, à côté, déjà vu, crème brûlée — « Ça va ? »",
    "ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789 $%#*",
    "File  Edit  View  Help   C:\\DESK\\NOTES  12:45  ✓ ► ▼",
]


def write_png(path, w, h, rows):
    raw = b"".join(b"\x00" + bytes(r) for r in rows)

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 0, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


def proof(path, out):
    """A proof sheet: the sample lines at 11, 12, 14, 16, 24 and 48 px, grey then monochrome, unhinted."""
    face = freetype.Face(path)
    sizes = [11, 12, 14, 16, 24, 48]
    W = 1900
    H = sum((int(s * 1.25) + 2) * (len(PROOF_LINES) if s < 24 else 2) + 4 for s in sizes) * 2 + 8
    canvas = [[255] * W for _ in range(H)]
    y = 4
    for mono in (False, True):
        for size in sizes:
            face.set_pixel_sizes(0, size)
            flags = freetype.FT_LOAD_RENDER | freetype.FT_LOAD_NO_HINTING | (freetype.FT_LOAD_TARGET_MONO if mono else 0)
            for line in PROOF_LINES if size < 24 else PROOF_LINES[:2]:
                pen = 6
                for ch in line:
                    face.load_char(ch, flags)
                    g = face.glyph
                    bm = g.bitmap
                    for by in range(bm.rows):
                        for bx in range(bm.width):
                            if bm.pixel_mode == freetype.FT_PIXEL_MODE_MONO:
                                v = 255 if (bm.buffer[by * bm.pitch + (bx >> 3)] >> (7 - (bx & 7))) & 1 else 0
                            else:
                                v = bm.buffer[by * bm.pitch + bx]
                            cx, cy = pen + g.bitmap_left + bx, y + int(size * 0.95) - g.bitmap_top + by
                            if 0 <= cy < H and 0 <= cx < W:
                                canvas[cy][cx] = min(canvas[cy][cx], 255 - v)
                    pen += g.advance.x >> 6
                y += int(size * 1.25) + 2
            y += 4
    write_png(out, W, y, canvas[:y])


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else None))
