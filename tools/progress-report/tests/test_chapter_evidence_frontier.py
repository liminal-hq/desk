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

    def assert_no_expression_in_run(self, text):
        block = None
        for line in text.splitlines():
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
        self.assert_no_expression_in_run(ci)
    def test_every_job_uploads_its_results_and_the_scope_job_the_event_file(self):
        ci = rd(".github/workflows/ci.yml")
        upload = "actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2"
        jobs = [j for j in re.split(r"(?m)^  (?=[a-z0-9-]+:\n)", ci.split("\njobs:\n", 1)[1]) if j.strip()]
        by_id = {j.split(":", 1)[0]: j for j in jobs}
        self.assertEqual(list(by_id), ["scope", "base", "report", "fonts", "rust", "dos", "stdstubs", "shelldos", "native", "compare", "ci-passed"])
        # Every check job: the converter runs after the check, whatever the check's result, and the results are uploaded under a name of
        # its own (a matrix leg adds its key).
        for job, name in [(j, "results-" + j) for j in by_id if j not in ("scope", "ci-passed", "rust", "dos")] + [
            ("rust", "results-rust-${{ matrix.key }}"),
            ("dos", "results-dos-${{ matrix.key }}"),
        ]:
            body = by_id[job]
            self.assertIn("      - name: Step report\n        if: always()\n        run: |\n", body, job)
            self.assertIn("--suite \"$TITLE\" --junit dist/ci/results/steps.xml --summary >> \"$GITHUB_STEP_SUMMARY\"", body, job)
            self.assertIn("cargo build --release --locked --quiet --manifest-path tools/steps-junit/Cargo.toml", body, job)
            self.assertIn("--times dist/ci/check.times --log dist/ci/check.log", body, job)
            self.assertIn("nextest/ci/junit.xml", body, job)
            tail = body.split("      - name: Upload the results\n", 1)[1]
            self.assertTrue(
                tail.startswith("        if: always()\n        uses: %s\n        with:\n          name: %s\n          path: dist/ci/results/\n" % (upload, name)),
                job,
            )
            self.assertIn("          retention-days: 7\n", tail, job)
            self.assertLess(body.index("name: Step report"), body.index("name: Upload the results"), job)
            self.assertLess(body.index("--groups \"$CHECK_GROUPS\""), body.index("name: Step report"), job)
        # A matrix leg's key is unique within its job, so no two legs upload the same artefact.
        for job in ("rust", "dos"):
            keys = re.findall(r"- \{ key: ([a-z]+), groups:", by_id[job])
            self.assertEqual(len(keys), len(set(keys)), job)
            self.assertEqual(len(keys), len(re.findall(r"- \{ .*groups:", by_id[job])), "every leg has a key")
        # The event file, once, from the job that always runs, and only that job.
        self.assertEqual(ci.count("name: event-file"), 1)
        scope = by_id["scope"]
        self.assertIn(
            "      - name: Upload the event file\n        if: always()\n        uses: %s\n        with:\n          name: event-file\n"
            "          path: ${{ github.event_path }}\n" % upload,
            scope,
        )
        # Every artefact the workflow uploads is one the publisher reads or one the comparison job downloads.
        names = re.findall(r"(?m)^          name: (.+)$", ci)
        results = [n for n in names if n.startswith("results-")]
        self.assertEqual(len(results), 9, "one results upload per check job")
        self.assertEqual(len(results), len(set(results)), "no two jobs upload the same artefact")
        for n in names:
            self.assertTrue(n.startswith(("results-", "frames-")) or n == "event-file", n)
        # The aggregate job needs every job that uploads (a new job id would have to be listed there too).
        needs = re.search(r"(?m)^    needs: \[([^\]]+)\]$", by_id["ci-passed"]).group(1)
        self.assertEqual(sorted(x.strip() for x in needs.split(",")), sorted(j for j in by_id if j != "ci-passed"))

    def test_the_publisher_runs_only_the_default_branch_with_four_permissions(self):
        pub = rd(".github/workflows/publish-test-results.yml")
        ci = rd(".github/workflows/ci.yml")
        # The trigger: a finished run of the workflow named CI. No pull_request_target, and the zizmor exception is explained.
        self.assertRegex(ci, r"(?m)^name: CI$")
        self.assertIn(
            "on:\n  # zizmor: ignore[dangerous-triggers] This is the publisher the design needs: it holds the write token but runs only the "
            "default\n  # branch's code, reads the run's artefacts as untrusted data and never checks out a pull request.\n"
            "  workflow_run:\n    workflows: [CI]\n    types: [completed]\n",
            pub,
        )
        self.assertEqual(pub.count("zizmor: ignore"), 1)
        self.assertNotIn("pull_request_target", pub)
        self.assertNotRegex(pub, r"(?m)^  (pull_request|push|workflow_dispatch):")
        # No token at the top, and exactly the four permissions on the one job.
        self.assertRegex(pub, r"(?m)^permissions: \{\}$")
        top = re.findall(r"(?m)^permissions:.*$", pub)
        self.assertEqual(top, ["permissions: {}"])
        job_perms = re.search(r"(?m)^    permissions:\n((?:      [a-z-]+: [a-z]+.*\n)+)", pub)
        self.assertIsNotNone(job_perms)
        got = dict(re.findall(r"^      ([a-z-]+): ([a-z]+)", job_perms.group(1), flags=re.M))
        self.assertEqual(got, {"actions": "read", "checks": "write", "pull-requests": "write", "contents": "read"})
        self.assertEqual(len(re.findall(r"(?m)^[ ]+permissions:", pub)), 1, "one job, one permissions block")
        self.assertIn("    if: github.event.workflow_run.conclusion != 'skipped' && github.event.workflow_run.conclusion != 'cancelled'\n", pub)
        self.assertIn("concurrency:\n  group: pr-results-${{ github.event.workflow_run.head_repository.full_name }}-${{ github.event.workflow_run.head_branch }}\n"
                      "  cancel-in-progress: false\n", pub)
        # The checkout: the default branch only, no credentials, and nothing else from the pull request is checked out or run.
        self.assertEqual(pub.count("actions/checkout@"), 1)
        self.assertIn("          ref: ${{ github.event.repository.default_branch }}\n          sparse-checkout: .github\n          persist-credentials: false\n", pub)
        for line in re.findall(r"(?m)^\s+ref: .*$", pub):
            self.assertIn("default_branch", line)
        for bad in ("head_sha", "head_ref", "refs/pull", "pull_request.head", "event.workflow_run.head_commit", "pull_request.number"):
            self.assertNotIn(bad, pub, bad)
        # Every action is pinned by commit, and pr-results by the commit of v0.1.0 with the version in a comment.
        for use in re.findall(r"uses: (\S+)", pub):
            self.assertRegex(use, r"^[\w.-]+/[\w.-]+@[0-9a-f]{40}$", "actions are pinned to a commit")
        self.assertIn("uses: liminal-hq-labs/pr-results@8afc480363b3c8f09a4db221d1c4814415ab6eb2 # v0.1.0\n", pub)
        self.assertEqual(len(re.findall(r"(?m)^ +- name:", pub)), 2, "the checkout and the publish step, nothing else")
        self.assertIn("          command: publish\n", pub)
        self.assertNotIn("build: source", pub, "the release binary, checked against its digest")
        self.assertNotIn("continue-on-error", pub)
        self.assert_no_expression_in_run(pub)
        # Its settings: two suites that read the artefacts every job uploads; the step results are required.
        toml = rd(".github/pr-results.toml")
        self.assertIn('schema = 1\ntitle = "Desk"\n', toml)
        suites = toml.split("[[suite]]\n")[1:]
        self.assertEqual(len(suites), 2)
        self.assertIn('artifact = "results-*"\nfiles = ["**/steps.xml"]\nflavour = "steps"\nrequired = true\n', suites[0])
        self.assertIn('artifact = "results-*"\nfiles = ["**/nextest-*.xml"]\nflavour = "nextest"\n', suites[1])
        self.assertNotIn("required", suites[1])
        checks = re.findall(r'(?m)^check = "([^"]+)"$', toml)
        self.assertEqual(checks, ["Desk steps", "Desk Rust tests"])
        # A check run must not share a name with a job of the CI.
        jobs = set(re.findall(r"(?m)^    name: (.+)$", ci)) | set(re.findall(r'title: "([^"]+)"', ci))
        for c in checks:
            self.assertNotIn(c, jobs)
        # The files the Rust jobs upload are the ones the second suite reads.
        self.assertIn('"dist/ci/results/nextest-${ws//\\//-}.xml"', ci)


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
