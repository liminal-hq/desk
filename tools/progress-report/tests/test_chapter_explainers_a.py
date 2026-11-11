# Checks of the address-translation and GDT explainers (X1 and X2) against the loader and the kernel, and of the refusals each promises.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import importlib.util
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
TOOL = os.path.normpath(os.path.join(HERE, ".."))
sys.path.insert(0, TOOL)

import gather  # noqa: E402
import manual  # noqa: E402
import mdlite  # noqa: E402


def module(name):
    spec = importlib.util.spec_from_file_location("t_" + name, os.path.join(manual.MANUAL_DIR, "explainers", name + ".py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def stage2():
    with open(os.path.join(gather.ROOT, "boot", "stage2.asm"), encoding="utf-8") as f:
        return f.read()


class AddressExplainerTests(unittest.TestCase):
    def setUp(self):
        self.m = module("address")

    def test_the_loaders_test_addresses_are_read_from_the_source(self):
        a, b = self.m.test_addresses()
        self.assertEqual((a, b), ((0x0000, 0x0500), (0xFFFF, 0x0510)))
        self.assertEqual(self.m.linear(*a), 0x00500)
        self.assertEqual(self.m.linear(*b), 0x100500)
        self.assertEqual(self.m.check(a, b), [])

    def test_it_refuses_addresses_that_do_not_alias_or_do_not_reach_the_second_megabyte(self):
        self.assertTrue(any("does not land on" in p for p in self.m.check((0, 0x500), (0xFFFF, 0x0520))))
        self.assertTrue(any("does not reach above 1 MiB" in p for p in self.m.check((0, 0x500), (0x0000, 0x0510))))
        self.assertTrue(any("needs more than twenty bits" in p for p in self.m.check((0xFFFF, 0x0510), (0xFFFF, 0x0510))))

    def test_it_refuses_a_source_it_cannot_read(self):
        with self.assertRaises(ValueError):
            self.m.test_addresses(stage2().replace("mov si, 0x0500", "mov si, bx"))
        with self.assertRaises(ValueError):
            self.m.test_addresses(stage2().replace("not ax\n        mov es, ax", "mov ax, 0x1000\n        mov es, ax"))

    def test_the_chapter_quotes_the_numbers_it_computes(self):
        html = self.m.render({}, mdlite.Ctx())
        for needle in ("0x100500", "0x10ffef", "wraps to 0xffef", "0x28"):
            self.assertIn(needle, html.lower())
        self.assertEqual(html.count('class="x-step"'), 8)


class GdtExplainerTests(unittest.TestCase):
    def setUp(self):
        self.m = module("gdt")

    def test_the_tables_are_read_and_decode_to_a_flat_pair(self):
        loader, kernel = self.m.loader_table(), self.m.kernel_table()
        self.assertEqual(loader, [0, 0x00CF9A000000FFFF, 0x00CF92000000FFFF])
        self.assertEqual(kernel, loader)
        code = self.m.decode(loader[1])
        self.assertEqual((code["base"], code["limit_field"], code["limit_bytes"], code["access"], code["flags"]), (0, 0xFFFFF, 1 << 32, 0x9A, 0xC))
        self.assertTrue(code["e"] and code["rw"] and code["p"] and code["s"] and not code["dc"])
        data = self.m.decode(loader[2])
        self.assertEqual((data["access"], data["e"], data["rw"]), (0x92, False, True))
        self.assertEqual(self.m.check(loader, kernel), [])

    def test_it_refuses_a_table_that_is_not_flat_or_that_disagrees(self):
        loader = self.m.loader_table()
        small = [0, 0x00CF9A000000FFFF & ~(0xF << 48) | (0x1 << 48), loader[2]]
        self.assertTrue(any("not a flat 4 GiB ring 0 code" in p for p in self.m.check(small, loader)))
        ring3 = [0, loader[1] | (3 << 45), loader[2]]
        self.assertTrue(any("not a flat 4 GiB ring 0 code" in p for p in self.m.check(ring3, ring3)))
        swapped = [0, loader[2], loader[1]]
        self.assertTrue(self.m.check(swapped, swapped))
        self.assertTrue(any("different" in p for p in self.m.check(loader, [0, loader[1], loader[2] ^ 1])))
        self.assertTrue(any("not three" in p for p in self.m.check(loader[:2], loader)))
        with self.assertRaises(ValueError):
            self.m.loader_table("no table here")
        with self.assertRaises(ValueError):
            self.m.kernel_table("static GDT: [u64; 4] = [0, 1, 2];")

    def test_the_chapters_use_the_figure_and_reuse_it(self):
        chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        for cid in ("m-cpu", "m-loader", "m-interrupts"):
            self.assertIn("explainer m-gdt", [" ".join([d["name"]] + d["args"]) for d in chapters[cid]["blocks"] if d["type"] == "directive"], cid)
        self.assertIn("explainer m-address", [" ".join([d["name"]] + d["args"]) for d in chapters["m-cpu"]["blocks"] if d["type"] == "directive"])
        self.assertEqual(self.m.render({}, mdlite.Ctx()).count('class="x-step"'), 10)
