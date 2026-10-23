# Checks of the memory chapter (M7) and the BIOS chapter (M8): the arithmetic they quote, against the transcript and the code.
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


def frames_free(entries, kernel, first_mib=0x100000, frame=4096, max_bytes=256 * 1024 * 1024):
    """A re-implementation of memory::init on (base, end, usable) entries: returns the free-frame count after the four passes."""
    limit = max_bytes // frame
    used = [True] * limit
    for base, end, usable in entries:
        if usable:
            for f in range(-(-base // frame), min(end // frame, limit)):
                used[f] = False
    for base, end, usable in entries:
        if not usable and end != base:
            for f in range(base // frame, min(-(-end // frame), limit)):
                used[f] = True
    for f in range(first_mib // frame):
        used[f] = True
    for f in range(kernel[0] // frame, -(-kernel[1] // frame)):
        used[f] = True
    return used.count(False), used


class MemoryChapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        cls.text = cls.chapters["m-memory"]["raw"]
        cls.serial = rd("tools", "progress-report", "media", "native-serial.txt")
        cls.memory = rd("kernel", "src", "memory.rs")
        cls.heap = rd("kernel", "src", "heap.rs")
        cls.fpu = rd("kernel", "src", "fpu.rs")
        cls.entries = [(int(a, 16), int(b, 16), kind == "usable") for a, b, kind in re.findall(r"^  map (0x[0-9a-f]+)-(0x[0-9a-f]+) (\w+)", cls.serial, re.M)]

    def test_the_constants_the_chapter_quotes_are_the_code_s(self):
        self.assertIn("pub const FRAME_SIZE: u32 = 4096;", self.memory)
        self.assertIn("const MAX_BYTES: u64 = 256 * 1024 * 1024;", self.memory)
        self.assertIn("const HEAP_FRAMES: usize = 1024; // 4 MiB", self.heap)
        self.assertIn("const GRAIN: usize = 8;", self.heap)
        self.assertEqual(256 * 1024 * 1024 // 4096, 65536)
        self.assertEqual(65536 // 32 * 4, 8192)
        for needle in ("`MAX_FRAMES` is 65,536", "8,192 bytes", "1,024 contiguous frames"):
            self.assertIn(needle, self.text)

    def test_the_usable_ram_line_is_the_sum_of_the_usable_entries_in_kib(self):
        self.assertEqual(len(self.entries), 6)
        usable = [(b, e) for b, e, u in self.entries if u]
        self.assertEqual([(e - b) // 1024 for b, e in usable], [639, 15232])
        self.assertEqual(sum((e - b) // 1024 for b, e in usable), 15871)
        self.assertIn("memory: usable RAM 15871 KiB", self.serial)
        self.assertIn("len / 1024", self.memory)
        self.assertIn("639 + 15,232 = 15,871 KiB", self.text)

    def test_the_frame_count_follows_from_the_map_and_the_kernel_range(self):
        kernel = tuple(int(x, 16) for x in re.search(r"memory: kernel image (0x[0-9a-f]+)-(0x[0-9a-f]+)", self.serial).groups())
        self.assertEqual(kernel, (0x100000, 0x13C000))
        free, used = frames_free(self.entries, kernel)
        printed = int(re.search(r"memory: (\d+) frames of 4 KiB free", self.serial).group(1))
        self.assertEqual(free, printed)
        self.assertEqual(printed, 3748)
        # the steps of the worked example
        pass1 = 0x9FC00 // 4096 + (0xFE0000 - 0x100000) // 4096
        self.assertEqual((0x9FC00 // 4096, (0xFE0000 - 0x100000) // 4096, pass1), (159, 3808, 3967))
        self.assertEqual(pass1 * 4, 15868)
        self.assertEqual(15871 - 15868, 3)
        self.assertEqual(0x9FC00 % 4096, 0xC00)
        self.assertEqual(pass1 - 159 - (0x13C000 - 0x100000) // 4096, 3748)
        self.assertEqual((0x13C000 - 0x100000) // 4096, 60)
        self.assertEqual(4096 - 256 - 60 - 32, 3748)
        for needle in ("**3,748**", "3,967", "15,868 KiB", "245,760 bytes"):
            self.assertIn(needle, self.text)

    def test_the_reservations_change_nothing_but_the_first_megabyte_and_the_kernel(self):
        usable_only = [e for e in self.entries if e[2]]
        kernel = (0x100000, 0x13C000)
        self.assertEqual(frames_free(self.entries, kernel)[0], frames_free(usable_only, kernel)[0])
        # an overlap resolves to reserved: a reserved entry laid over a usable one removes its frames
        over = self.entries + [(0x200000, 0x300000, False)]
        self.assertEqual(frames_free(over, kernel)[0], 3748 - 256)

    def test_the_heap_lands_after_the_kernel_and_the_heap_figures_follow(self):
        free, used = frames_free(self.entries, (0x100000, 0x13C000))
        first = used.index(False)
        self.assertEqual(first, 316)
        self.assertEqual(first * 4096, 0x13C000)
        self.assertEqual(all(not used[f] for f in range(first, first + 1024)), True)
        self.assertEqual((first + 1024) * 4096, 0x53C000)
        self.assertEqual(3748 - 1024, 2724)
        self.assertIn("heap: 4096 KiB at 0x13c000", self.serial)
        self.assertEqual(4 * 1024 * 1024 - 40, 4194264)
        self.assertEqual(0x13C000 + 40, 0x13C028)
        self.assertEqual(sum(x * x for x in range(1, 11)), 385)
        self.assertEqual(hex(385), "0x181")

    def test_the_x87_numbers_are_the_code_s_and_the_hardware_s(self):
        self.assertIn("0x037F", self.fpu)
        self.assertEqual(0x037F, 0b0000_0011_0111_1111)
        self.assertEqual(0x037F & 0x3F, 0x3F)
        self.assertEqual((0x037F >> 8) & 3, 3)
        self.assertEqual((0x037F >> 10) & 3, 0)
        self.assertIn("clear = const !((1u32 << 2) | (1 << 3))", self.fpu)
        self.assertIn("set = const (1u32 << 1) | (1 << 5)", self.fpu)
        h = sum(1 / i for i in range(1, 11))
        self.assertAlmostEqual(h, 2.928968253968254, places=12)
        self.assertEqual("%.6f" % h, "2.928968")
        self.assertEqual("%.6f" % (2 ** 0.5), "1.414214")
        self.assertIn("fpu: control word 0x037f, sqrt(2) = 1.414214, H(10) = 2.928968", self.serial)

    def test_the_kernel_end_is_page_aligned_and_includes_bss_and_the_stack(self):
        ld = rd("kernel", "link.ld")
        self.assertIn("__kernel_end = ALIGN(4096);", ld)
        main = rd("kernel", "src", "main.rs")
        self.assertIn(".skip 65536", main)


class BiosChapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        cls.text = cls.chapters["m-bioscalls"]["raw"]
        cls.doc = rd("docs", "N3-BIOS-PATH.md")
        cls.rs = rd("kernel", "src", "bios.rs")
        cls.asm = rd("kernel", "bios16.asm")
        cls.expected = rd("tests", "native", "expected-bios.txt")

    def test_the_measured_cost_is_quoted_as_the_document_records_it(self):
        for row in ("| QEMU 11.1.2, TCG | about 47 µs (24 ticks) |", "| QEMU 8.2.2, TCG | about 32 µs (16 ticks) |", "| Bochs 2.7 at its default 4 MIPS | about 160 µs (81 ticks) |"):
            self.assertIn(row, self.doc)
            self.assertIn(row, self.text)
        self.assertIn("One `INT 1Ah` AH=02h call, 5,000 in a row, timed in 10 ms kernel ticks", self.doc)
        self.assertIn("One `INT 1Ah` AH=02h call, 5,000 in a row, timed in 10 ms kernel ticks", self.text)
        for words in ("a rough guess of 100 µs for a trivial call and ten to a few hundred milliseconds for a mode set or a floppy sector", "the right order, unmeasured", "emulated speeds and say nothing about a 486"):
            self.assertIn(words, self.doc)
        for words in ("a rough guess of 100 µs for a trivial call and ten to a few hundred milliseconds for a mode set or a floppy sector", "the right order, unmeasured", "emulated speeds and say nothing about a 486"):
            self.assertIn(words, self.text)

    def test_the_cost_in_ticks_matches_the_microseconds(self):
        for ticks, micro in ((24, 47), (16, 32), (81, 160)):
            self.assertLessEqual(abs(ticks * 10_000 / 5000 - micro), 2)
        self.assertIn("COST_CALLS: u32 = 5000", rd("kernel", "src", "biostest.rs"))

    def test_the_layout_constants_match_the_code_and_the_assembly(self):
        self.assertIn("pub const LOW_BASE: u32 = 0x1_0000;", self.rs)
        self.assertIn("pub const BOUNCE_BASE: u32 = 0x1_1000;", self.rs)
        self.assertIn("pub const BOUNCE_LEN: usize = 8192;", self.rs)
        self.assertIn("const STACK_BASE: u32 = 0x1_3000;", self.rs)
        self.assertIn("BASE            equ 0x10000", self.asm)
        self.assertIn("STACK_TOP       equ 0x4000", self.asm)
        self.assertIn("bios: real-mode trampoline at 0x10000, bounce buffer 0x11000 (8192 bytes), stack 0x13000", self.expected)
        self.assertEqual(0x10000 >> 4, 0x1000)
        self.assertEqual(0x11000 >> 4, 0x1100)

    def test_the_sixteen_bit_descriptors_decode_as_the_chapter_says(self):
        def decode(q):
            b = q.to_bytes(8, "little")
            return b, b[2] | b[3] << 8 | b[4] << 16 | b[7] << 24, b[0] | b[1] << 8 | (b[6] & 15) << 16, b[5], b[6] >> 4
        b, base, limit, access, flags = decode(0x00009A010000FFFF)
        self.assertEqual(b.hex(" ").upper(), "FF FF 00 00 01 9A 00 00")
        self.assertEqual((base, limit, access, flags), (0x10000, 0xFFFF, 0x9A, 0))
        self.assertEqual(decode(0x000092010000FFFF)[1:4], (0x10000, 0xFFFF, 0x92))
        self.assertIn("dw 0xFFFF, BASE & 0xFFFF", self.asm)
        self.assertIn("db (BASE >> 16) & 0xFF, 0x9A, 0x00, BASE >> 24", self.asm)
        self.assertIn("db (BASE >> 16) & 0xFF, 0x92, 0x00, BASE >> 24", self.asm)

    def test_the_pic_mask_and_the_entry_flags_in_the_worked_example(self):
        self.assertIn("const BIOS_PIC_MASK: u16 = !(1 << 0 | 1 << 2 | 1 << 6 | 1 << 8 | 1 << 14 | 1 << 15);", self.rs)
        open_bits = (1 << 0) | (1 << 2) | (1 << 6) | (1 << 8) | (1 << 14) | (1 << 15)
        self.assertEqual(open_bits, 0xC145)
        mask = ~open_bits & 0xFFFF
        self.assertEqual(mask, 0x3EBA)
        self.assertEqual((mask & 0xFF, mask >> 8), (0xBA, 0x3E))
        self.assertEqual([i for i in range(8) if not mask >> i & 1], [0, 2, 6])
        self.assertEqual([8 + i for i in range(8) if not (mask >> 8) >> i & 1], [8, 14, 15])
        self.assertIn("(regs.flags & 0x0CD5) | 0x0002", self.rs)
        self.assertEqual(0x0CD5, 0x1 | 0x4 | 0x10 | 0x40 | 0x80 | 0x400 | 0x800)
        self.assertEqual(0x1A * 4, 0x68)
        self.assertIn("pub const BIOS_BASES: (u8, u8) = (0x08, 0x70);", rd("hal", "drivers", "pic", "src", "lib.rs"))

    def test_the_e820_and_wait_parameters_are_the_scenarios(self):
        t = rd("kernel", "src", "biostest.rs")
        self.assertIn("bounce.write_low(20, &1u32.to_le_bytes())", t)
        self.assertIn("edx: 0x534D_4150", t)
        self.assertEqual(0x534D4150.to_bytes(4, "big"), b"SMAP")
        self.assertIn("eax: 0x8600, ecx: 0x000F, edx: 0x4240", t)
        self.assertEqual(0x000F4240, 1_000_000)
        self.assertEqual(0x0007A120, 500_000)

    def test_the_transcript_has_the_lines_the_chapter_quotes(self):
        for line in ("bios: INT 15h AX=E820 after boot: {N:1..32} entries, the loader's map had {N:1..32}, identical: true",
                     "bios: stress: the tick count kept one hundred ticks to the RTC second: true",
                     "bios: VBE set, draw and return to text worked twice"):
            self.assertIn(line, self.expected)
        sums = rd("tests", "native", "bios.sums")
        for name in ("vbe-640 640x480 84cd5f8b", "vbe-800 800x600 2c094efc", "text 720x400 3eeee6dd"):
            self.assertIn(name, sums)

    def test_the_status_table_does_not_overclaim_what_the_document_says_was_not_run(self):
        states = {s["id"]: s["state"] for s in self.chapters["m-bioscalls"]["status"]}
        for key in ("bios.nmi", "bios.others"):
            self.assertEqual(states[key], "built-unverified")
        for key in ("bios.hang", "bios.paging", "bios.v86", "bios.hardware"):
            self.assertEqual(states[key], "planned")
        self.assertIn("Nothing ran on real hardware.", self.doc)


if __name__ == "__main__":
    unittest.main()
