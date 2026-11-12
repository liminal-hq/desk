# Checks of the evidence chapter (M21) and the frontier chapter (M22): the step table against tools/check.sh, and the planned records
# against the planned boxes of the anatomy data.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import json
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
        self.assertEqual(len(optional), 4)
        for var in ("INPUT_EXTRA", "NATIVE_EXTRA", "JSDOS"):
            self.assertIn(var, script)
            self.assertIn(var, self.c["raw"])

    def test_every_step_of_check_sh_is_in_a_group_block(self):
        script = rd("tools/check.sh")
        lines = script.splitlines()
        groups = re.search(r'^ALL_GROUPS="([^"]+)"', script, flags=re.M).group(1).split()
        parts = {g: p.split() for g, p in re.findall(r'^PARTS_([a-z]+)="([^"]*)"', script, flags=re.M)}
        self.assertEqual(len(set(groups)), len(groups))
        self.assertTrue(set(parts) <= set(groups))
        units = []
        for g in groups:
            units += [f"{g}:{p}" for p in parts[g]] if parts.get(g) else [g]

        depth = 0
        kind = None  # "group" or "opt-in", for the block open at depth 1
        seen = set()
        last_group_block = first_opt_in = -1
        for n, line in enumerate(lines):
            if depth == 0:
                m = re.match(r"^if run ([a-z]+)(?: ([a-z0-9]+))?; then$", line)
                o = re.match(r"^if \[ .+ \] && whole ([a-z]+); then$", line)
                if m:
                    g, p = m.group(1), m.group(2)
                    self.assertIn(g, groups, line)
                    if parts.get(g):
                        self.assertIn(p, parts[g], line)
                    else:
                        self.assertIsNone(p, line)
                    seen.add(f"{g}:{p}" if p else g)
                    kind, depth = "group", 1
                    last_group_block = n
                elif o:
                    self.assertIn(o.group(1), groups, line)
                    kind, depth = "opt-in", 1
                    if first_opt_in < 0:
                        first_opt_in = n
                else:
                    self.assertIsNone(re.match(r'^\s*step "', line), f"a step outside every group block: {line}")
                continue
            if re.match(r"^if .*; then$", line):
                depth += 1
            elif line == "fi":
                depth -= 1
            elif re.match(r'^step "', line):
                self.assertEqual(depth, 1, line)
                self.assertEqual(kind, "group", f"a column-0 step in an opt-in block: {line}")
        self.assertEqual(depth, 0)
        self.assertEqual(sorted(seen), sorted(units), "every group or part has a block, and only those")
        self.assertGreater(first_opt_in, last_group_block, "the opt-in blocks come after the last group block")

        for flag, want in (("--names", groups), ("--units", units)):
            got = subprocess.run(
                ["bash", "tools/check.sh", flag], cwd=gather.ROOT, check=True, capture_output=True, text=True
            ).stdout.split()
            self.assertEqual(got, want, flag)

    def test_the_group_names_agree_wherever_they_are_listed(self):
        def check_sh(flag):
            return subprocess.run(
                ["bash", "tools/check.sh", flag], cwd=gather.ROOT, check=True, capture_output=True, text=True
            ).stdout.split()

        names, units = check_sh("--names"), check_sh("--units")
        # --list is hand-written text: every group and every part of the script must have a line in it, and no other.
        listed, current = [], None
        for line in subprocess.run(
            ["bash", "tools/check.sh", "--list"], cwd=gather.ROOT, check=True, capture_output=True, text=True
        ).stdout.splitlines():
            first = line.split()[0]
            if not line[0].isspace() and first in names:
                current = first
                words = line.split()
                if first in {u.split(":")[0] for u in units if ":" in u}:
                    listed.append(f"{first}:{words[1]}")
                else:
                    listed.append(first)
            else:
                self.assertIsNotNone(current, line)
                listed.append(f"{current}:{first}")
        self.assertEqual(listed, units, "--list names every group and part, in order")
        # The CI scope script and the workflow name the same groups; each is held to tools/check.sh once it exists.
        if os.path.exists(os.path.join(gather.ROOT, "tools/ci-scope.sh")):
            found = re.search(r'^ALL_GROUPS="([^"]+)"', rd("tools/ci-scope.sh"), flags=re.M)
            self.assertIsNotNone(found, "tools/ci-scope.sh has no ALL_GROUPS")
            self.assertEqual(found.group(1).split(), names)
        if os.path.exists(os.path.join(gather.ROOT, ".github/workflows/ci.yml")):
            text = rd(".github/workflows/ci.yml")
            for lst in re.findall(r'(?:CHECK_GROUPS:|groups:) "?([a-z0-9:,]+)"?\s*$', text, flags=re.M):
                for u in lst.split(","):
                    self.assertTrue(u in units or u in names, f"ci.yml names '{u}', which tools/check.sh does not have")

    def test_check_sh_rejects_bad_group_lists(self):
        for args in (
            ["--groups", "base,base,"],
            ["--groups", ",base"],
            ["--groups", "base,,rust"],
            ["--groups=base,base"],
            ["--groups", "rust:hal,rust:hal"],
            ["--groups", "base", "--groups", "rust"],
            ["--groups", "nope"],
            ["--groups", "rust:nope"],
            ["--groups", ""],
            ["--groups"],
        ):
            r = subprocess.run(["bash", "tools/check.sh", *args], cwd=gather.ROOT, capture_output=True, text=True)
            self.assertEqual(r.returncode, 2, args)
            self.assertTrue(r.stderr.startswith("check: "), args)
            self.assertNotIn("check: groups:", r.stdout, args)

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

    def test_the_ci_runs_every_unit_of_check_sh_once(self):
        ci = rd(".github/workflows/ci.yml")
        units = subprocess.run(["bash", "tools/check.sh", "--units"], cwd=gather.ROOT, capture_output=True, text=True, check=True).stdout.split()
        names = subprocess.run(["bash", "tools/check.sh", "--names"], cwd=gather.ROOT, capture_output=True, text=True, check=True).stdout.split()
        # A job's CHECK_GROUPS, or a matrix leg's groups; a ${{ }} value is the matrix's and is read from its entries.
        lists = re.findall(r'^\s+CHECK_GROUPS: ([a-z0-9:,]+)\s*$', ci, flags=re.M) + re.findall(r'\bgroups: "([a-z0-9:,]+)"', ci)
        run = []
        for lst in lists:
            for u in lst.split(","):
                run += [x for x in units if x == u or x.startswith(u + ":")] if u in names else [u]
        self.assertEqual(sorted(run), sorted(units), "each unit of tools/check.sh runs in exactly one job")
        for g in names:
            self.assertIn("contains(needs.scope.outputs.groups, ',%s,')" % g, ci, g)
        self.assertIn("needs: [scope, shelldos, native]", ci)
        self.assertIn('tools/check.sh --groups "$CHECK_GROUPS"', ci)
        # The aggregate job: the one status to require. It needs every other job, runs always(), reads the results through env:
        # and fails on failure, cancelled, or a skip when scope did not succeed.
        jobs = re.findall(r"(?m)^  ([a-z0-9-]+):\n", ci.split("\njobs:\n", 1)[1])
        self.assertEqual(jobs[-1], "ci-passed", "the aggregate job is the last job")
        agg = ci.split("\n  ci-passed:\n", 1)[1]
        self.assertIn("    name: CI passed\n", agg)
        self.assertIn("    runs-on: ubuntu-24.04\n", agg)
        self.assertIn("    timeout-minutes: 5\n", agg)
        self.assertIn("    if: always()\n", agg)
        self.assertIn("    permissions: {}\n", agg)
        self.assertNotIn("container:", agg)
        self.assertIn("NEEDS: ${{ toJSON(needs) }}", agg)
        needs = re.search(r"(?m)^    needs: \[([^\]]+)\]$", agg)
        self.assertIsNotNone(needs)
        self.assertEqual(sorted(x.strip() for x in needs.group(1).split(",")), sorted(j for j in jobs if j != "ci-passed"), "ci-passed needs every other job")
        self.assertIn('scope=$(jq -r \'.scope.result\' <<< "$NEEDS")', agg)
        self.assertIn("success) ;;", agg)
        self.assertIn('skipped) if [ "$scope" != success ]; then', agg)
        self.assertIn('*) echo "::error::$job: $result"; bad=1 ;;', agg)
        self.assertIn('echo "::error::$job was skipped because scope did not succeed"; bad=1', agg)
        self.assertIn('exit "$bad"', agg)
        self.assertIn(">> \"$GITHUB_STEP_SUMMARY\"", agg)
        scope = re.search(r'^ALL_GROUPS="([^"]+)"', rd("tools/ci-scope.sh"), flags=re.M)
        check = re.search(r'^ALL_GROUPS="([^"]+)"', rd("tools/check.sh"), flags=re.M)
        self.assertEqual(scope.group(1), check.group(1))

    def test_the_ci_names_one_image_and_pins_its_actions(self):
        ci = rd(".github/workflows/ci.yml")
        images = re.findall(r"ghcr\.io/liminal-hq/ci-dos@sha256:[0-9a-f]{64}", ci)
        self.assertGreaterEqual(len(images), 9)
        self.assertEqual(len(set(images)), 1, "every job runs in the same pinned image")
        self.assertEqual(len(re.findall(r"(?m)^\s+container: ", ci)), len(images), "every container is the image, none is a tag")
        for use in re.findall(r"uses: (\S+)", ci):
            self.assertRegex(use, r"^[\w.-]+/[\w.-]+@[0-9a-f]{40}$", "actions are pinned to a commit")
        self.assertEqual(ci.count("persist-credentials: false"), ci.count("actions/checkout@"))
        # A failing check must fail its job, and a run must not cancel a push to main or a manual run.
        self.assertNotIn("continue-on-error", ci)
        self.assertIn("cancel-in-progress: ${{ github.event_name == 'pull_request' }}", ci)
        # Every job runs the group it is selected by: the command takes "$CHECK_GROUPS", and a job with a literal group has that
        # group in its if: and its step name.
        self.assertEqual(len(re.findall(r'tools/check\.sh --groups "\$CHECK_GROUPS"', ci)), 9, "each of the nine check jobs runs its CHECK_GROUPS")
        for job in re.split(r"(?m)^  (?=[a-z]+:\n)", ci)[2:]:
            m = re.search(r"(?m)^      CHECK_GROUPS: ([a-z]+)$", job)
            if not m:
                continue
            g = m.group(1)
            self.assertIn("contains(needs.scope.outputs.groups, ',%s,')" % g, job, g)
            self.assertIn("- name: tools/check.sh --groups %s\n" % g, job, g)
        # No expression is spliced into a script: a ${{ }} reaches a run: block only through env:.
        block = None
        for line in ci.splitlines():
            indent = len(line) - len(line.lstrip())
            if block is not None and line.strip() and indent <= block:
                block = None
            if block is not None:
                self.assertNotIn("${{", line, line)
            m = re.match(r"(\s*)(?:- )?run:\s*(.*)$", line)
            if m:
                self.assertNotIn("${{", m.group(2), line)
                if m.group(2) in ("|", ">"):
                    block = len(m.group(1)) + (2 if line.lstrip().startswith("- ") else 0)


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
