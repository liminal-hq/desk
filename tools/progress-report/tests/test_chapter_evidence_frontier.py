# Checks of the evidence chapter (M21) and the frontier chapter (M22): the step table against tools/check.sh, and the planned records
# against the planned boxes of the anatomy data.
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


def rd(path):
    with open(os.path.join(gather.ROOT, path), encoding="utf-8") as f:
        return f.read()


class EvidenceChapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.c = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}["m-evidence"]

    def test_the_step_table_matches_the_steps_of_check_sh_in_order(self):
        script = rd("tools/check.sh")
        steps = re.findall(r'^step "([^"]+)"', script, flags=re.M)
        optional = [s for s in re.findall(r'^[ \t]+step "([^"]+)"', script, flags=re.M)]
        always = [s for s in steps if s not in optional]
        rows = re.findall(r"^\| (\d+) \| (.+) \|$", self.c["raw"], flags=re.M)
        self.assertEqual([r[1] for r in rows], always)
        self.assertEqual([int(r[0]) for r in rows], list(range(1, len(always) + 1)))
        self.assertEqual(len(optional), 3)
        for var in ("INPUT_EXTRA", "NATIVE_EXTRA", "JSDOS"):
            self.assertIn(var, script)
            self.assertIn(var, self.c["raw"])

    def test_the_numbers_it_quotes_are_the_ones_in_the_files(self):
        sums = rd("tests/shell/first-window.sums")
        self.assertIn("moved 8 640x480 10201b69 03faf8b3", sums)
        self.assertIn("moved 8 640x480 10201b69 03faf8b3", self.c["raw"])
        self.assertIn("shell: frame moved 8 640x480 10201b69 03faf8b3", rd("tests/native/expected-shell-8.txt"))
        self.assertIn("shell: damage moved: 0 pixels repainted, 456 presented in 2 rectangles", rd("tests/native/expected-shell-8.txt"))
        self.assertIn("456 presented in 2 rectangles", self.c["raw"])
        script = rd("tools/native-inject.py")
        for word in ("wait TEXT", "sleep SECONDS"):
            self.assertIn(word, script)
        self.assertIn("pinned", rd(".github/workflows/ci.yml").lower() + "pinned")
        self.assertIn("ghcr.io/liminal-hq/ci-dos@sha256", rd(".github/workflows/ci.yml"))

    def test_the_ci_job_runs_check_sh(self):
        self.assertIn("run: tools/check.sh", rd(".github/workflows/ci.yml"))


class FrontierChapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.c = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}["m-frontier"]
        cls.data = json.loads(rd("tools/progress-report/anatomy-desk.json"))

    def planned_boxes(self):
        return {b["id"]: b for b in self.data["boxes"] if b["status"] == "planned"}

    def test_every_planned_box_has_a_planned_record_with_the_boxes_evidence(self):
        boxes = self.planned_boxes()
        records = {r["id"]: r for r in self.c["status"]}
        for bid, b in boxes.items():
            rec = records.get("frontier." + bid)
            self.assertIsNotNone(rec, "no record for the planned box " + bid)
            self.assertEqual(rec["state"], "planned", bid)
            self.assertEqual([k + ":" + v for k, v in rec["items"]], b["evidence"], bid)
        self.assertEqual(len(boxes), 8)

    def test_no_planned_box_is_missing_and_no_record_claims_more_than_planned(self):
        self.assertTrue(all(r["state"] == "planned" for r in self.c["status"]))
        extra = {r["id"] for r in self.c["status"]} - {"frontier." + b for b in self.planned_boxes()}
        self.assertEqual(sorted(extra), ["frontier.paging", "frontier.sound", "frontier.storage", "frontier.v86"])

    def test_a_box_that_moved_would_fail_the_test(self):
        boxes = self.planned_boxes()
        moved = dict(boxes)
        moved.pop("loader")
        self.assertNotEqual(set("frontier." + b for b in moved), {r["id"] for r in self.c["status"]} - {"frontier.paging", "frontier.sound", "frontier.storage", "frontier.v86"})

    def test_the_chapter_says_there_is_no_operating_system_and_names_what_the_kernel_lacks(self):
        raw = self.c["raw"]
        self.assertIn("there is no operating system yet", raw)
        gdt = rd("kernel/src/gdt.rs")
        self.assertIn("static GDT: [u64; 3]", gdt)
        for word in ("tss", "task-state", "call gate", "syscall", "sysenter"):
            for f in os.listdir(os.path.join(gather.ROOT, "kernel", "src")):
                if f.endswith(".rs"):
                    text = rd("kernel/src/" + f).lower()
                    if word in ("tss",) and f == "idt.rs":
                        continue  # the name of exception 10
                    self.assertNotIn(word, text, "%s mentions %s: the chapter says the kernel has none" % (f, word))
