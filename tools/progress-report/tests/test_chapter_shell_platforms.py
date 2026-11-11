# Checks of the shell chapter (M15) and the platforms chapter (M16): the facts they quote, against the sources.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import os
import re
import subprocess
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


class ShellPlatformsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        cls.shell = cls.chapters["m-shell"]["raw"]
        cls.plat = cls.chapters["m-platforms"]["raw"]
        cls.scn = rd("tests", "shell", "first-window.scn")
        cls.native = rd("tests", "native", "expected-shell-8.txt")
        cls.kernel = rd("kernel", "src", "shell.rs")
        cls.dos = rd("shell", "dos", "src", "main.rs")

    def test_the_scenario_facts_quoted_are_in_the_file(self):
        for line in ("open notes 60 60 300 180", "open welcome 220 150 340 200", "expect-damage 8 456 2",
                     "expect-damage 4 608 2", "pointer 120 70", "move 40 30", "pointer 543 161", "type \"desk\""):
            self.assertIn(line, self.scn)
        for text in ("60 60 300 180", "220 150 340 200", "expect-damage 8 456 2", "(120, 70)"):
            self.assertIn(text, self.shell + self.plat)

    def test_the_damage_arithmetic_matches_the_native_transcript(self):
        # extents are the outer boxes inflated by the halo of 3
        notes, welcome = (57, 57, 363, 243), (217, 147, 563, 353)
        box = lambda a, b: (min(a[0], b[0]), min(a[1], b[1]), max(a[2], b[2]), max(a[3], b[3]))
        area = lambda r: (r[2] - r[0]) * (r[3] - r[1])
        self.assertEqual(area(box(notes, welcome)), 149776)
        self.assertEqual(area(box(notes, (97, 87, 403, 273))), 74736)
        self.assertEqual(area(box((97, 87, 403, 273), welcome)), 123956)
        self.assertEqual(2 * 12 * 19, 456)
        for n in ("149776 pixels repainted, 149776 presented in 1", "74736 pixels repainted, 74736 presented in 1",
                  "123956 pixels repainted, 123956 presented in 1", "0 pixels repainted, 456 presented in 2",
                  "307200 pixels repainted, 307200 presented in 1"):
            self.assertIn(n, self.native)
        self.assertIn("HALO: i32 = 3", rd("shell", "ui", "src", "chrome.rs"))

    def test_every_row_of_the_side_by_side_table_cites_both_sides(self):
        sec = self.plat.split("### Side by side, line for line")[1].split("###")[0]
        rows = [r for r in sec.splitlines() if r.startswith("| ") and not r.startswith("| What") and not r.startswith("|---")]
        self.assertGreaterEqual(len(rows), 15)
        for r in rows:
            cells = [c.strip() for c in r.strip("|").split(" | ")]
            self.assertEqual(len(cells), 3, r)
            k = re.findall(r"\(sym:(kernel/src/(?:shell|hal)\.rs)::(\w+)\)", cells[1])
            d = re.findall(r"\(sym:(shell/dos/src/main\.rs)::(\w+)\)", cells[2])
            self.assertTrue(k and d, r)
            for path, name in k + d:
                self.assertRegex(rd(*path.split("/")), r"\b%s\b" % name, name)

    def test_there_is_no_platform_trait_and_nothing_claims_one(self):
        for dirpath, dirs, files in os.walk(gather.ROOT):
            dirs[:] = [d for d in dirs if d not in (".git", "target", "dist", "vendor", "node_modules", "worktrees", "design")]
            for f in files:
                if f.endswith(".rs"):
                    self.assertNotRegex(rd(os.path.relpath(os.path.join(dirpath, f), gather.ROOT)), r"\btrait\s+Platform\b", f)
        for text in (self.shell, self.plat):
            for m in re.finditer(r"[^.\n]*Platform` trait[^.\n]*\.", text):
                self.assertRegex(m.group(0), r"(?i)\b(no|not|sketch|planned|without|yet|never|nothing|would)\b", m.group(0))
        self.assertIn("planned", self.plat.split("id: platforms.trait")[1].split("note:")[0])

    def test_the_dos_exit_statuses_quoted_are_in_the_program(self):
        for s in ("0 success", "1 a", "2 the requested video mode", "3 a panic", "4 the interactive", "5 an argument"):
            self.assertIn(s, self.dos)

    def test_the_kernel_plays_with_run_and_dos_with_apply(self):
        self.assertIn("run as run_scenario", self.kernel)
        self.assertIn("apply(shell, step)", self.dos)
        self.assertNotIn("run_scenario", self.dos)


if __name__ == "__main__":
    unittest.main()
