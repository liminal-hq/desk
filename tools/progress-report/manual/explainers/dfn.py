# A glyph out of a .DFN file: the header of fonts/generated/ui.dfn field by field, then the bitmap of one glyph row by row.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# The file is read with a replica of `Font::parse` in `shell/font/src/lib.rs` (the same checks, in the same order). The explainer refuses a
# file the replica would refuse, a header that disagrees with what `docs/FORMATS.md` and `docs/FIRST-WINDOW.md` say about the four faces
# (the magic, the cell height and baseline, a glyph count between 289 and 332), and a font file whose glyph for the chosen character is missing.
import os
import re
import struct

import gather
from explainers import stepper
from mdlite import Raw, esc

ID = "m-dfn"
FILE = "fonts/generated/ui.dfn"
CHAR = "A"
# From docs/FIRST-WINDOW.md ("desk-font"): the cells and baselines of ui, title, fixed and small, and the range of their glyph counts.
EXPECT = {"ui": (14, 11), "title": (14, 12), "fixed": (16, 13), "small": (8, 7)}
COUNT_RANGE = (289, 332)


def src(path):
    with open(os.path.join(gather.ROOT, path), encoding="utf-8") as f:
        return f.read()


def parse(b):
    """Font::parse of shell/font/src/lib.rs: (height, baseline, count, glyphs) with glyphs [(code, width, offset, rows)], or ValueError."""
    if b[:4] != b"DFN1"[:len(b[:4])] or len(b) < 4:
        raise ValueError("bad magic")
    if len(b) < 8:
        raise ValueError("truncated header")
    height, baseline, count = b[4], b[5], struct.unpack("<H", b[6:8])[0]
    if height == 0:
        raise ValueError("zero height")
    if baseline > height:
        raise ValueError("baseline out of the cell")
    at, prev, glyphs = 8, None, []
    for i in range(count):
        if len(b) < at + 5:
            raise ValueError("truncated at glyph %d (offset %d)" % (i, at))
        code, width = struct.unpack("<I", b[at:at + 4])[0], b[at + 4]
        size = height * -(-width // 8)
        if len(b) - at - 5 < size:
            raise ValueError("truncated at glyph %d (offset %d)" % (i, at))
        if prev is not None and code <= prev:
            raise ValueError("glyph %d is out of order" % i)
        prev = code
        glyphs.append((code, width, at, b[at + 5:at + 5 + size]))
        at += 5 + size
    if at != len(b):
        raise ValueError("%d trailing bytes" % (len(b) - at))
    return height, baseline, count, glyphs


def check(b, name="ui", formats=None, first=None):
    problems = []
    formats = formats if formats is not None else src("docs/FORMATS.md")
    first = first if first is not None else src("docs/FIRST-WINDOW.md")
    if "DFN1" not in formats or "height (1 byte" not in formats or "glyph count (2 bytes)" not in formats:
        problems.append("docs/FORMATS.md no longer describes the DFN1 header the explainer reads (magic, height, baseline, a two-byte glyph count)")
    try:
        height, baseline, count, glyphs = parse(b)
    except ValueError as e:
        return ["the file is refused by the parser replica: %s" % e]
    want = EXPECT[name]
    if (height, baseline) != want:
        problems.append("the %s face has a cell of %d rows and a baseline of %d, but docs/FIRST-WINDOW.md says %d and %d" % (name, height, baseline, want[0], want[1]))
    if not COUNT_RANGE[0] <= count <= COUNT_RANGE[1]:
        problems.append("the %s face has %d glyphs, outside the %d to %d that docs/FIRST-WINDOW.md gives" % (name, count, *COUNT_RANGE))
    if "%d to %d glyphs" % COUNT_RANGE not in first and "289 to 332 glyphs" not in first:
        problems.append("docs/FIRST-WINDOW.md no longer states the glyph count range")
    if not any(g[0] == ord(CHAR) for g in glyphs):
        problems.append("the face has no glyph for %r" % CHAR)
    return problems


def render(D, ctx):
    with open(os.path.join(gather.ROOT, FILE), "rb") as f:
        b = f.read()
    problems = check(b)
    if problems:
        raise ValueError("the .DFN explainer refuses its sources: " + "; ".join(problems))
    height, baseline, count, glyphs = parse(b)
    code, width, at, rows = [g for g in glyphs if g[0] == ord(CHAR)][0]
    index = [g[0] for g in glyphs].index(code)
    stride = -(-width // 8)
    CELL, GX, GY = 15, 20, 36
    o = ['<svg viewBox="0 0 760 %d" class="diagram m-dfnfig" role="img" aria-label="The glyph %s of the UI face as a grid of pixels, row by row, with the bytes of each row">' % (GY + height * CELL + 40, CHAR)]
    o.append('<text x="%d" y="22" font-size="12" class="d-muted">%s: the glyph %s, %d pixels wide, %d rows</text>' % (GX, esc(FILE), CHAR, width, height))
    first_row_step = 6
    for y in range(height):
        step = first_row_step + y
        rowbytes = rows[y * stride:(y + 1) * stride]
        for x in range(width):
            on = rowbytes[x // 8] & (0x80 >> (x % 8))
            o.append('<rect x="%d" y="%d" width="%d" height="%d" class="df-%s" data-from="%d"/>' % (GX + x * CELL, GY + y * CELL, CELL - 1, CELL - 1, "on" if on else "off", step))
        o.append('<text x="%d" y="%d" font-size="12" class="df-hex" data-from="%d">%s</text>' % (GX + (width + 1) * CELL, GY + y * CELL + 12, step, esc(" ".join("%02X" % v for v in rowbytes) + "   " + "".join(format(v, "08b") for v in rowbytes)[:width])))
        o.append('<rect x="%d" y="%d" width="%d" height="%d" fill="none" class="df-cur" data-at="%d" stroke-width="2"/>' % (GX - 2, GY + y * CELL - 1, width * CELL + 3, CELL + 1, step))
    base_y = GY + baseline * CELL - 1
    o.append('<line x1="%d" y1="%d" x2="%d" y2="%d" class="df-base" data-from="%d"/><text x="%d" y="%d" font-size="11" class="d-muted" data-from="%d">baseline: %d rows above this line</text>' % (
        GX - 6, base_y, GX + width * CELL + 6, base_y, first_row_step + height, GX + width * CELL + 14, base_y + 12, first_row_step + height, baseline))
    o.append("</svg>")
    head = " ".join("%02X" % v for v in b[:8])
    steps = [
        ("The file", Raw(esc("%s is %s bytes. It begins with an eight-byte header and is followed by %d glyph records, one after the other, with no index: the header and the records are all there is. The header bytes are %s." % (FILE, "{:,}".format(len(b)), count, head)))),
        ("Bytes 0 to 3: the magic", Raw(esc("%s = %r: DFN1, 'DFN version 1'. The reader refuses any other four bytes, so a file of another format fails at once." % (" ".join("%02X" % v for v in b[:4]), b[:4].decode())))),
        ("Byte 4: the cell height", Raw(esc("%02X = %d. Every glyph has exactly this many rows, so the whole cell is %d pixels tall and the line spacing follows from it. A height of zero is refused." % (b[4], height, height)))),
        ("Byte 5: the baseline", Raw(esc("%02X = %d. The baseline is the number of rows above the line the letters stand on; here %d rows above and %d below. A baseline larger than the height is refused." % (b[5], baseline, baseline, height - baseline)))),
        ("Bytes 6 and 7: the glyph count", Raw(esc("%02X %02X, little-endian = %d. The reader walks that many records and then requires the file to end exactly there. docs/FIRST-WINDOW.md says the four faces have 289 to 332 glyphs." % (b[6], b[7], count)))),
        ("A glyph record: finding %s" % CHAR, Raw(esc("A record is a four-byte code point, a one-byte width (the advance) and height rows of ceil(width / 8) bytes. Records are in ascending code-point order, which is what lets the reader find a glyph by binary search. The record for U+%04X is number %d (counting from 0) at byte offset %d: the code point %s, the width %d (%d byte%s per row), and then %d rows." % (
            code, index, at, " ".join("%02X" % v for v in b[at:at + 4]), width, stride, "" if stride == 1 else "s", height)))),
    ]
    for y in range(height):
        rowbytes = rows[y * stride:(y + 1) * stride]
        bitsrow = "".join(format(v, "08b") for v in rowbytes)[:width]
        steps.append(("Row %d" % y, Raw(esc("Row %d is %s, which is %s. The leftmost pixel is the highest bit of the first byte; a set bit is a pixel in the foreground colour. %s" % (y, " ".join("%02X" % v for v in rowbytes), bitsrow.replace("1", "#").replace("0", "."), ("This row is above the baseline." if y < baseline else "This row is below the baseline.") if y in (baseline - 1, baseline) else "")))))
    cap = ('Read from <code>%s</code> with a replica of <code>Font::parse</code> in <code>shell/font/src/lib.rs</code>. The build refuses a file the parser would refuse, a header that disagrees with <code>docs/FORMATS.md</code> or with the cells, baselines and glyph counts of <code>docs/FIRST-WINDOW.md</code>, and a face with no glyph for the chosen character.' % FILE)
    return stepper(ID, "A glyph out of a .DFN file, byte by byte", "".join(o), steps, cap, ctx, "x-wide")
