# Checks of the surfaces chapter (M13) and the text chapter (M14): the damage numbers, layouts and font bytes they quote, against the sources and transcripts.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import os
import re
import struct
import sys
import unittest
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
TOOL = os.path.normpath(os.path.join(HERE, ".."))
sys.path.insert(0, TOOL)

import gather  # noqa: E402
import manual  # noqa: E402


def rd(*parts):
    with open(os.path.join(gather.ROOT, *parts), encoding="utf-8") as f:
        return f.read()


def rb(*parts):
    with open(os.path.join(gather.ROOT, *parts), "rb") as f:
        return f.read()


# A replica of desk_gfx::Rect and Damage (rect.rs and damage.rs), enough to replay the first window's scenario.
def area(r):
    return max(0, r[2] - r[0]) * max(0, r[3] - r[1])


def empty(r):
    return r[2] <= r[0] or r[3] <= r[1]


def union(a, b):
    if empty(a):
        return b if not empty(b) else (0, 0, 0, 0)
    if empty(b):
        return a
    return (min(a[0], b[0]), min(a[1], b[1]), max(a[2], b[2]), max(a[3], b[3]))


def intersect(a, b):
    r = (max(a[0], b[0]), max(a[1], b[1]), min(a[2], b[2]), min(a[3], b[3]))
    return (0, 0, 0, 0) if empty(r) else r


def inflate(r, by):
    return (0, 0, 0, 0) if empty(r) else (r[0] - by, r[1] - by, r[2] + by, r[3] + by)


