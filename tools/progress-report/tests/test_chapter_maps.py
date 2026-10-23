# Checks of appendix B, "Maps and numbers": its tables are read from the files that hold the numbers, and a row whose proof is gone is refused.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import copy
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
TOOL = os.path.normpath(os.path.join(HERE, ".."))
sys.path.insert(0, TOOL)

import gather  # noqa: E402
import manual_maps as maps  # noqa: E402


class MapsTests(unittest.TestCase):
    def setUp(self):
        self.t = {x["id"]: x for x in maps.tables()}

    def test_every_table_is_there_and_every_proof_occurs_in_its_file(self):
        self.assertEqual(sorted(self.t), ["bios", "exceptions", "exit", "irq", "memory", "modes", "ports"])
        self.assertEqual(maps.check(list(self.t.values())), [])

    def test_the_numbers_are_the_ones_the_chapters_use(self):
        addrs = [int(r[0], 16) for r in self.t["memory"]["rows"]]
        for addr in (0x500, 0x820, 0x2000, 0x7C00, 0x7E00, 0x10000, 0x11000, 0x13000, 0x100000):
            self.assertIn(addr, addrs, hex(addr))
        statuses = {r[0] for r in self.t["exit"]["rows"]}
        self.assertTrue({"0", "1", "2", "124", "3", "33", "35", "125"} <= statuses, statuses)
        modes = [(r[0], r[1], r[2]) for r in self.t["modes"]["rows"]]
        self.assertEqual(modes[0], ("0", "none", "Normal"))
        self.assertEqual(modes[14], ("14", "shell-corrupt", "ShellCorrupt"))
        vec = {r[0]: r for r in self.t["irq"]["rows"]}
        self.assertEqual((vec["0"][2], vec["0"][3], vec["12"][2], vec["12"][3]), ("0x20", "0x08", "0x2C", "0x74"))
        self.assertIn(["0x00", "divide error", "no"], [r[:3] for r in self.t["exceptions"]["rows"]])
        self.assertIn(["0x0D", "general protection fault", "yes"], [r[:3] for r in self.t["exceptions"]["rows"]])

    def test_a_row_whose_proof_is_gone_is_refused(self):
        t = copy.deepcopy(list(self.t.values()))
        t[0]["rows"][0][-1] = ("boot/stage2.asm", "BOOTINFO        equ 0x9999")
        self.assertTrue(any("does not occur" in p for p in maps.check(t)))
        t = copy.deepcopy(list(self.t.values()))
        t[1]["rows"][0][-1] = ("hal/drivers/pit/src/lib.rs", "const NOT_A_PORT: u16 = 1;")
        self.assertTrue(maps.check(t))

    def test_the_mode_table_has_a_row_for_every_expected_file(self):
        listed = " ".join(r[3] for r in self.t["modes"]["rows"])
        for f in sorted(os.listdir(os.path.join(gather.ROOT, "tests", "native"))):
            if f.startswith("expected-") and f.endswith(".txt"):
                self.assertIn(f, listed, f)
