# Checks of the interrupts chapter (M6): its vectors, divisor and tick arithmetic and negative-check files against the code they come from.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
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


class InterruptsChapterTests(unittest.TestCase):
    """The interrupts chapter (M6): numbers in the prose follow from the constants, files named hold the lines quoted."""

    @classmethod
    def setUpClass(cls):
        cls.chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        cls.text = cls.chapters["m-interrupts"]["raw"]
        cls.idt = rd("kernel", "src", "idt.rs")
        cls.pic = rd("hal", "drivers", "pic", "src", "lib.rs")
        cls.pit = rd("hal", "drivers", "pit", "src", "lib.rs")
        cls.main = rd("kernel", "src", "main.rs")

    def test_the_remapped_bases_and_the_vectors_the_prose_names(self):
        self.assertIn("pub const REMAPPED_BASES: (u8, u8) = (0x20, 0x28);", self.pic)
        self.assertIn("pub const BIOS_BASES: (u8, u8) = (0x08, 0x70);", self.pic)
        self.assertIn("pub const IRQ_BASE: u8 = 0x20;", self.idt)
        self.assertIn("PIC.init(REMAPPED_BASES, 0xFFFE)", rd("kernel", "src", "pic.rs"))
        self.assertEqual(0xFFFE & 0xFF, 0xFE)
        self.assertEqual(0xFFFE >> 8, 0xFF)
        self.assertEqual(0x28 + (12 - 8), 0x2C)
        self.assertEqual((0x20 + 7, 0x28 + 7), (0x27, 0x2F))
        self.assertEqual((0x08 + 5), 0x0D)
        for needle in ("0x2C", "`0x27`", "`0x2F`", "`0x0D`", "`0x08`"):
            self.assertIn(needle, self.text)

    def test_the_error_code_mask_lists_the_vectors_the_prose_names(self):
        self.assertIn("& 0x60227D00) == 0", self.idt)
        bits = [n for n in range(32) if (0x60227D00 >> n) & 1]
        self.assertEqual(bits, [8, 10, 11, 12, 13, 14, 17, 21, 29, 30])
        self.assertIn("8, 10 to 14, 17, 21, 29 and 30", self.text)
        self.assertNotIn(0, bits)
        self.assertNotIn(6, bits)

    def test_the_table_sizes_and_limits(self):
        self.assertIn("[u64; 3]", rd("kernel", "src", "gdt.rs"))
        self.assertEqual(3 * 8 - 1, 0x17)
        self.assertIn("[Gate; 256]", self.idt)
        self.assertEqual(256 * 8 - 1, 2047)
        self.assertEqual(0x7FF, 2047)
        self.assertIn("0x8E", self.idt)
        for needle in ("`0x17`", "2,047", "`0x7FF`", "flags `0x8E`"):
            self.assertIn(needle, self.text)

    def test_the_divisor_and_the_tick_arithmetic_follow_from_the_constants(self):
        self.assertIn("pub const INPUT_HZ: u32 = 1_193_182;", self.pit)
        self.assertIn("pub const HZ: u32 = 100;", rd("kernel", "src", "pit.rs"))
        self.assertIn("const TICK_WAIT: u32 = 10;", self.main)
        self.assertIn("spins > 200_000_000", self.main)
        divisor = (1193182 + 100 // 2) // 100
        self.assertEqual(divisor, 11932)
        self.assertEqual(hex(divisor), "0x2e9c")
        self.assertEqual((1193182 * 1000 + divisor // 2) // divisor, 99998)
        clocks = 10 * divisor
        self.assertEqual(clocks, 119320)
        self.assertEqual(round(clocks / 1193182, 6), 0.100002)
        for needle in ("11,932", "`0x2E9C`", "99,998", "119,320", "0.100002", "200,000,000"):
            self.assertIn(needle, self.text)
        self.assertIn("timer: waited for 10 ticks with hlt, elapsed 10 ticks", rd("tests", "native", "expected.txt"))

    def test_each_negative_check_file_holds_the_lines_the_prose_quotes(self):
        want = {
            "div0": ["EXCEPTION 0x00: divide error", "error code 0x00000000", "ECX 0x00000000  EDX 0x00000000"],
            "gp": ["EXCEPTION 0x0d: general protection fault", "error code 0x00000028", "EAX 0x00000028"],
            "ud": ["EXCEPTION 0x06: invalid opcode", "error code 0x00000000"],
            "notick": ["test: timer masked on purpose", "the timer did not tick"],
        }
        for name, lines in want.items():
            body = rd("tests", "native", "expected-%s.txt" % name)
            self.assertIn("expected-%s.txt" % name, self.text)
            for line in lines:
                self.assertIn(line, body)
                self.assertIn(line, self.text)

    def test_the_selector_in_the_gp_case_decodes_as_the_prose_says(self):
        sel = 0x28
        self.assertEqual((sel & 3, (sel >> 2) & 1, sel >> 3), (0, 0, 5))
        self.assertGreater(sel, 0x17)

    def test_the_spurious_lines_and_the_eoi_are_where_the_prose_puts_them(self):
        self.assertIn("7 => self.in_service(MASTER) & 0x80 == 0", self.pic)
        self.assertIn("15 => self.in_service(SLAVE) & 0x80 == 0", self.pic)
        self.assertIn("OCW3_READ_ISR: u8 = 0x0B", self.pic)
        hal = rd("kernel", "src", "hal.rs")
        self.assertIn("PIC.eoi(irq);", hal)
        self.assertLess(hal.index("h.call()", hal.index("fn irq_dispatch")), hal.index("PIC.eoi(irq);"))

    def test_the_sidebars_have_a_source_and_at_most_five_sentences(self):
        blocks = re.findall(r"(?m)^> \*\*From the stacks.*(?:\n>.*)*", self.text)
        self.assertEqual(len(blocks), 2)
        for b in blocks:
            lines = [l[2:] for l in b.splitlines()]
            self.assertTrue(lines[-1].startswith("Source: ["))
            prose = " ".join(lines[:-1])
            self.assertLessEqual(len(re.findall(r"[.!?](?:\s|$)", prose)), 5)

    def test_every_new_term_is_used_with_a_link(self):
        for t in ("idt", "interrupt-vector", "exception", "interrupt-gate", "spurious-interrupt", "tick"):
            self.assertIn("(term:%s)" % t, self.text)


if __name__ == "__main__":
    unittest.main()