def align_x(r, a):
    if a <= 1 or empty(r):
        return r
    return ((r[0] // a) * a, r[1], -((-r[2]) // a) * a, r[3])


def mergeable(a, b):
    if empty(a) or empty(b):
        return False
    x_overlap, y_overlap = a[0] < b[2] and b[0] < a[2], a[1] < b[3] and b[1] < a[3]
    x_touch, y_touch = a[0] <= b[2] and b[0] <= a[2], a[1] <= b[3] and b[1] <= a[3]
    return (x_overlap and y_touch) or (y_overlap and x_touch)


class Damage:
    def __init__(self, bounds, max_rects):
        self.bounds, self.max, self.rects = bounds, max_rects, []

    def _set(self, items):
        while True:
            for i in range(len(items)):
                for j in range(i + 1, len(items)):
                    if mergeable(items[i], items[j]):
                        items[i] = union(items[i], items[j])
                        items[j] = items[-1]
                        items.pop()
                        break
                else:
                    continue
                break
            else:
                if len(items) > self.max:
                    best = None
                    for i in range(len(items)):
                        for j in range(i + 1, len(items)):
                            u = union(items[i], items[j])
                            extra = area(u) - area(items[i]) - area(items[j])
                            if best is None or extra < best[0]:
                                best = (extra, i, j)
                    _, i, j = best
                    items[i] = union(items[i], items[j])
                    items[j] = items[-1]
                    items.pop()
                    continue
                break
        self.rects = items

    def add(self, r):
        r = intersect(r, self.bounds)
        if not empty(r):
            self._set(self.rects + [r])

    def add_all(self, other):
        for r in other.rects:
            self.add(r)

    def align(self, a):
        self._set([intersect(align_x(r, a), self.bounds) for r in self.rects])

    def area(self):
        return sum(area(r) for r in self.rects)


def parse_dfn(path):
    b = rb(*path.split("/"))
    assert b[:4] == b"DFN1"
    height, baseline, count = b[4], b[5], struct.unpack("<H", b[6:8])[0]
    at, glyphs, offsets = 8, {}, {}
    for _ in range(count):
        code = struct.unpack("<I", b[at:at + 4])[0]
        width = b[at + 4]
        size = height * ((width + 7) // 8)
        glyphs[code] = (width, b[at + 5:at + 5 + size])
        offsets[code] = at
        at += 5 + size
    assert at == len(b)
    return {"height": height, "baseline": baseline, "count": count, "size": len(b), "glyphs": glyphs, "offsets": offsets, "bytes": b}


def advance(font, text):
    g = font["glyphs"]
    return sum(g.get(ord(c), g[ord("?")])[0] for c in text)


class SurfacesChapterTests(unittest.TestCase):
    """M13: the damage numbers of the worked example follow from the merge rules and are in the transcripts."""

    @classmethod
    def setUpClass(cls):
        cls.chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        cls.text = cls.chapters["m-pixels"]["raw"]
        cls.damage_src = rd("shell", "gfx", "src", "damage.rs")
        cls.max_rects = int(re.search(r"pub const MAX_RECTS: usize = (\d+);", cls.damage_src).group(1))
        cls.scn = rd("tests", "shell", "first-window.scn")
        cls.transcripts = {d: rd("tests", "native", "expected-shell-%d.txt" % d) for d in (8, 4)}

    def expected(self, depth):
        out = {}
        for m in re.finditer(r"^shell: damage (\w+): (\d+) pixels repainted, (\d+) presented in (\d+) rectangles$", self.transcripts[depth], re.M):
            out[m.group(1)] = tuple(int(m.group(i)) for i in (2, 3, 4))
        return out

    def windows(self):
        wins = {}
        for m in re.finditer(r'^open (\w+) (\d+) (\d+) (\d+) (\d+) "', self.scn, re.M):
            n, x, y, w, h = m.group(1), *map(int, m.groups()[1:])
            wins[n] = (x, y, x + w, y + h)
        return wins

    def extent(self, outer):
        return inflate(outer, 3)

    def replay(self, depth):
        bounds = (0, 0, 640, 480)
        align = 8 if depth == 4 else 1
        wins = self.windows()
        notes, welcome = wins["notes"], wins["welcome"]
        scene, overlay = Damage(bounds, self.max_rects), Damage(bounds, self.max_rects)
        cursor = [320, 240]
        out = {}

        def rect_at(p):
            return (p[0], p[1], p[0] + 12, p[1] + 19)

        def move(x, y):
            overlay.add(rect_at(cursor))
            cursor[:] = [x, y]
            overlay.add(rect_at(cursor))

        def frame(name):
            present = Damage(bounds, self.max_rects)
            present.rects = list(scene.rects)
            present.add_all(overlay)
            present.align(align)
            out[name] = (scene.area(), present.area(), len(present.rects))
            scene.rects, overlay.rects = [], []

        scene.add(bounds)
        frame("open")
        move(420, 300)  # pointer 320 240 then move 100 60
        frame("moved")
        move(120, 70)  # a click on Notes' title bar raises it: its extent and Welcome's
        scene.add(self.extent(notes))
        scene.add(self.extent(welcome))
        frame("focus")
        move(160, 100)  # the drag by (40, 30)
        old = self.extent(notes)
        notes = (notes[0] + 40, notes[1] + 30, notes[2] + 40, notes[3] + 30)
        scene.add(old)
        scene.add(self.extent(notes))
        frame("dragged")
        move(400, 300)  # a click in Welcome's content focuses it; then "desk"
        scene.add(self.extent(welcome))
        scene.add(self.extent(notes))
        ui = parse_dfn("fonts/generated/ui.dfn")
        field = (welcome[0] + 12, welcome[1] + 23 + 36, welcome[0] + 12 + 300, welcome[1] + 23 + 36 + 20)
        text = ""
        for c in "desk":
            before = self.ink(ui, field, text)
            text += c
            scene.add(intersect(union(before, self.ink(ui, field, text)), field))
        frame("typed")
        move(543, 161)  # the close box
        scene.add(self.extent(welcome))
        scene.add(self.extent(notes))
        frame("closed")
        return out

    @staticmethod
    def ink(font, field, text):
        cell = (field[0] + 6, field[1] + 3, field[0] + 6 + advance(font, text), field[1] + 3 + font["height"])
        caret = (cell[2], field[1] + 4, cell[2] + 1, field[1] + 4 + 12)
        return union(cell, caret)

    def test_the_replayed_damage_equals_both_native_transcripts(self):
        for depth in (8, 4):
            self.assertEqual(self.replay(depth), self.expected(depth), "depth %d" % depth)

    def test_the_moved_frame_arithmetic_in_the_chapter(self):
        self.assertEqual(self.max_rects, 16)
        old, new = (320, 240, 332, 259), (420, 300, 432, 319)
        self.assertFalse(mergeable(old, new))
        self.assertEqual(area(old), 228)
        self.assertEqual(2 * area(old), 456)
        self.assertEqual((align_x(old, 8), align_x(new, 8)), ((320, 240, 336, 259), (416, 300, 432, 319)))
        self.assertEqual(2 * 16 * 19, 608)
        self.assertEqual(self.expected(8)["moved"], (0, 456, 2))
        self.assertEqual(self.expected(4)["moved"], (0, 608, 2))
        self.assertIn("expect-damage 8 456 2", self.scn)
        self.assertIn("expect-damage 4 608 2", self.scn)
        for needle in ("456", "608", "(320, 240)", "(420, 300)", "16 × 19 = 304"):
            self.assertIn(needle, self.text)

    def test_the_merged_boxes_in_the_chapter_follow_from_the_extents(self):
        wins = self.windows()
        notes, welcome = wins["notes"], wins["welcome"]
        self.assertEqual((notes, welcome), ((60, 60, 360, 240), (220, 150, 560, 350)))
        en, ew = self.extent(notes), self.extent(welcome)
        self.assertEqual((en, ew), ((57, 57, 363, 243), (217, 147, 563, 353)))
        self.assertTrue(mergeable(en, ew))
        focus = union(en, ew)
        self.assertEqual((focus, area(focus), 506 * 296), ((57, 57, 563, 353), 149776, 149776))
        moved = (notes[0] + 40, notes[1] + 30, notes[2] + 40, notes[3] + 30)
        drag = union(en, self.extent(moved))
        self.assertEqual((self.extent(moved), drag, area(drag)), ((97, 87, 403, 273), (57, 57, 403, 273), 74736))
        typed = union(ew, self.extent(moved))
        self.assertEqual((typed, area(typed)), ((97, 87, 563, 353), 123956))
        self.assertEqual([area(align_x(r, 8)) for r in (focus, drag, typed)], [151552, 76032, 125552])
        self.assertEqual(640 * 480, 307200)
        for n in ("149,776", "74,736", "123,956", "151,552", "76,032", "125,552", "307,200"):
            self.assertIn(n, self.text)
        e8, e4 = self.expected(8), self.expected(4)
        self.assertEqual((e8["focus"], e8["dragged"], e8["typed"], e8["closed"]), ((149776,) * 2 + (1,), (74736,) * 2 + (1,), (123956,) * 2 + (1,), (123956,) * 2 + (1,)))
        self.assertEqual((e4["focus"][1], e4["dragged"][1], e4["typed"][1]), (151552, 76032, 125552))

    def test_corners_do_not_merge_and_edges_do(self):
        self.assertTrue(mergeable((0, 0, 10, 10), (10, 0, 20, 10)))
        self.assertFalse(mergeable((0, 0, 10, 10), (10, 10, 20, 20)))
        self.assertIn("pub const fn new", rd("shell", "gfx", "src", "rect.rs"))
        self.assertIn("(x_overlap && y_touch) || (y_overlap && x_touch)", self.damage_src)

    def test_the_digests_the_chapter_quotes_are_the_sums(self):
        sums = rd("tests", "shell", "first-window.sums")
        self.assertIn("open 8 640x480 920263aa c310363f", sums)
        for token in ("920263aa", "c310363f"):
            self.assertIn(token, self.text)
        self.assertEqual(zlib.crc32(b"123456789"), 0xCBF43926)
        self.assertIn("0xEDB8_8320", rd("shell", "gfx", "src", "digest.rs"))
        self.assertEqual(640 * 480 * 3, 921600)
        self.assertEqual(len([ln for ln in sums.splitlines() if ln.strip()]), 12)

    def test_the_planar_examples_follow_from_planarise(self):
        def planarise(group):
            planes = [0, 0, 0, 0]
            for i, px in enumerate(group):
                for p in range(4):
                    planes[p] |= ((px >> p) & 1) << (7 - i)
            return planes

        self.assertEqual(planarise([10] * 8), [0x00, 0xFF, 0x00, 0xFF])
        self.assertEqual(planarise([15] + [0] * 7), [0x80] * 4)
        self.assertEqual(80 * 480, 38400)
        src = rd("shell", "present", "src", "planar.rs")
        self.assertIn("bit = 7 - i", src)
        self.assertIn("pub const MAX_PLANE_BYTES: usize = 160;", src)
        self.assertIn("sequencer_write(self.ports, seq::MAP_MASK, 0x0F);", src)
        self.assertIn("fn align(&self) -> u32 {\n        8\n    }", src)

    def test_the_names_the_chapter_lists_exist(self):
        for path, names in (("shell/gfx/src/compose.rs", ("Compositor", "CursorLayer", "FrameStats", "MAX_WIDTH")),
                            ("shell/gfx/src/present.rs", ("Present", "MemoryPresent", "PresentStats")),
                            ("shell/gfx/src/digest.rs", ("FrameDigest", "Crc32", "crc32")),
                            ("shell/present/src/planar.rs", ("planarise", "PlaneSink", "VgaPlanes", "PlanarPresent")),
                            ("shell/present/src/readback.rs", ("read_back_linear", "read_back_planar"))):
            src = rd(*path.split("/"))
            for n in names:
                self.assertRegex(src, r"\b%s\b" % n)
        self.assertIn("pub const MAX_WIDTH: u32 = 1280;", rd("shell", "gfx", "src", "compose.rs"))
        for crate, d in (("desk-gfx", "gfx"), ("desk-present", "present")):
            self.assertIn('name = "%s"' % crate, rd("shell", d, "Cargo.toml"))
        self.assertIn("sym:shell/gfx/src/damage.rs::mergeable", self.text)


class TextChapterTests(unittest.TestCase):
    """M14: the font headers, the glyph, the chrome layout and the field rectangle against the files and the code."""

    @classmethod
    def setUpClass(cls):
        cls.chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        cls.text = cls.chapters["m-text"]["raw"]
        cls.faces = {n: parse_dfn("fonts/generated/%s.dfn" % n) for n in ("ui", "title", "small", "fixed")}

    def test_the_header_table_is_the_files(self):
        expect = {"ui": (14, 11, 289, 5639), "title": (14, 12, 289, 5975), "small": (8, 7, 289, 3765), "fixed": (16, 13, 332, 6980)}
        for name, (h, bl, n, size) in expect.items():
            f = self.faces[name]
            self.assertEqual((f["height"], f["baseline"], f["count"], f["size"]), (h, bl, n, size), name)
            head = " ".join("%02X" % b for b in f["bytes"][:8])
            self.assertIn("`%s`" % head, self.text)
            self.assertIn("{:,}".format(size), self.text)
            for c in ("?", "…", "×"):
                self.assertIn(ord(c), f["glyphs"])
            self.assertNotIn(0xFFFD, f["glyphs"])
            self.assertEqual(sorted(f["glyphs"]), list(f["glyphs"]))
        self.assertEqual((0x0121, 0x014C), (289, 332))

    def test_the_glyph_d_is_what_the_worked_example_prints(self):
        ui = self.faces["ui"]
        self.assertEqual(ui["offsets"][ord("d")], 1328)
        width, rows = ui["glyphs"][ord("d")]
        self.assertEqual(width, 6)
        self.assertEqual(rows.hex(), "00" "00" "08" "08" "08" "68" "98" "88" "88" "98" "68" "00" "00" "00")
        self.assertIn("00 00 08 08 08 68 98 88 88 98 68 00 00 00", self.text)
        art = ["".join("#" if r & (0x80 >> x) else "." for x in range(6)) for r in rows]
        picture = re.search(r"```\n((?:[.#]{6}\n){14})```", self.text).group(1).split()
        self.assertEqual(art, picture)
        self.assertEqual(ui["bytes"][1328:1332], bytes([0x64, 0, 0, 0]))

    def test_the_widths_the_chapter_quotes(self):
        self.assertEqual(advance(self.faces["title"], "Notes"), 35)
        self.assertEqual(advance(self.faces["title"], "Welcome to Desk"), 99)
        self.assertEqual(advance(self.faces["title"], "…"), 9)
        self.assertEqual(advance(self.faces["ui"], "desk"), 22)
        self.assertEqual([advance(self.faces["ui"], c) for c in "desk"], [6, 6, 5, 5])
        # fit_with_ellipsis with 60 pixels of room
        title = self.faces["title"]
        room, used, end, text = 60 - 9, 0, 0, "Welcome to Desk"
        for i, c in enumerate(text):
            used += title["glyphs"][ord(c)][0]
            if used > room:
                break
            end = i + 1
        self.assertEqual((text[:end], advance(title, text[:end]), advance(title, text[:end]) + 9), ("Welcome", 49, 58))
        self.assertEqual(49 + 4, 53)

    def test_the_chrome_numbers_follow_the_layout_code(self):
        src = rd("shell", "ui", "src", "chrome.rs")
        consts = {k: int(v) for k, v in re.findall(r"const (\w+): (?:u32|i32) = (\d+);", src)}
        self.assertEqual((consts["TITLE_HEIGHT"], consts["HALO"], consts["BOX"], consts["MIN_WIDTH"], consts["MIN_HEIGHT"]), (22, 3, 18, 120, 40))
        self.assertEqual((consts["TITLE_INSET"], consts["TITLE_TOP"], consts["BOX_TOP"], consts["BOX_GAP"], consts["BOX_MARGIN"], consts["TITLE_CELL"]), (8, 4, 2, 2, 8, 14))

        def layout(x, y, w, h):
            step = consts["BOX"] + consts["BOX_GAP"]
            close_x = x + w - consts["BOX_MARGIN"] - consts["BOX"]
            buttons = [close_x - 2 * step, close_x - step, close_x]
            text_x = x + consts["TITLE_INSET"]
            text_end = buttons[0] - consts["BOX_MARGIN"]
            return {"buttons": buttons, "text": (text_x, y + consts["TITLE_TOP"], text_end - text_x), "rule_y": y + 22, "content": (y + 23, h - 23)}

        n = layout(60, 60, 300, 180)
        self.assertEqual(n["buttons"], [294, 314, 334])
        self.assertEqual(n["text"], (68, 64, 218))
        self.assertEqual((n["rule_y"], n["content"]), (82, (83, 157)))
        wl = layout(220, 150, 340, 200)
        self.assertEqual((wl["buttons"][2], wl["buttons"][2] + 18), (534, 552))
        self.assertEqual(wl["text"][2], 258)
        self.assertTrue(534 <= 543 < 552 and 152 <= 161 < 170)
        # exercise 2 of the chapter: outer box (100, 40, 200 x 120)
        self.assertEqual(layout(100, 40, 200, 120)["buttons"][2], 274)
        for needle in ("(334, 62)", "(68, 64), 218 × 14", "(60, 83), 300 × 157", "(57, 57) to (363, 243)", "534 to 552"):
            self.assertIn(needle, self.text)
        self.assertEqual(self.faces["title"]["height"], 14)

    def test_the_field_rectangle_of_the_first_key(self):
        ui = self.faces["ui"]
        content = (220, 150 + 23)
        field = (content[0] + 12, content[1] + 36)
        self.assertEqual(field, (232, 209))
        widget = rd("shell", "ui", "src", "widget.rs")
        self.assertIn("FIELD_HEIGHT: u32 = 20;", widget)
        self.assertIn("CARET_HEIGHT: u32 = 12;", widget)
        self.assertIn("TEXT_INSET: i32 = 6;", widget)
        self.assertIn("TEXT_TOP: i32 = 3;", widget)
        self.assertIn("CARET_TOP: i32 = 4;", widget)
        cell_before = (238, 212, 238, 226)  # empty
        caret_before = (238, 213, 239, 225)
        cell_after = (238, 212, 238 + advance(ui, "d"), 226)
        caret_after = (cell_after[2], 213, cell_after[2] + 1, 225)
        before, after = union(cell_before, caret_before), union(cell_after, caret_after)
        box = union(before, after)
        self.assertEqual((box, area(box)), ((238, 212, 245, 226), 98))
        self.assertIn("7 × 14 = 98", self.text)

    def test_the_slot_tables_and_the_rounding(self):
        src = rd("shell", "ui", "src", "theme.rs")
        s8 = re.search(r"const SLOTS_8: \[u8; Role::COUNT\] = \[([^\]]*)\]", src).group(1)
        s4 = re.search(r"const SLOTS_4: \[u8; Role::COUNT\] = \[([^\]]*)\]", src).group(1)
        s8, s4 = [int(x) for x in s8.split(",")], [int(x) for x in s4.split(",")]
        self.assertEqual(len(s8), 15)
        rows = re.findall(r"^\| `(\w+)` \| [^|]+\| `#[0-9a-f]{6}` \| (\d+) \| (\d+)", self.text, re.M)
        roles = re.search(r"pub enum Role \{(.*?)\}", src, re.S).group(1).split()
        roles = [r.strip(",") for r in roles]
        self.assertEqual([r[0] for r in rows], roles)
        self.assertEqual([int(r[1]) for r in rows], s8)
        self.assertEqual([int(r[2]) for r in rows], s4)
        self.assertEqual((224 * 63 + 127) // 255, 55)

    def test_the_names_the_chapter_lists_exist(self):
        font = rd("shell", "font", "src", "lib.rs")
        for n in ("parse", "glyph", "glyph_or_fallback", "advance", "Glyph", "FontError", "Font"):
            self.assertRegex(font, r"\b%s\b" % n)
        for e in ("BadMagic", "Truncated", "ZeroHeight", "BaselineOutOfCell", "OutOfOrder", "TrailingBytes"):
            self.assertIn(e, font)
        for path, names in (("theme.rs", ("Role", "Depth", "Theme")), ("palette.rs", ("DESK16", "AFTERGLOW_SURFACES", "palette")),
                            ("text.rs", ("draw_text", "fit_with_ellipsis")), ("icons.rs", ("Icon", "MINIMISE", "MAXIMISE", "CLOSE")),
                            ("widget.rs", ("Widget", "Label", "TextLine")), ("chrome.rs", ("ChromeLayout", "paint_chrome", "HALO", "TITLE_HEIGHT", "BOX"))):
            src = rd("shell", "ui", "src", path)
            for n in names:
                self.assertRegex(src, r"\b%s\b" % n)
        for crate, d in (("desk-font", "font"), ("desk-ui", "ui")):
            self.assertIn('name = "%s"' % crate, rd("shell", d, "Cargo.toml"))

    def test_each_new_term_is_defined_once_and_linked(self):
        for cid, terms in (("m-pixels", ("back-buffer", "rectangle", "damage", "compositor", "frame-digest", "presenter")),
                           ("m-text", ("glyph", "dfn", "theme", "window-chrome", "slot"))):
            raw = self.chapters[cid]["raw"]
            for t in terms:
                self.assertIn("(term:%s)" % t, raw, t)
            self.assertEqual(len(re.findall(r"^term: ", raw, re.M)), len(terms))


if __name__ == "__main__":
    unittest.main()
