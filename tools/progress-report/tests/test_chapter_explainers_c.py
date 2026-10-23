# Checks of the crate-graph, .DFN and target explainers (X6, X7 and X8) against the manifests, the font files and the target files, and of the
# refusals each promises.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import copy
import importlib.util
import os
import struct
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


def ctx():
    ids = [c["id"] for c in manual.load_plan()["chapters"]]
    return mdlite.Ctx(chapters=ids)


class CrateGraphTests(unittest.TestCase):
    def setUp(self):
        self.m = module("crates")
        self.nodes, self.order = self.m.load()

    def test_the_real_manifests_pass_and_have_the_shape_the_chapters_describe(self):
        self.assertEqual(self.m.check(self.nodes), [])
        kinds = {k: sorted(n for n, v in self.nodes.items() if v["kind"] == k) for k, _ in self.m.KINDS}
        self.assertEqual(kinds["hal"], ["desk-hal"])
        self.assertEqual(len(kinds["driver"]), 7)
        self.assertEqual(kinds["core"], ["desk-font", "desk-gfx", "desk-shell", "desk-ui", "desk-wm"])
        self.assertEqual(kinds["presenter"], ["desk-present"])
        self.assertEqual(self.nodes["desk-gfx"]["deps"], ["desk-hal"])
        self.assertIn("desk-vga", self.nodes["desk-gfx"]["dev"], "the core's one edge towards a driver is a test-only edge")
        self.assertIn("desk-present", self.nodes["desk-shell"]["dev"])

    def test_it_refuses_a_cycle_a_core_crate_that_reaches_down_and_an_unclassified_crate(self):
        n = copy.deepcopy(self.nodes)
        n["desk-hal"]["deps"] = ["desk-serial"]
        self.assertTrue(any("cycle" in p for p in self.m.check(n)))
        n = copy.deepcopy(self.nodes)
        n["desk-ui"]["deps"] = n["desk-ui"]["deps"] + ["desk-present"]
        self.assertTrue(any("depends on the presenter" in p for p in self.m.check(n)))
        n = copy.deepcopy(self.nodes)
        n["desk-gfx"]["deps"] = n["desk-gfx"]["deps"] + ["desk-vga"]
        self.assertTrue(any("not the event decoder" in p for p in self.m.check(n)))
        n = copy.deepcopy(self.nodes)
        n["desk-newthing"] = {"name": "desk-newthing", "kind": "driver", "path": "hal/drivers/newthing", "deps": [], "dev": []}
        self.assertTrue(any("no chapter accounts for" in p for p in self.m.check(n)))

    def test_it_draws_every_crate_with_a_link_and_a_text_version(self):
        html = self.m.render({}, ctx())
        for name in self.nodes:
            self.assertIn(name, html)
        self.assertIn("The same graph as text", html)
        self.assertEqual(html.count('class="x-step"'), 6)


class DfnTests(unittest.TestCase):
    def setUp(self):
        self.m = module("dfn")
        with open(os.path.join(gather.ROOT, self.m.FILE), "rb") as f:
            self.b = f.read()

    def test_the_replica_reads_the_four_committed_faces(self):
        for name, want in (("ui", (14, 11)), ("title", (14, 12)), ("fixed", (16, 13)), ("small", (8, 7))):
            with open(os.path.join(gather.ROOT, "fonts", "generated", name + ".dfn"), "rb") as f:
                h, b, count, glyphs = self.m.parse(f.read())
            self.assertEqual((h, b), want)
            self.assertEqual(len(glyphs), count)
            self.assertTrue(289 <= count <= 332)
            codes = [g[0] for g in glyphs]
            self.assertEqual(codes, sorted(set(codes)))
            self.assertTrue(all(c in codes for c in (ord("?"), 0x2026, 0xD7)) and 0xFFFD not in codes)
        self.assertEqual(self.m.check(self.b), [])

    def test_the_glyph_it_draws_is_the_letter_a(self):
        h, base, count, glyphs = self.m.parse(self.b)
        code, width, at, rows = [g for g in glyphs if g[0] == ord("A")][0]
        self.assertEqual(len(rows), h * -(-width // 8))
        self.assertTrue(any(rows), "the glyph has set pixels")
        self.assertGreater(sum(bin(v).count("1") for v in rows), 10)

    def test_it_refuses_what_the_parser_refuses_and_what_the_documents_contradict(self):
        for bad in (b"XFN1" + self.b[4:], self.b[:6], self.b[:20], self.b + b"\0", self.b[:4] + b"\0" + self.b[5:], self.b[:4] + bytes([14, 15]) + self.b[6:]):
            self.assertTrue(self.m.check(bad), bad[:8])
        self.assertTrue(any("a baseline of 10" in p for p in self.m.check(self.b[:5] + bytes([10]) + self.b[6:])))
        glyphs = self.m.parse(self.b)[3]
        fewer = self.b[:6] + struct.pack("<H", 288) + self.b[8:glyphs[288][2]]
        self.assertTrue(any("glyphs, outside" in p for p in self.m.check(fewer)), "a valid file with 288 glyphs is outside the documented range")
        self.assertTrue(any("no longer describes the DFN1 header" in p for p in self.m.check(self.b, formats="nothing")))

    def test_it_draws_every_row(self):
        html = self.m.render({}, ctx())
        self.assertEqual(html.count('class="x-step"'), 6 + 14)


class TargetsTests(unittest.TestCase):
    def setUp(self):
        self.m = module("targets")

    def test_the_table_is_the_files(self):
        files = self.m.load()
        self.assertEqual(self.m.check(files), [])
        self.assertEqual([l for _, l, _ in files], ["kernel", "dos-elf", "dos.json"])
        html = self.m.render({}, ctx())
        for needle in ("i486-unknown-none-elf", "i486-pc-windows-gnu", "gnu-lld", "ld.lld", "-mmx,-sse,-sse2", "no reason is recorded", "tg-diff", "tg-same"):
            self.assertIn(needle, html)

    def test_it_refuses_a_target_that_stops_being_a_32_bit_i486_one_and_a_field_it_has_no_group_for(self):
        with self.assertRaises(ValueError):
            self.m.load({"core/dos.json": "{not json"})
        files = self.m.load()
        bad = [(p, l, dict(d, cpu="pentium") if l == "kernel" else d) for p, l, d in files]
        self.assertTrue(any("not i486" in p for p in self.m.check(bad)))
        bad = [(p, l, dict(d, **{"target-pointer-width": 64}) if l == "dos-elf" else d) for p, l, d in files]
        self.assertTrue(any("pointer width" in p for p in self.m.check(bad)))
        import json
        with open(os.path.join(gather.ROOT, "core", "dos.json"), encoding="utf-8") as f:
            d = json.load(f)
        d["a-new-field"] = 1
        orig = self.m.src
        try:
            self.m.src = lambda p: json.dumps(d) if p == "core/dos.json" else orig(p)
            with self.assertRaises(ValueError):
                self.m.render({}, ctx())
        finally:
            self.m.src = orig
