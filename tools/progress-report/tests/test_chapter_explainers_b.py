# Checks of the frames-and-heap and the interrupt explainers (X4 and X5) against the kernel and the recorded run, and of the refusals each promises.
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


def rd(path):
    with open(os.path.join(gather.ROOT, path), encoding="utf-8") as f:
        return f.read()


class FramesExplainerTests(unittest.TestCase):
    def setUp(self):
        self.m = module("frames")
        self.t, self.r = self.m.transcript(), self.m.rules()

    def test_the_replay_reproduces_the_recorded_numbers(self):
        out = self.m.replay(self.t, self.r)
        self.assertEqual((out["after1"], out["first_mib"], out["kernel"], out["final"]), (3967, 159, 60, 3748))
        self.assertEqual(out["heap_first"] * 4096, 0x13C000)
        self.assertEqual(out["after_heap"], 3748 - 1024)
        self.assertEqual(self.m.check(self.t, self.r, out), [])
        self.assertEqual(sum((e - b) // 1024 for b, e, k in self.t["entries"] if k == "usable"), 15871)

    def test_it_refuses_a_total_a_count_and_a_heap_that_disagree(self):
        out = self.m.replay(self.t, self.r)
        self.assertTrue(any("usable entries sum" in p for p in self.m.check(dict(self.t, usable_kib=15870), self.r, out)))
        self.assertTrue(any("frames are free" in p for p in self.m.check(dict(self.t, free=3747), self.r, out)))
        self.assertTrue(any("first fit puts it" in p for p in self.m.check(dict(self.t, heap_at=0x140000), self.r, out)))
        self.assertTrue(any("heap.rs" in p for p in self.m.check(dict(self.t, heap_kib=2048), self.r, out)))

    def test_it_refuses_a_transcript_or_a_source_it_cannot_read(self):
        with self.assertRaises(ValueError):
            self.m.transcript("no map here")
        with self.assertRaises(ValueError):
            self.m.rules(rd("kernel/src/memory.rs").replace("let last = (r.base + r.len) / fs;", "let last = (r.base + r.len + fs - 1) / fs;"))

    def test_m7_draws_it(self):
        chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        self.assertIn("explainer m-frames", [" ".join([d["name"]] + d["args"]) for d in chapters["m-memory"]["blocks"] if d["type"] == "directive"])
        self.assertEqual(self.m.render({}, mdlite.Ctx()).count('class="x-step"'), 6)


class IrqExplainerTests(unittest.TestCase):
    def setUp(self):
        self.m = module("irq")
        self.f = self.m.facts()

    def test_the_facts_agree(self):
        self.assertEqual((self.f["base_master"], self.f["base_slave"], self.f["irq_base"], self.f["timer_gate"]), (0x20, 0x28, 0x20, 0x20))
        self.assertEqual(self.f["mask"], 0x60227D00)
        self.assertEqual((self.f["names"][0], self.f["names"][13]), ("divide error", "general protection fault"))
        self.assertEqual(self.m.check(self.f), [])

    def test_it_refuses_sources_that_disagree(self):
        self.assertTrue(any("IRQ_BASE" in p for p in self.m.check(dict(self.f, irq_base=0x30))))
        self.assertTrue(any("does not contain" in p for p in self.m.check(dict(self.f, div0="EXCEPTION 0x00: breakpoint"))))
        swapped = self.f["dispatch"].replace("h.call()", "XX").replace("PIC.eoi(irq)", "h.call()").replace("XX", "PIC.eoi(irq)")
        self.assertTrue(any("before it sends the end-of-interrupt" in p for p in self.m.check(dict(self.f, dispatch=swapped))))
        with self.assertRaises(ValueError):
            self.m.facts({"idt": "nothing"})

    def test_m6_draws_it_and_the_vectors_with_error_codes_are_the_cpus(self):
        chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        self.assertIn("explainer m-irq", [" ".join([d["name"]] + d["args"]) for d in chapters["m-interrupts"]["blocks"] if d["type"] == "directive"])
        self.assertEqual([v for v in range(32) if (1 << v) & self.f["mask"]], [8, 10, 11, 12, 13, 14, 17, 21, 29, 30])
        self.assertEqual(self.m.render({}, mdlite.Ctx()).count('class="x-step"'), 9)
