# Checks of the layers explainer (M1) against tools/layers/layers.txt, the rules the layering check reads, and of the refusals it promises.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import copy
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


class LayersExplainerTests(unittest.TestCase):
    def setUp(self):
        self.m = module("layers")
        self.d = self.m.load()

    def test_the_real_rules_pass_and_have_the_shape_the_chapter_describes(self):
        self.assertEqual(self.m.check(self.d), [])
        self.assertEqual(self.d["order"], ["hal", "dos", "shared", "adapter-dos", "system-native", "hosted-skeleton", "spikes", "outside"])
        self.assertEqual(self.d["may"]["hal"] if "hal" in self.d["may"] else [], [], "the HAL names nothing")
        self.assertEqual(self.d["may"]["dos"], ["hal"])
        self.assertEqual(sorted(self.d["may"]["adapter-dos"]), ["dos", "hal", "shared"])
        self.assertEqual(sorted(self.d["may"]["system-native"]), ["hal", "shared"])
        self.assertNotIn("dos", self.d["may"]["system-native"], "the kernel does not use DOS")
        self.assertNotIn("system-native", self.d["may"]["dos"])
        self.assertEqual(self.d["layers"]["dos"], ["dos"])

    def test_the_hal_is_at_the_bottom_and_what_may_name_everything_is_at_the_top(self):
        rank = self.m.ranks(self.d)
        self.assertEqual(rank["hal"], 0)
        for lower, higher in (("hal", "dos"), ("dos", "adapter-dos"), ("shared", "adapter-dos"), ("shared", "system-native"), ("system-native", "spikes"), ("spikes", "outside")):
            self.assertLess(rank[lower], rank[higher], (lower, higher))

    def test_it_refuses_a_cycle_a_missing_path_an_unknown_layer_and_an_uncoloured_layer(self):
        d = copy.deepcopy(self.d)
        d["may"]["hal"] = ["dos"]
        self.assertTrue(any("cycle" in p for p in self.m.check(d)))
        d = copy.deepcopy(self.d)
        d["layers"]["dos"] = ["no-such-directory"]
        self.assertTrue(any("not in the repository" in p for p in self.m.check(d)))
        d = copy.deepcopy(self.d)
        d["may"]["shared"] = ["hal", "nowhere"]
        self.assertTrue(any("not a layer" in p for p in self.m.check(d)))
        d = copy.deepcopy(self.d)
        d["layers"]["newlayer"] = ["hal"]
        d["order"].append("newlayer")
        self.assertTrue(any("no colour" in p for p in self.m.check(d)))

    def test_it_draws_every_layer_with_a_text_version_and_a_step_for_the_exceptions(self):
        ids = [c["id"] for c in manual.load_plan()["chapters"]]
        html = self.m.render({}, mdlite.Ctx(chapters=ids))
        for name in self.d["order"]:
            self.assertIn(name, html)
        self.assertIn("The same layers as text", html)
        self.assertEqual(html.count('class="x-step"'), len(set(self.m.ranks(self.d).values())) + 1)
        self.assertIn("Recorded exceptions", html)

    def test_the_rules_file_and_the_chapter_name_the_same_layers(self):
        with open(os.path.join(gather.ROOT, "tools", "progress-report", "manual", "M01-map.md"), encoding="utf-8") as f:
            text = f.read()
        self.assertIn("<!-- explainer m-layers -->", text)
        self.assertIn("eight layers", text)
        self.assertEqual(len(self.d["order"]), 8)
        for name in self.d["order"]:
            self.assertIn("`%s`" % name, text, "the chapter names the layer " + name)


if __name__ == "__main__":
    unittest.main()
