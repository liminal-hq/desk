# Checks of the input and graphics chapters (M11 and M12): the bytes, lines and counts they quote against the tests and transcripts they come from.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import json
import os
import re
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
TOOL = os.path.normpath(os.path.join(HERE, ".."))
sys.path.insert(0, TOOL)

import gather  # noqa: E402
import manual  # noqa: E402


def rd(*parts):
    with open(os.path.join(gather.ROOT, *parts), encoding="utf-8") as f:
        return f.read()


# A replica of just enough of desk_kbd::Decoder (scancode set 1) to decode the byte runs the chapter quotes.
PLAIN = {0x1D: "LeftCtrl", 0x1E: "A", 0x2A: "LeftShift", 0x38: "LeftAlt"}
EXT = {0x48: "Up", 0x53: "Delete"}
MODS = {"LeftShift": 1, "LeftCtrl": 2, "LeftAlt": 4}


def decode_keys(data):
    held, out, ext = set(), [], False
    for b in data:
        if b == 0xE0:
            ext = True
            continue
        name = (EXT if ext else PLAIN)[b & 0x7F]
        ext = False
        pressed = b & 0x80 == 0
        if name in MODS:
            (held.add if pressed else held.discard)(name)
        mods = sum(MODS[h] for h in held)
        out.append((name, pressed, mods))
    return out


def native_line(event):
    name, pressed, mods = event
    names = [n for bit, n in ((1, "shift"), (2, "ctrl"), (4, "alt")) if mods & bit]
    return "key: %s %s mods=%s" % (name, "down" if pressed else "up", "+".join(names) or "-")


def decode_packet(b0, bx, by):
    return bx - (256 if b0 & 0x10 else 0), -(by - (256 if b0 & 0x20 else 0)), b0 & 7


def parse_mode(source, name):
    """The ModeRegs constant `name` of regs.rs as a dict of integer lists."""
    body = re.search(r"pub const %s: ModeRegs = ModeRegs \{(.*?)\n\};" % name, source, re.S).group(1)
    out = {"misc": int(re.search(r"misc: (0x[0-9A-Fa-f]+)", body).group(1), 16)}
    for field in ("seq", "crtc", "gc", "attr"):
        m = re.search(r"%s: \[(.*?)\]," % field, body, re.S)
        out[field] = [int(v, 16) for v in re.findall(r"0x[0-9A-Fa-f]+", m.group(1))]
    return out


def mode_writes(m):
    """The (port, value) writes of set_mode_registers, in order, as the function's comment and body give them."""
    w = [(0x3C0, 0x00)]
    w += [(0x3C4, 0), (0x3C5, 1), (0x3C2, m["misc"])]
    for i in range(1, 5):
        w += [(0x3C4, i), (0x3C5, m["seq"][i])]
    w += [(0x3C4, 0), (0x3C5, m["seq"][0])]
    w += [(0x3D4, 0x11), (0x3D5, m["crtc"][0x11] & 0x7F)]
    for i, v in enumerate(m["crtc"]):
        w += [(0x3D4, i), (0x3D5, v)]
    for i, v in enumerate(m["gc"]):
        w += [(0x3CE, i), (0x3CF, v)]
    for i, v in enumerate(m["attr"]):
        w += [(0x3C0, i), (0x3C0, v)]
    w += [(0x3C0, 0x20), (0x3C6, 0xFF)]
    return w


