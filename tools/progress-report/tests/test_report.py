# Tests for the progress report generator: the markdown reader, the gather step on a throwaway git repository, a fixture build with
# link checking, and the real chapters and explainers against the repository's own files.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tools/progress-report/tests/test_report.py
import json
import os
import re
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
TOOL = os.path.normpath(os.path.join(HERE, ".."))
sys.path.insert(0, TOOL)

import build  # noqa: E402
import explainers as ex  # noqa: E402
import gather  # noqa: E402
import mdlite  # noqa: E402

ENV = dict(os.environ, GIT_AUTHOR_NAME="T", GIT_AUTHOR_EMAIL="t@example.com", GIT_COMMITTER_NAME="T", GIT_COMMITTER_EMAIL="t@example.com",
           GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_SYSTEM="/dev/null")


def rd(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


def wr(path, text):
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


def git(cwd, *args):
    return subprocess.run(["git", *args], cwd=cwd, env=ENV, check=True, capture_output=True, text=True).stdout


def make_repo(path):
    git(path, "init", "-q", "-b", "main")
    wr(os.path.join(path, "a.txt"), "one\n")
    git(path, "add", "a.txt")
    git(path, "commit", "-q", "-m", "Initial Commit")
    git(path, "checkout", "-q", "-b", "feat/x")
    os.makedirs(os.path.join(path, "docs"))
    wr(os.path.join(path, "docs", "x.md"), "x\n")
    git(path, "add", "docs/x.md")
    git(path, "commit", "-q", "-m", "docs: add x")
    git(path, "checkout", "-q", "main")
    git(path, "merge", "-q", "--no-ff", "-m", "Merge main into feat/x\n\n**Conflicts**\n- `tools/check.sh`: kept every step\n\nThe `ci.yml` merged cleanly.", "feat/x")
    wr(os.path.join(path, "b.txt"), "b\n")
    git(path, "add", "b.txt")
    git(path, "commit", "-q", "-m", "chore: b")
    git(path, "checkout", "-q", "-b", "feat/y")
    wr(os.path.join(path, "c.txt"), "c\n")
    git(path, "add", "c.txt")
    git(path, "commit", "-q", "-m", "feat: c")
    git(path, "checkout", "-q", "main")
    git(path, "merge", "-q", "--no-ff", "-m", "Add c (#1)", "feat/y")


class MdTests(unittest.TestCase):
    def test_front_matter(self):
        meta, body = mdlite.front_matter("---\nid: a\nprs: [1, 2]\ntitle: Has: a colon\n---\nbody\n")
        self.assertEqual(meta["id"], "a")
        self.assertEqual(meta["prs"], ["1", "2"])
        self.assertEqual(meta["title"], "Has: a colon")
        self.assertEqual(body.strip(), "body")
        with self.assertRaises(mdlite.MdError):
            mdlite.front_matter("no front matter")

    def test_inline_and_links(self):
        ctx = mdlite.Ctx({"m.x": 1234}, ["one"], "https://example.com/r", [])
        out = mdlite.inline("**b** *i* `c<d>` [t](chapter:one) [`f`](repo:docs/f.md) {{m.x}} `{{m.x}}`", ctx)
        self.assertIn("<strong>b</strong>", out)
        self.assertIn("<em>i</em>", out)
        self.assertIn("<code>c&lt;d&gt;</code>", out)
        self.assertIn('href="one.html"', out)
        self.assertIn('href="https://example.com/r/blob/main/docs/f.md"', out)
        self.assertIn("1,234", out)
        self.assertIn("<code>1,234</code>", out)

    def test_unknown_placeholder_and_chapter_fail(self):
        ctx = mdlite.Ctx({}, ["one"])
        with self.assertRaises(mdlite.MdError):
            mdlite.inline("{{nope}}", ctx)
        with self.assertRaises(mdlite.MdError):
            mdlite.inline("[x](chapter:missing)", ctx)

    def test_shelves_and_sidebar(self):
        blocks = mdlite.parse_blocks("### Catalogued\n- a\n\n### Uncatalogued\n- b\n\n### Wish list\n- c\n\n> **From the stacks — T.** Text.\n> Source: [s](https://example.com)\n")
        html, shelves = mdlite.render(blocks, mdlite.Ctx(), lambda n, a: "")
        self.assertEqual(sorted(shelves), ["catalogued", "uncatalogued", "wish"])
        self.assertIn('class="stacks"', html)
        self.assertIn("not evidence", html)


class GatherTests(unittest.TestCase):
    def test_gather_on_a_throwaway_repository(self):
        with tempfile.TemporaryDirectory() as d:
            make_repo(d)
            data = gather.gather(d, "main", offline=True)
            self.assertEqual(data["sources"]["gh"], "snapshot" if os.path.exists(os.path.join(gather.HERE, "snapshots", "gh.json")) else "none")
            self.assertEqual(len(data["prs"]), 1)
            pr = data["prs"][0]
            self.assertEqual(pr["number"], 1)
            self.assertEqual(pr["title"], "Add c")
            self.assertEqual(pr["commitCount"], 1)
            self.assertEqual(len(data["sync"]), 1)
            self.assertEqual(data["sync"][0]["branch"], "feat/x")
            self.assertEqual(data["sync"][0]["conflicts"][0]["paths"], ["tools/check.sh"])
            self.assertEqual(data["sync"][0]["clean"], ["ci.yml"])
            self.assertEqual(data["main"]["files"], 4)
            self.assertGreaterEqual(data["metrics"]["commitsAll"]["value"], 5)
            for m in data["metrics"].values():
                self.assertTrue(m["how"])

    def test_dry_run_survives_a_shallow_clone(self):
        with tempfile.TemporaryDirectory() as d:
            make_repo(d)
            clone = os.path.join(d, "clone")
            subprocess.run(["git", "clone", "-q", "--depth", "1", "file://" + d, clone], check=True, capture_output=True, env=ENV)
            data = gather.gather(clone, "HEAD", offline=True, dry_run=True)
            self.assertTrue(data["shallow"])
            self.assertIn("pull requests merged", gather.summary(data))

    def test_roadmap_downgrades_unmerged_nodes(self):
        road = {"branches": [], "nodes": [{"id": "x", "branch": "b", "state": "done", "pr": 9, "deps": []}, {"id": "y", "branch": "b", "state": "planned", "deps": []}]}
        progress, warnings = gather.resolve_roadmap(road, [{"number": 1, "ci": "passing"}])
        self.assertEqual(road["nodes"][0]["state"], "progress")
        self.assertEqual(len(warnings), 1)
        road = {"branches": [], "nodes": [{"id": "x", "branch": "b", "state": "done-emu", "pr": 1, "deps": []}]}
        gather.resolve_roadmap(road, [{"number": 1, "ci": "passing"}])
        self.assertEqual(road["nodes"][0]["state"], "done-emu")


class BuildTests(unittest.TestCase):
    def build_fixture(self, out):
        with tempfile.TemporaryDirectory() as d:
            make_repo(d)
            data = gather.gather(d, "main", offline=True)
        chapters = build.load_chapters(os.path.join(HERE, "fixture", "chapters"))
        site = build.Site(data, chapters, os.path.join(HERE, "fixture", "pages"), out, assets=False)
        site.write()
        return data, chapters

    def test_fixture_site_is_valid_and_links_resolve(self):
        with tempfile.TemporaryDirectory() as out:
            site_dir = os.path.join(out, "site")
            data, chapters = self.build_fixture(site_dir)
            self.assertEqual(build.check_site(site_dir, [c["id"] for c in chapters]), [])
            for f in ("index.html", "one.html", "two.html", "site.css", "site.js", "manifest.json", "_publish/index.html"):
                self.assertTrue(os.path.exists(os.path.join(site_dir, f)), f)
            index = rd(os.path.join(site_dir, "index.html"))
            self.assertIn("one.html", index)
            self.assertIn("Catalogued", index)
            self.assertIn("how measured", index)
            frag = rd(os.path.join(site_dir, "_publish", "index.html"))
            self.assertNotIn("<!doctype", frag.lower())
            self.assertNotIn("<body", frag)
            self.assertIn("<title>Desk Progress Report</title>", frag)
            manifest = json.loads(rd(os.path.join(site_dir, "manifest.json")))
            self.assertNotIn("index.html", manifest["publish"]["files"])
            self.assertIn("one.html", manifest["publish"]["files"])
            self.assertTrue(manifest["publish"]["file_path"].endswith("_publish/index.html"))
            self.assertEqual(manifest["publish"]["url"], build.ARTIFACT_URL)
            self.assertLess(max(e["bytes"] for e in manifest["files"]), manifest["limits"]["perFileBytes"])

    def test_the_checker_catches_a_broken_link_and_a_missing_anchor(self):
        with tempfile.TemporaryDirectory() as out:
            site_dir = os.path.join(out, "site")
            data, chapters = self.build_fixture(site_dir)
            p = os.path.join(site_dir, "two.html")
            text = rd(p).replace("</main>", '<a href="gone.html">x</a><a href="one.html#nope">y</a></main>')
            wr(p, text)
            problems = build.check_site(site_dir, ["one", "two"])
            self.assertTrue(any("broken link gone.html" in x for x in problems))
            self.assertTrue(any("anchor #nope" in x for x in problems))


REAL = os.path.join(TOOL, "chapters")


class RealContentTests(unittest.TestCase):
    def test_real_chapters_are_well_formed(self):
        chapters = build.load_chapters(REAL)
        ids = [c["id"] for c in chapters]
        self.assertGreaterEqual(len(chapters), 14)
        for c in chapters:
            self.assertRegex(c["date"], r"^\d{4}-\d{2}-\d{2}$", c["id"])
            shelves = {mdlite.shelf_of(b) for b in c["blocks"]} - {None}
            self.assertEqual(shelves, {"catalogued", "uncatalogued", "wish"}, c["id"] + " needs all three shelves")
            self.assertTrue(any(b["type"] == "quote" and b["lines"][0].startswith("**From the stacks") for b in c["blocks"]) or c["section"] == "reference", c["id"])
            for b in c["blocks"]:
                if b["type"] == "directive":
                    self.assertIn(b["name"], ("explainer", "figure", "facts"))
            text = rd(os.path.join(REAL, c["file"]))
            for target in re.findall(r"\]\(chapter:([\w-]+)", text):
                self.assertIn(target, ids, "%s links to a missing chapter" % c["id"])

    def test_sidebars_name_a_source_or_hedge(self):
        for c in build.load_chapters(REAL):
            for b in c["blocks"]:
                if b["type"] == "quote" and b["lines"][0].startswith("**From the stacks"):
                    self.assertTrue(any(l.startswith("Source") for l in b["lines"][1:]), c["id"] + " has a sidebar without a source line")
                    self.assertLessEqual(len(re.findall(r"[.!?](?:\s|$)", b["lines"][0])), 5, c["id"] + " sidebar is too long")

    def test_boot_explainer_against_the_transcript_and_expected_file(self):
        root = gather.ROOT
        expected = rd(os.path.join(root, "tests", "native", "expected.txt"))
        serial = rd(os.path.join(TOOL, "media", "native-serial.txt"))
        D = {"measured": json.loads(rd(os.path.join(TOOL, "snapshots", "measured.json")))}
        html = ex.boot(D, mdlite.Ctx(), serial, expected)
        self.assertEqual(html.count('class="x-step"'), 17)
        self.assertEqual(len(ex.parse_map(serial.splitlines())), 6)
        with self.assertRaises(ValueError):
            ex.boot(D, mdlite.Ctx(), serial.replace("boot complete", "boot failed"), expected)

    def test_expected_pattern_matcher(self):
        self.assertTrue(ex.expected_matches("a {N:1..32}\n...\nb {HEX}", ["a 5", "x", "b 0x1f"]))
        self.assertFalse(ex.expected_matches("a\nb", ["b", "a"]))


if __name__ == "__main__":
    unittest.main()
