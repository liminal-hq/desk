# Checks of the devices chapter (M3): its ports, lines and worked numbers against the drivers they come from.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import importlib.util
import os
import re
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
TOOL = os.path.normpath(os.path.join(HERE, ".."))
sys.path.insert(0, TOOL)

import gather  # noqa: E402
import manual  # noqa: E402


def rd(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


def explainer_module(name):
    spec = importlib.util.spec_from_file_location("t_" + name, os.path.join(manual.MANUAL_DIR, "explainers", name + ".py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class DevicesChapterTests(unittest.TestCase):
    """The devices chapter (M3): its ports, lines and worked numbers against the drivers they come from."""

    @classmethod
    def setUpClass(cls):
        cls.chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        cls.text = cls.chapters["m-devices"]["raw"]
        cls.ports = explainer_module("ports")

    def test_every_port_the_prose_names_is_one_the_code_uses(self):
        serial = rd(os.path.join(gather.ROOT, "hal", "drivers", "serial", "src", "lib.rs"))
        # the data ports one above each 8259 command port, and COM3 and COM4, which the text names and the table does not
        allowed = {v for _, _, _, _, v in self.ports.PORTS} | {0x20 + 1, 0xA0 + 1, self.ports.read_const(serial, "COM3"), self.ports.read_const(serial, "COM4")}
        named = set()
        for group in re.findall(r"ports? ((?:`0x[0-9A-Fa-f]+`(?:,? (?:and|to) |, )?)+)", self.text):
            named |= {int(h, 16) for h in re.findall(r"0x[0-9A-Fa-f]+", group)}
        self.assertTrue(named, "the chapter names no port in the form 'port `0x..`'")
        self.assertEqual(sorted(hex(n) for n in named - allowed), [])
        # and each is in the source text of the driver or kernel file the table cites, not only in the table
        for _, _, path, name, value in self.ports.PORTS:
            self.assertEqual(self.ports.read_const(rd(os.path.join(gather.ROOT, path)), name), value)

    def test_every_irq_line_the_prose_names_exists(self):
        for n in re.findall(r"IRQ (\d+)", self.text):
            self.assertLessEqual(int(n), 15)

    def test_the_vectors_and_the_pit_numbers_in_the_worked_examples_follow_from_the_constants(self):
        pic = rd(os.path.join(gather.ROOT, "hal", "drivers", "pic", "src", "lib.rs"))
        bases = self.ports.read_bases(pic)
        self.assertEqual(self.ports.vector(12, bases["REMAPPED_BASES"]), 0x2C)
        self.assertEqual(self.ports.vector(12, bases["BIOS_BASES"]), 0x74)
        self.assertIn("`0x2C`", self.text)
        self.assertIn("`0x74`", self.text)
        pit = rd(os.path.join(gather.ROOT, "hal", "drivers", "pit", "src", "lib.rs"))
        self.assertIn("pub const INPUT_HZ: u32 = 1_193_182;", pit)
        divisor = (1193182 + 100 // 2) // 100
        self.assertEqual(divisor, 11932)
        self.assertEqual(hex(divisor), "0x2e9c")
        self.assertEqual((1193182 * 1000 + divisor // 2) // divisor, 99998)
        self.assertEqual((1193182 * 1000 + 65536 // 2) // 65536, 18207)
        for needle in ("11,932", "`0x2E9C`", "99,998", "18,207"):
            self.assertIn(needle, self.text)
        self.assertIn("0x34", pit)

    def test_the_mouse_packets_decode_as_the_chapter_says(self):
        def decode(b0, bx, by):
            return bx - (256 if b0 & 0x10 else 0), -(by - (256 if b0 & 0x20 else 0)), b0 & 7
        self.assertEqual(decode(0x38, 0xFF, 0xFE), (-1, 2, 0))
        self.assertEqual(decode(0x09, 10, 4), (10, -4, 1))
        self.assertEqual(decode(0x2A, 0x03, 0xFD), (3, 3, 2))
        tests = rd(os.path.join(gather.ROOT, "hal", "drivers", "i8042", "src", "tests.rs"))
        self.assertIn("pkt(0x08 | 0x10 | 0x20, 0xFF, 0xFE, 0)", tests)
        self.assertIn("m.mouse_bytes(&[0x09, 10, 4,", tests)
        self.assertIn("`38 FF FE`", self.text)
        self.assertIn("dx = -1, dy = +2", self.text)

    def test_the_ports_explainer_refuses_a_changed_constant_and_a_renumbered_line(self):
        self.assertEqual(self.ports.check(), [])
        path = "hal/drivers/pit/src/lib.rs"
        text = rd(os.path.join(gather.ROOT, path))
        changed = text.replace("pub const CHANNEL0: u16 = 0x40;", "pub const CHANNEL0: u16 = 0x41;")
        self.assertTrue(any("CHANNEL0 as 0x41" in p for p in self.ports.check(files={path: changed})))
        missing = text.replace("CHANNEL0", "CHANNEL_ZERO")
        self.assertTrue(any("no u16 constant CHANNEL0" in p for p in self.ports.check(files={path: missing})))
        irq = rd(os.path.join(gather.ROOT, "hal", "hal", "src", "irq.rs"))
        self.assertTrue(any("numbers MOUSE as 11" in p for p in self.ports.check(irq_text=irq.replace("MOUSE: Irq = Irq(12)", "MOUSE: Irq = Irq(11)"))))
        pic = rd(os.path.join(gather.ROOT, "hal", "drivers", "pic", "src", "lib.rs"))
        self.assertTrue(any("multiples of eight" in p for p in self.ports.check(pic_text=pic.replace("(0x20, 0x28)", "(0x21, 0x28)"))))
        self.assertTrue(any("just after the master" in p for p in self.ports.check(pic_text=pic.replace("(0x20, 0x28)", "(0x20, 0x20)"))))
        self.assertTrue(any("but the chapter says" in p for p in self.ports.check(pic_text=pic.replace("(0x20, 0x28)", "(0x30, 0x38)"))))
