# Checks of the HAL chapter (M9) and the system drivers chapter (M10): the sequences and numbers they quote, against the sources.
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


class HalChaptersTests(unittest.TestCase):
    """M9 and M10: every name, register value and sequence the prose quotes is in the driver source or its test."""

    @classmethod
    def setUpClass(cls):
        cls.chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        cls.hal = cls.chapters["m-hal"]["raw"]
        cls.drv = cls.chapters["m-sysdrivers"]["raw"]
        cls.serial = rd("hal", "drivers", "serial", "src", "lib.rs")
        cls.pic = rd("hal", "drivers", "pic", "src", "lib.rs")
        cls.pit = rd("hal", "drivers", "pit", "src", "lib.rs")

    def test_the_hal_names_exist_in_the_contract_crate(self):
        src = "".join(rd("hal", "hal", "src", f) for f in sorted(os.listdir(os.path.join(gather.ROOT, "hal", "hal", "src"))))
        for name in ("PortIo", "InterruptController", "IrqRegistry", "IrqHandler", "IrqService", "SharedIrqService", "Clock",
                     "Delay", "DebugOut", "SpscRing", "MmioRegion", "HalError", "RawPorts", "TickCounter", "Display",
                     "InputSource", "Bios", "Chain", "Irq", "Ticks", "Rgb6"):
            self.assertRegex(src, r"\b(trait|struct|enum) " + name + r"\b", name)
        for case in ("Unsupported", "NoDevice", "Timeout", "InvalidArgument", "Busy", "OutOfMemory", "Platform"):
            self.assertIn(case, src)
        self.assertEqual(len(re.findall(r"^\s+(?:Unsupported|NoDevice|Timeout|InvalidArgument|Busy|OutOfMemory|Platform\(u16\)),$",
                                        rd("hal", "hal", "src", "error.rs"), re.M)), 7)

    def test_the_clippy_ban_lists_ten_methods(self):
        banned = re.findall(r'path = "core::sync::atomic::Atomic::(\w+)"', rd("hal", "clippy.toml"))
        self.assertEqual(len(banned), 10)
        for m in banned:
            self.assertIn("`%s`" % m, self.hal)

    def test_the_hal_has_one_feature_and_no_dependencies(self):
        toml = rd("hal", "hal", "Cargo.toml")
        self.assertIn("std = []", toml)
        self.assertNotIn("[dependencies]", toml)
        self.assertIn('#![cfg_attr(not(any(test, feature = "std")), no_std)]', rd("hal", "hal", "src", "lib.rs"))

    def test_the_drivers_contain_no_unsafe(self):
        for src in (self.serial, self.pic, self.pit):
            self.assertNotIn("unsafe", src)

    def test_the_migration_state_matches_the_tree(self):
        kernel = rd("kernel", "Cargo.toml")
        for crate in ("desk-hal", "desk-serial", "desk-pit", "desk-pic"):
            self.assertIn(crate, kernel)
        self.assertFalse(os.path.exists(os.path.join(gather.ROOT, "kernel", "src", "port.rs")))
        self.assertFalse(os.path.exists(os.path.join(gather.ROOT, "hal", "drivers", "vgatext")))
        self.assertFalse(os.path.exists(os.path.join(gather.ROOT, "rt")))
        rt = rd("spikes", "d0-rust-dos", "rt", "Cargo.toml")
        self.assertIn("desk-pic", rt)
        self.assertNotIn("desk-pit", rt)
        self.assertNotIn("desk-serial", rt)

    def test_the_serial_sequence_in_the_prose_is_the_drivers(self):
        for const, val in (("LCR_DLAB", "0x80"), ("LCR_8N1", "0x03"), ("FCR_ENABLE_CLEAR_14", "0xC7"),
                           ("MCR_DTR_RTS_OUT2", "0x0B"), ("MCR_LOOPBACK", "0x1E"), ("PROBE_BYTE", "0xAE")):
            self.assertRegex(self.serial, r"const %s: u8 = %s;" % (const, val))
            self.assertIn("`%s`" % val, self.drv)
        for port, val in (("0x3F9", "0x00"), ("0x3FB", "0x80"), ("0x3F8", "0x01"), ("0x3F9", "0x00"), ("0x3FB", "0x03"),
                          ("0x3FA", "0xC7"), ("0x3FC", "0x0B")):
            self.assertIn("(%s, %s)" % (port, val), self.serial)
        self.assertIn("pub const WRITE_POLLS: u32 = 100_000;", self.serial)
        self.assertEqual(115200 // 9600, 12)
        self.assertEqual(115200 // 2, 57600)
        self.assertIn("(12, 0x03, 0xC7, 0x0B, 0)", self.serial)
        # the COM2 writes of the worked example
        for port, val in (("0x2F9", "0x00"), ("0x2FB", "0x80"), ("0x2F8", "0x0C"), ("0x2FB", "0x03"), ("0x2FA", "0xC7"), ("0x2FC", "0x0B")):
            self.assertIn("`%s`←`%s`" % (port, val), self.drv)
        self.assertEqual(0xC7, 0b11000111)
        self.assertEqual(0x0B, 1 | 2 | 8)
        self.assertEqual(0x1E, 0x10 | 0x08 | 0x04 | 0x02)

    def test_the_pic_sequence_in_the_prose_is_the_drivers(self):
        for const, val in (("ICW1_INIT_ICW4", "0x11"), ("ICW3_SLAVE_ON_IRQ2", "0x04"), ("ICW3_SLAVE_ID", "0x02"),
                           ("ICW4_8086", "0x01"), ("OCW2_EOI", "0x20"), ("OCW3_READ_ISR", "0x0B"), ("OCW3_READ_IRR", "0x0A")):
            self.assertRegex(self.pic, r"const %s: u8 = %s;" % (const, val))
        for pair in ("(0x20, 0x11)", "(0xA0, 0x11)", "(0x21, 0x20)", "(0xA1, 0x28)", "(0x21, 0x04)", "(0xA1, 0x02)",
                     "(0x21, 0x01)", "(0xA1, 0x01)", "Access::outb(0x21, 0xFE)", "Access::outb(0xA1, 0xFF)"):
            self.assertIn(pair, self.pic)
        for needle in ("`0x20` ← `0x11`", "`0xA0` ← `0x11`", "`0x21` ← `0x04`", "`0xA1` ← `0x02`", "`0x0B`", "`0xFFFE`"):
            self.assertIn(needle, self.drv)
        self.assertEqual((0xFFFE & 0xFF, 0xFFFE >> 8), (0xFE, 0xFF))
        self.assertEqual(0x28 + 4, 0x2C)
        self.assertEqual(0x70 + 4, 0x74)
        self.assertIn("assert_eq!(pic.in_service_mask(), 0x1001", self.pic)
        self.assertIn("assert_eq!(pic.pending_mask(), 0x0002", self.pic)

    def test_the_pit_numbers_follow_from_the_constants(self):
        self.assertIn("pub const INPUT_HZ: u32 = 1_193_182;", self.pit)
        self.assertIn("const CMD_CH0_LOHI_RATE: u8 = 0x34;", self.pit)
        self.assertIn("const CMD_CH0_LOHI_SQUARE: u8 = 0x36;", self.pit)
        self.assertIn("const CMD_CH0_LATCH: u8 = 0x00;", self.pit)
        self.assertEqual(0x34, 0b00110100)
        self.assertEqual(0x36, 0b00110110)

        def div(hz):
            return (1193182 + hz // 2) // hz

        self.assertEqual((div(19), div(60), div(100), div(1000), div(795454)), (62799, 19886, 11932, 1193, 2))
        self.assertGreater(div(18), 65536)
        self.assertLess(div(795455), 2)
        self.assertEqual(hex(1193), "0x4a9")

        def mhz(d):
            return (1193182 * 1000 + d // 2) // d

        self.assertEqual((mhz(11932), mhz(65536), mhz(1193)), (99998, 18207, 1000153))
        self.assertEqual(25 * 1_000_000 // 99998, 250)
        for needle in ("62,799", "66,288", "795,454", "99,998", "18,207", "1,000,153", "`0x04A9`"):
            self.assertIn(needle, self.drv)
        self.assertAlmostEqual(1e9 / 1193182, 838, delta=1)

    def test_the_kernel_calls_what_the_platform_table_says(self):
        self.assertIn("SERIAL.init(115_200)", rd("kernel", "src", "serial.rs"))
        self.assertIn("PIC.init(REMAPPED_BASES, 0xFFFE)", rd("kernel", "src", "pic.rs"))
        self.assertIn("program_clock(&CLOCK, hz)", rd("kernel", "src", "pit.rs"))
        self.assertIn("IrqHandler::from_shared(CLOCK.counter())", rd("kernel", "src", "pit.rs"))
        bios = rd("kernel", "src", "bios.rs")
        for call in ("PIC.pending_mask()", "PIC.in_service_mask()", "PIC.init(BIOS_BASES"):
            self.assertIn(call, bios)
        dos_irq = rd("spikes", "d0-rust-dos", "rt", "src", "irq.rs")
        for call in ("pic.is_spurious(irq)", "pic.eoi_spurious(irq)", "pic.eoi(irq)", "pic.unmask(irq)"):
            self.assertIn(call, dos_irq)
        self.assertNotIn("Pic8259::new(unsafe { RawPorts::new() }).init", dos_irq)
        for f in sorted(os.listdir(os.path.join(gather.ROOT, "spikes", "d0-rust-dos", "rt", "src"))):
            self.assertNotIn("restore_bios_rate", rd("spikes", "d0-rust-dos", "rt", "src", f))
        for f in ("serial.rs", "pic.rs", "pit.rs", "hal.rs", "main.rs", "bios.rs"):
            for fn in ("restore_bios_rate", "read_counter", ".probe()", "read_byte"):
                self.assertNotIn(fn, rd("kernel", "src", f), f + " " + fn)

    def test_the_test_counts_the_prose_gives(self):
        def count(*parts):
            return len(re.findall(r"#\[test\]", rd(*parts)))
        self.assertEqual(count("hal", "drivers", "serial", "src", "lib.rs"), 8)
        self.assertEqual(count("hal", "drivers", "pic", "src", "lib.rs"), 6)
        self.assertEqual(count("hal", "drivers", "pit", "src", "lib.rs"), 8)
        self.assertIn("eight tests", self.drv)
        self.assertIn("six tests", self.drv)


if __name__ == "__main__":
    unittest.main()