class InputGraphicsChapterTests(unittest.TestCase):
    """M11 and M12 against the tests, transcripts and tables they quote."""

    @classmethod
    def setUpClass(cls):
        cls.chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        cls.input = cls.chapters["m-input"]
        cls.gfx = cls.chapters["m-graphics"]

    def test_the_worked_key_bytes_decode_to_the_transcript_lines_the_chapter_quotes(self):
        tests = rd("hal", "drivers", "i8042", "src", "tests.rs")
        self.assertIn("m.key_bytes(&[0x2A, 0x1E, 0x9E, 0xAA, 0xE0, 0x48, 0xE0, 0xC8]);", tests)
        events = decode_keys([0x2A, 0x1E, 0x9E, 0xAA, 0xE0, 0x48, 0xE0, 0xC8])
        self.assertEqual([e[0] for e in events], ["LeftShift", "A", "A", "LeftShift", "Up", "Up"])
        expected = rd("tests", "native", "expected-input.txt").splitlines()
        shown = [native_line(e) for e in events[:4]]
        start = next(i for i, l in enumerate(expected) if l == shown[0] and expected[i + 1].startswith("key: A down mods=shift"))
        self.assertEqual(expected[start:start + 2][0], shown[0])
        self.assertEqual([l.split(" char=")[0] for l in expected[start:start + 4]], shown)
        for line in ("key: LeftShift down mods=shift", "key: A down mods=shift char='A'", "key: A up mods=shift", "key: LeftShift up mods=-"):
            self.assertIn(line, self.input["raw"])
        self.assertIn("`2A 1E 9E AA`", self.input["raw"])

    def test_ctrl_alt_delete_bytes_decode_to_the_six_expected_lines(self):
        kbd = rd("hal", "drivers", "kbd", "src", "lib.rs")
        self.assertIn("&[0x1D, 0x38, 0xE0, 0x53, 0xE0, 0xD3, 0xB8, 0x9D]", kbd)
        events = decode_keys([0x1D, 0x38, 0xE0, 0x53, 0xE0, 0xD3, 0xB8, 0x9D])
        expected = rd("tests", "native", "expected-input.txt").splitlines()
        i = expected.index("key: LeftCtrl down mods=ctrl")
        self.assertEqual([l.split(" char=")[0] for l in expected[i:i + 6]], [native_line(e) for e in events])
        self.assertIn("`1D 38 E0 53 E0 D3 B8 9D`", self.input["raw"])

    def test_the_mouse_packets_in_the_worked_example_decode_as_stated(self):
        tests = rd("hal", "drivers", "i8042", "src", "tests.rs")
        self.assertIn("m.mouse_bytes(&[0x09, 10, 4, 0x3A, 0xFF, 0xFE, 0x08, 0, 0]);", tests)
        self.assertEqual(decode_packet(0x09, 10, 4), (10, -4, 1))
        self.assertEqual(decode_packet(0x3A, 0xFF, 0xFE), (-1, 2, 2))
        self.assertEqual(decode_packet(0x08, 0, 0), (0, 0, 0))
        self.assertIn("dx −1, dy +2, buttons 2", self.input["raw"])
        expected = rd("tests", "native", "expected-input.txt")
        for line in ("mouse: dx=10 dy=-5 buttons=0 wheel=0", "mouse: dx=0 dy=0 buttons=1 wheel=0"):
            self.assertIn(line, expected)
        self.assertIn("`mouse: dx=10 dy=-5 buttons=0 wheel=0`", self.input["raw"])

    def test_the_counts_the_chapter_quotes_are_the_trees(self):
        def tests_in(*parts):
            return len(re.findall(r"#\[test\]", rd(*parts)))
        self.assertEqual(tests_in("hal", "drivers", "i8042", "src", "tests.rs"), 47)
        self.assertEqual(tests_in("hal", "drivers", "kbd", "src", "lib.rs"), 24)
        self.assertEqual(tests_in("hal", "drivers", "int33", "src", "tests.rs"), 16)
        self.assertIn("47 host tests", self.input["raw"])
        expected = rd("tests", "native", "expected-input.txt")
        self.assertIn("input: escape released after 26 key events and 8 mouse events", expected)
        self.assertEqual(len([l for l in expected.splitlines() if l.startswith("key: ")]), 26)
        self.assertEqual(len([l for l in expected.splitlines() if l.startswith("mouse: ")]), 8)

    def test_the_dos_mouse_path_is_tagged_no_higher_than_the_anatomy_box(self):
        with open(os.path.join(TOOL, "anatomy-dos.json"), encoding="utf-8") as f:
            boxes = {b["id"]: b for b in json.load(f)["boxes"]}
        self.assertEqual(boxes["mouse"]["status"], "unverified")
        states = {s["id"]: s["state"] for s in self.input["status"]}
        self.assertEqual(states["input.dos.mouse"], "built-unverified")
        self.assertEqual(states["input.live"], "built-unverified")
        for sid, state in states.items():
            if "mouse" in sid and "dos" in sid:
                self.assertNotEqual(state, "built-verified", sid)
        self.assertEqual(boxes["kbd"]["status"], "verified")

    def test_the_host_only_claims_are_not_marked_verified(self):
        states = {s["id"]: s["state"] for s in self.input["status"]}
        self.assertEqual(states["input.resync"], "built-unverified")
        self.assertEqual(states["input.hardware"], "planned")
        gstates = {s["id"]: s["state"] for s in self.gfx["status"]}
        for sid in ("gfx.retrace", "gfx.overlap", "gfx.lfb.browser"):
            self.assertEqual(gstates[sid], "built-unverified", sid)
        self.assertEqual(gstates["gfx.hardware"], "planned")

    def test_mode_13h_is_128_writes_and_the_first_and_last_are_the_ones_quoted(self):
        regs = rd("hal", "drivers", "vga", "src", "regs.rs")
        m13 = parse_mode(regs, "MODE_13H")
        self.assertEqual((len(m13["seq"]), len(m13["crtc"]), len(m13["gc"]), len(m13["attr"])), (5, 25, 9, 21))
        w = mode_writes(m13)
        self.assertEqual(len(w), 128)
        self.assertEqual(1 + 13 + 52 + 18 + 43 + 1, 128)
        self.assertEqual(w[:8], [(0x3C0, 0), (0x3C4, 0), (0x3C5, 1), (0x3C2, 0x63), (0x3C4, 1), (0x3C5, 1), (0x3C4, 2), (0x3C5, 0x0F)])
        self.assertEqual(w[-4:], [(0x3C0, 0x14), (0x3C0, 0x00), (0x3C0, 0x20), (0x3C6, 0xFF)])
        self.assertEqual(m13["crtc"][0x11], 0x8E)
        self.assertEqual(m13["crtc"][0x11] & 0x7F, 0x0E)
        self.assertEqual((m13["seq"][4], m13["gc"][5], m13["crtc"][9] & 0x1F), (0x0E, 0x40, 1))
        # the vertical display end: register 0x12 plus bit 8 from register 7 is line 399
        vde = m13["crtc"][0x12] | ((m13["crtc"][7] >> 1 & 1) << 8) | ((m13["crtc"][7] >> 6 & 1) << 9)
        self.assertEqual(vde, 399)
        m12 = parse_mode(regs, "MODE_12H")
        self.assertEqual((m12["misc"], m12["seq"][4], m12["attr"][:16]), (0xE3, 0x06, list(range(16))))
        # the same numbers, from the test the chapter says asserts them
        test = rd("hal", "drivers", "vga", "tests", "registers.rs")
        self.assertIn("assert_eq!(w.len(), 128);", test)
        self.assertIn("(0x3C0, 0x14), (0x3C0, 0x00), (0x3C0, 0x20), (0x3C6, 0xFF)", test)
        for needle in ("128", "`3C0 ← 14`", "`3C6 ← FF`", "`3D5 ← 0E`"):
            self.assertIn(needle, self.gfx["raw"])

    def test_the_planar_pixel_example_and_the_mask_arithmetic(self):
        test = rd("hal", "drivers", "vga", "tests", "planar.rs")
        self.assertIn("c.pixel(10, 2, 0x0A);", test)
        self.assertIn("[(0x3CE, 0), (0x3CF, 0x0A), (0x3CE, 8), (0x3CF, 0x20)]", test)
        self.assertEqual(0x80 >> (10 % 8), 0x20)
        self.assertEqual(2 * 80 + 10 // 8, 161)
        self.assertEqual([p for p in range(4) if 0x0A >> p & 1], [1, 3])
        self.assertIn("`2 × 80 + 10 / 8 = 161`", self.gfx["raw"])

    def test_the_vbe_attribute_word_decodes_as_the_chapter_says(self):
        listing = rd("tools", "progress-report", "media", "jsdos-vbe.txt")
        line = re.search(r"^mode 0x101: .*$", listing, re.M).group(0)
        self.assertIn("attrs 0x009b lfb true pitch 640 phys 0xc0000000", line)
        attrs = 0x9B
        bits = [b for b in range(8) if attrs >> b & 1]
        self.assertEqual(bits, [0, 1, 3, 4, 7])
        vbe = rd("hal", "drivers", "vga", "src", "vbe.rs")
        for const, bit in (("ATTR_SUPPORTED", 0), ("ATTR_GRAPHICS", 4), ("ATTR_NO_BANKED", 6), ("ATTR_LINEAR", 7)):
            self.assertIn("pub const %s: u16 = 1 << %d;" % (const, bit), vbe)
        self.assertIn("mode 0x102: 800x600x4 model 3 attrs 0x001b lfb false pitch 100", listing)
        nolfb = rd("tools", "progress-report", "media", "jsdos-vbe-nolfb.txt")
        self.assertIn("mode 0x101: 640x480x8 model 4 attrs 0x001b lfb false pitch 640 phys 0x00000000", nolfb)
        self.assertEqual(640 * 480, 0x4B000)
        self.assertEqual(0xFD04B000 - 0xFD000000, 640 * 480)
        self.assertIn("0xfd000000-0xfd04b000", self.gfx["raw"])

    def test_the_fade_example_numbers(self):
        # Rgb6::from_rgb8(0xff, 0xaa, 0x40) and scaled(8, 16), as the HAL's own tests assert them
        self.assertEqual(tuple((v * 63 + 127) // 255 for v in (0xFF, 0xAA, 0x40)), (63, 42, 16))
        video = rd("hal", "hal", "src", "video.rs")
        self.assertIn("Rgb6::from_rgb8(0xff, 0xaa, 0x40), Rgb6::new(63, 42, 16)", video)
        self.assertIn("63, 42, 16", self.gfx["raw"])

    def test_every_qemu_hides_statement_cites_a_document(self):
        text = self.gfx["raw"]
        section = text[text.index("## What QEMU hides"):text.index("## The code")]
        bullets = [b for b in section.split("\n- ")[1:]]
        self.assertGreaterEqual(len(bullets), 6)
        for b in bullets:
            self.assertRegex(b, r"`docs/[A-Za-z0-9_-]+\.md`", b[:60])
        for name in re.findall(r"`(docs/[A-Za-z0-9_-]+\.md)`", section):
            self.assertTrue(os.path.exists(os.path.join(gather.ROOT, name)), name)

    def test_the_checksums_the_chapter_quotes_are_in_the_kernels_constants_and_transcripts(self):
        gfx = rd("kernel", "src", "gfx.rs")
        for const, value in (("HOSTED_13H", "0x5c0f_1f16"), ("HOSTED_12H", "0xfaf8_b758"), ("HOSTED_VBE_640X480", "0x5bc6_5936")):
            self.assertRegex(gfx, r"%s: u32 = %s;" % (const, value))
        expected = rd("tests", "native", "expected-gfx-vbe.txt")
        for value in ("0x5bc65936", "0x5c0f1f16"):
            self.assertIn(value, expected)
            self.assertIn(value, self.gfx["raw"])
        self.assertIn("0xfaf8b758", rd("tests", "native", "expected-gfx-fallback.txt"))
        self.assertIn("the driver's retrace-synchronised fade took {N:0..400} ticks", expected)
        self.assertIn("took 0 ticks for 16 + 16 frames", self.gfx["raw"])


if __name__ == "__main__":
    unittest.main()
