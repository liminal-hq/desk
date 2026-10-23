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
import charts  # noqa: E402
import explainers as ex  # noqa: E402
import explainers_hw as hw  # noqa: E402
import figures  # noqa: E402
import gather  # noqa: E402
import manual  # noqa: E402
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

    def test_blobs_are_read_in_one_batch(self):
        with tempfile.TemporaryDirectory() as d:
            make_repo(d)
            repo = gather.Repo(d, "main")
            got = repo.blobs(["a.txt", "docs/x.md", "nope.txt"])
            self.assertEqual(got["a.txt"], b"one\n")
            self.assertEqual(got["docs/x.md"], b"x\n")
            self.assertIsNone(got["nope.txt"])
            self.assertEqual(repo.lines(r"\.txt$"), 3)

    def test_a_document_that_no_longer_yields_its_number_is_reported_missing(self):
        with tempfile.TemporaryDirectory() as d:
            make_repo(d)
            data = gather.gather(d, "main", offline=True)
            self.assertIn("emojiPixel", data["missing"])
            self.assertEqual(data["prs"][0]["mergeFull"], git(d, "rev-parse", "main").strip())

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


class RobustnessTests(unittest.TestCase):
    def test_link_targets_may_contain_parentheses(self):
        out = mdlite.inline("[Foo](https://en.wikipedia.org/wiki/Foo_(bar)) then [x](https://e.com)", mdlite.Ctx())
        self.assertIn('href="https://en.wikipedia.org/wiki/Foo_(bar)"', out)
        self.assertIn('href="https://e.com"', out)

    def test_data_text_is_neutralised(self):
        self.assertEqual(mdlite.inline(mdlite.neutral("a {{x}} `b` *c* [d](e)"), mdlite.Ctx()).count("<"), 0)

    def test_times_follow_the_real_clock_change(self):
        if not hasattr(ex.LOCAL, "key"):
            self.skipTest("no tz database on this machine")
        self.assertEqual(ex.hhmm("2026-10-22T20:59:50Z"), "16:59")
        self.assertEqual(ex.hhmm("2026-11-02T12:00:00Z"), "07:00")

    def test_roadmap_is_validated(self):
        base = [{"id": "desk", "branch": "root", "state": "progress", "deps": []}]
        with self.assertRaises(ValueError):
            ex.tree_layout({"nodes": base + [{"id": "a", "branch": "b", "state": "done", "deps": ["nope"]}]})
        with self.assertRaises(ValueError):
            ex.tree_layout({"nodes": base + [{"id": "a", "branch": "b", "state": "done", "deps": ["b"]}, {"id": "b", "branch": "b", "state": "done", "deps": ["a"]}]})
        with self.assertRaises(ValueError):
            ex.tree_layout({"nodes": base + [{"id": "a", "branch": "b", "state": "bogus", "deps": []}]})

    def test_merge_day_rejects_an_unknown_pull_request(self):
        with self.assertRaises(ValueError):
            ex.mergeday({"prs": [], "sync": []}, mdlite.Ctx(), [99])

    def test_build_does_not_clear_a_directory_it_did_not_make(self):
        with tempfile.TemporaryDirectory() as d:
            wr(os.path.join(d, "precious.txt"), "keep me\n")
            chapters = build.load_chapters(os.path.join(HERE, "fixture", "chapters"))
            with tempfile.TemporaryDirectory() as r:
                make_repo(r)
                data = gather.gather(r, "main", offline=True)
            site = build.Site(data, chapters, os.path.join(HERE, "fixture", "pages"), d, assets=False)
            with self.assertRaises(ValueError):
                site.write()
            self.assertTrue(os.path.exists(os.path.join(d, "precious.txt")))

    def test_command_lines_fail_cleanly(self):
        self.assertEqual(build.main(["build.py", "--out"]), 2)
        self.assertEqual(gather.main(["gather.py", "--ref"]), 2)
        with tempfile.TemporaryDirectory() as d:
            wr(os.path.join(d, "00-bad.md"), "---\nid: bad\ntitle: T\nsummary: S\nprs: [x]\n---\ntext\n")
            self.assertEqual(build.main(["build.py", "--chapters", d, "--out", os.path.join(d, "out"), "--data", os.path.join(TOOL, "snapshots", "gh.json")]), 1)


REAL = os.path.join(TOOL, "chapters")


class RealContentTests(unittest.TestCase):
    def test_real_chapters_are_well_formed(self):
        chapters = build.load_chapters(REAL, manual.MANUAL_DIR)
        ids = [c["id"] for c in chapters]
        self.assertGreaterEqual(len(chapters), 14)
        for c in chapters:
            if c["section"] == "manual":
                continue  # the manual has no shelves and its own template; RealManualTests checks it
            self.assertRegex(c["date"], r"^\d{4}-\d{2}-\d{2}$", c["id"])
            shelves = {mdlite.shelf_of(b) for b in c["blocks"]} - {None}
            self.assertEqual(shelves, {"catalogued", "uncatalogued", "wish"}, c["id"] + " needs all three shelves")
            self.assertTrue(any(b["type"] == "quote" and b["lines"][0].startswith("**From the stacks") for b in c["blocks"]) or c["section"] == "reference", c["id"])
            for b in c["blocks"]:
                if b["type"] == "directive":
                    self.assertIn(b["name"], ("explainer", "figure", "facts", "table"))
            text = rd(os.path.join(REAL, c["file"]))
            for target in re.findall(r"\]\(chapter:([\w-]+)", text):
                self.assertIn(target, ids, "%s links to a missing chapter" % c["id"])

    def test_sidebars_name_a_source_or_hedge(self):
        for c in build.load_chapters(REAL, manual.MANUAL_DIR):
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

    def test_expected_pattern_matcher_follows_native_check(self):
        self.assertTrue(ex.expected_matches("a {N:1..32}\n...\nb {HEX}\n{ANY}", ["a 5", "x", "b 0x1f", "anything"]))
        self.assertFalse(ex.expected_matches("a\nb", ["b", "a"]))
        self.assertFalse(ex.expected_matches("usable {N:14000..16384} KiB", ["usable 5 KiB"]), "the range must be honoured")
        self.assertFalse(ex.expected_matches("at {HEX}", ["at 1f"]), "{HEX} needs the 0x prefix")
        self.assertFalse(ex.expected_matches("a\nb", ["a", "extra", "b"]), "lines may only be skipped at '...'")


def broken_site(figs_block, extra=""):
    """A one-chapter site with the given figures block, built offline from a throwaway repository; returns (site dir, problems)."""
    out = tempfile.mkdtemp()
    ch = os.path.join(out, "chapters")
    os.makedirs(ch)
    wr(os.path.join(ch, "00-one.md"), "---\nid: one\nn: 1\ntitle: One\ndate: 2026-10-23\nsummary: S\nprs: []\n---\nText.\n\n<!-- figure arch -->\n%s\n### Catalogued\n- a\n\n### Uncatalogued\n- b\n\n### Wish list\n- c\n\n```figures\n%s\n```\n" % (extra, figs_block))
    return out, ch


GOOD = "id: arch-test\nkind: diagram\ntrack: shared\ntitle: The architecture\nalt: Layered diagram of the hosted and native tracks from apps to the machine.\ncaption: Layers.\nat: figure arch\nthumb: svg"


class FigureTests(unittest.TestCase):
    def build(self, figs, extra="", assets=False):
        out, ch = broken_site(figs, extra)
        with tempfile.TemporaryDirectory() as d:
            make_repo(d)
            data = gather.gather(d, "main", offline=True)
        site_dir = os.path.join(out, "site")
        chapters = build.load_chapters(ch)
        site = build.Site(data, chapters, os.path.join(HERE, "fixture", "pages"), site_dir, assets=assets)
        site.write()
        return site_dir, build.check_site(site_dir, [c["id"] for c in chapters])

    def test_a_registered_figure_gets_an_anchor_a_tile_and_a_working_link(self):
        d, problems = self.build(GOOD)
        self.assertEqual(problems, [])
        gallery = rd(os.path.join(d, "gallery.html"))
        self.assertIn('href="one.html#fig-arch-test"', gallery)
        self.assertIn("The architecture", gallery)
        self.assertIn('id="fig-arch-test"', rd(os.path.join(d, "one.html")))
        self.assertIn("Recent figures", rd(os.path.join(d, "index.html")))
        manifest = json.loads(rd(os.path.join(d, "manifest.json")))
        self.assertIn("gallery.html", manifest["publish"]["files"])

    def test_no_registry_means_no_gallery(self):
        out, ch = broken_site("")
        wr(os.path.join(ch, "00-one.md"), rd(os.path.join(ch, "00-one.md")).replace("<!-- figure arch -->", ""))
        with tempfile.TemporaryDirectory() as d:
            make_repo(d)
            data = gather.gather(d, "main", offline=True)
        site = build.Site(data, build.load_chapters(ch), os.path.join(HERE, "fixture", "pages"), os.path.join(out, "site"), assets=False)
        site.write()
        self.assertFalse(os.path.exists(os.path.join(out, "site", "gallery.html")))

    def test_a_figure_without_alt_text_fails_the_build(self):
        for bad in (GOOD.replace("alt: Layered diagram of the hosted and native tracks from apps to the machine.\n", ""), GOOD.replace("Layered diagram of the hosted and native tracks from apps to the machine.", "arch.png")):
            with self.assertRaises(mdlite.MdError):
                self.build(bad)

    def test_a_record_must_be_complete_and_well_formed(self):
        for bad in (GOOD.replace("kind: diagram", "kind: poster"), GOOD.replace("track: shared", "track: moon"), GOOD.replace("thumb: svg", "thumb: jpeg"), GOOD.replace("title: The architecture\n", ""),
                    GOOD.replace("state", "state") + "\nstate: sleepy", GOOD.replace("id: arch-test", "id: Arch Test")):
            with self.assertRaises(mdlite.MdError, msg=bad):
                self.build(bad)

    def test_a_record_that_names_a_directive_the_chapter_lacks_fails(self):
        with self.assertRaises(mdlite.MdError):
            self.build(GOOD.replace("at: figure arch", "at: figure tree"))

    def test_a_drawing_that_no_record_claims_fails(self):
        with self.assertRaises(mdlite.MdError):
            self.build(GOOD, extra="\n<!-- figure tree -->\n")

    def test_duplicate_ids_and_unknown_chapters_fail(self):
        with self.assertRaises(mdlite.MdError):
            self.build(GOOD + "\n\n" + GOOD)
        rec = figures.parse_block(GOOD, "nowhere")
        with self.assertRaises(mdlite.MdError):
            figures.validate(rec, [{"id": "one"}])

    def test_the_checker_catches_a_registry_entry_whose_anchor_or_image_is_missing(self):
        d, problems = self.build(GOOD.replace("at: figure arch\nthumb: svg", "at: figure arch\nanchor: nope\nthumb: svg"))
        self.assertTrue(any("anchor #nope" in p for p in problems), problems)
        d, problems = self.build(GOOD.replace("thumb: svg", "thumb: img:img/none.png"))
        self.assertTrue(any("none.png" in p for p in problems), problems)

    def test_previews_are_made_from_the_rendered_figure(self):
        html = '<figure><svg viewBox="0 0 2 2" id="a"><g><svg id="b"></svg></g><path d="M0,0" marker-end="url(#a)"/></svg></figure>'
        self.assertEqual(len(figures.svgs_of(html)), 1)
        pv = figures.preview({"thumb": "svg", "id": "x", "alt": "A", "title": "T"}, html, "g1-", {})
        self.assertIn('id="g1-a"', pv)
        self.assertIn("url(#g1-a)", pv)
        self.assertIn('aria-hidden="true"', pv)
        card = figures.steps_card("T", '<h4 class="x-title"><button>One</button></h4><h4 class="x-title">Two</h4>')
        self.assertIn("2 steps", card)

    def test_the_real_registry_is_complete_and_resolves(self):
        chapters = build.load_chapters(REAL)
        recs = [r for c in chapters for r in c["figs"]]
        self.assertGreaterEqual(len(recs), 30)
        reg = figures.validate(recs, chapters)
        claimed = {r.get("at") for r in reg}
        for r in reg:
            ch = [c for c in chapters if c["id"] == r["chapter"]][0]
            keys = [figures.directive_key(b["name"], b["args"]) for b in ch["blocks"] if b["type"] == "directive"]
            self.assertIn(r["at"], keys, "%s: %s" % (r["chapter"], r["id"]))
            self.assertGreater(len(r["alt"].split()), 5, r["id"])
        for c in chapters:
            for key in figures.unregistered(c["blocks"], [r for r in reg if r["chapter"] == c["id"]], c["id"]):
                self.assertIn(key, claimed, "%s draws %s and nothing claims it" % (c["id"], key))
        for r in reg:
            if r["thumb"].startswith("img:"):
                name = r["thumb"][4:]
                self.assertTrue(name.startswith(("img/img-", "shots/", "img/pixel-", "img/qemu")), name)
                if name.startswith("img/img-"):
                    self.assertTrue(os.path.exists(os.path.join(TOOL, "media", name[4:])), name)


class OpenWorkTests(unittest.TestCase):
    def make(self, d):
        make_repo(d)
        git(d, "checkout", "-q", "-b", "feat/p", "main")
        wr(os.path.join(d, "p.txt"), "p\n")
        git(d, "add", "p.txt")
        git(d, "commit", "-q", "-m", "feat: p")
        git(d, "checkout", "-q", "-b", "feat/q", "feat/p")
        wr(os.path.join(d, "hal.rs"), "#[test]\nfn a() {}\n")
        os.makedirs(os.path.join(d, "hal"))
        wr(os.path.join(d, "hal", "x.rs"), "#[test]\nfn a() {}\n#[test]\nfn b() {}\n")
        git(d, "add", "hal/x.rs", "hal.rs")
        git(d, "commit", "-q", "-m", "feat: q")
        git(d, "checkout", "-q", "-b", "feat/r", "main")
        git(d, "merge", "-q", "--no-ff", "-m", "merge p", "feat/p")
        wr(os.path.join(d, "r.txt"), "r\n")
        git(d, "add", "r.txt")
        git(d, "commit", "-q", "-m", "feat: r")
        git(d, "checkout", "-q", "main")

    def prs(self, d):
        def pr(n, b, base, draft=False, ok=True):
            return {"number": n, "title": "T%d" % n, "headRefName": b, "baseRefName": base, "headRefOid": git(d, "rev-parse", b).strip(), "state": "OPEN", "isDraft": draft,
                    "createdAt": "2026-10-23T00:00:00Z", "labels": [], "additions": 1, "deletions": 0, "changedFiles": 1, "url": "u", "checks": [{"status": "COMPLETED", "conclusion": "SUCCESS" if ok else "FAILURE"}]}
        return [pr(2, "feat/p", "main"), pr(3, "feat/q", "feat/p", ok=False), pr(4, "feat/r", "main", draft=True)]

    def test_stacked_branches_are_not_credited_with_their_parents_work(self):
        with tempfile.TemporaryDirectory() as d:
            self.make(d)
            w = gather.open_work(d, self.prs(d))
            by = {p["number"]: p for p in w["prs"]}
            self.assertEqual(by[2]["commitCount"], 1)
            self.assertEqual(by[3]["commitCount"], 1)
            self.assertEqual(by[4]["commitCount"], 2, "r's own commit and the merge that brought p in, but not p's commit")
            self.assertEqual([c["merge"] for c in by[4]["commits"]], [True, False])
            self.assertEqual(by[4]["stackedOn"], [2])
            self.assertEqual(by[3]["stackedOn"], [2])
            self.assertEqual((by[2]["state"], by[4]["state"]), ("in review", "in progress"))
            self.assertEqual((by[2]["ci"], by[3]["ci"]), ("passing", "failing"))
            self.assertEqual(w["commitsOnBranches"], 4)
            self.assertEqual(w["widest"], 3)
            self.assertEqual([t["hostTests"] for t in w["tips"] if t["number"] == 3], [2])

    def test_a_missing_branch_means_use_the_snapshot(self):
        with tempfile.TemporaryDirectory() as d:
            self.make(d)
            prs = self.prs(d)
            prs[0]["headRefName"] = "feat/never-fetched"
            self.assertIsNone(gather.open_work(d, prs))

    def test_a_moved_branch_is_reported(self):
        with tempfile.TemporaryDirectory() as d:
            self.make(d)
            prs = self.prs(d)
            prs[0]["headRefOid"] = "0" * 40
            self.assertTrue(gather.open_work(d, prs)["warnings"])

    def test_trial_merges_find_the_conflict_between_independent_branches(self):
        with tempfile.TemporaryDirectory() as d:
            make_repo(d)
            for name in ("p1", "p2"):
                git(d, "checkout", "-q", "-b", "feat/" + name, "main")
                wr(os.path.join(d, "shared.txt"), name + "\n")
                git(d, "add", "shared.txt")
                git(d, "commit", "-q", "-m", "feat: " + name)
            git(d, "checkout", "-q", "main")
            out = gather.trial_merges(d, {2: "feat/p1", 3: "feat/p2"})
            self.assertEqual(out, [{"a": 2, "b": 3, "files": ["shared.txt"]}])
            git(d, "checkout", "-q", "-b", "feat/z", "feat/p1")
            wr(os.path.join(d, "z.txt"), "z\n")
            git(d, "add", "z.txt")
            git(d, "commit", "-q", "-m", "feat: z")
            self.assertEqual(gather.trial_merges(d, {2: "feat/p1", 4: "feat/z"}), [], "a branch that contains the other is skipped")

    def test_pr_state_and_ci(self):
        self.assertEqual(gather.pr_state({"mergedAt": "x"}), "merged")
        self.assertEqual(gather.pr_state({"state": "OPEN", "isDraft": True}), "in progress")
        self.assertEqual(gather.pr_state({"state": "CLOSED"}), "closed")
        self.assertEqual(gather.ci_of([]), "none")
        self.assertEqual(gather.ci_of([{"status": "IN_PROGRESS"}]), "pending")

    def test_roadmap_uses_open_pull_requests_and_tags(self):
        road = {"branches": [], "nodes": [
            {"id": "a", "branch": "b", "state": "done-emu", "prs": [5], "deps": []},
            {"id": "b", "branch": "b", "state": "review", "prs": [5, 1], "deps": []},
            {"id": "c", "branch": "b", "state": "planned", "work": "in progress", "deps": []},
            {"id": "d", "branch": "b", "state": "planned", "deps": []},
            {"id": "e", "branch": "b", "state": "done", "deps": []},
            {"id": "f", "branch": "b", "state": "review", "prs": [77], "deps": []}]}
        _, warnings = gather.resolve_roadmap(road, [{"number": 1, "ci": "passing"}], [{"number": 5, "state": "in review", "ci": "passing"}])
        n = {x["id"]: x for x in road["nodes"]}
        self.assertEqual((n["a"]["state"], n["a"]["tag"]), ("review", "in review"))
        self.assertEqual((n["b"]["state"], n["b"]["tag"]), ("review", "in review"))
        self.assertEqual(n["c"]["tag"], "in progress")
        self.assertEqual(n["d"]["tag"], "planned")
        self.assertEqual(n["e"]["tag"], "merged")
        self.assertEqual(n["f"]["state"], "progress")
        self.assertTrue(any("node a drops to review" in w for w in warnings))

    def test_captures_compare_with_their_branch(self):
        text = "intro\n\n```text\nfirst\n```\n\n## Results\n\nx\n\n```text\none\ntwo\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n"
        self.assertEqual(gather.extract(text, {"after": "## Results", "fence": 1}), "one\ntwo\n")
        self.assertEqual(gather.extract(text, {"after": "## Results", "table": 1}), "| a | b |\n|---|---|\n| 1 | 2 |\n")
        self.assertIsNone(gather.extract(text, {"after": "## Nowhere", "fence": 1}))
        self.assertEqual(gather.extract(text, None), text)


class HardwareExplainerTests(unittest.TestCase):
    D = {"captures": []}

    def test_every_hardware_explainer_builds_from_the_captures(self):
        ctx = mdlite.Ctx()
        counts = {"pit": (hw.pit_test, 9), "input": (hw.input_replay, 19), "vga13": (hw.vga13, 10), "vbe": (hw.vbe_attrs, 3), "pipelines": (hw.pipelines, 7), "memchain": (hw.memchain, 5), "biscall": (hw.bios_roundtrip, 14)}
        for name, (fn, n) in counts.items():
            html = fn(self.D, ctx)
            self.assertEqual(html.count('class="x-step"'), n, name)
            self.assertIn('data-n="%d"' % n, html)
        self.assertIn("std, desk", hw.std_sizes(self.D, ctx))
        self.assertIn("video block", hw.bootinfo(self.D, ctx).lower())
        self.assertIn("<svg", hw.hal_layers(self.D, ctx))
        self.assertIn("microseconds", hw.bios_cost(self.D, ctx))

    def with_media(self, name, old, new):
        real = hw.media

        def fake(n):
            t = real(n)
            return t.replace(old, new) if n == name else t
        hw.media = fake
        self.addCleanup(setattr, hw, "media", real)

    def test_the_input_replay_refuses_a_transcript_that_the_derived_bytes_do_not_reproduce(self):
        self.with_media("input-expected.txt", "key: A down mods=shift char='A'", "key: A down mods=shift char='a'")
        with self.assertRaises(ValueError):
            hw.input_replay(self.D, mdlite.Ctx())

    def test_the_pit_test_refuses_a_script_that_the_arithmetic_does_not_give(self):
        self.with_media("hal-pit.rs.txt", "Access::outb(0x40, 0x9C)", "Access::outb(0x40, 0x9D)")
        with self.assertRaises(ValueError):
            hw.pit_test(self.D, mdlite.Ctx())

    def test_the_mode_13h_walk_refuses_a_table_of_the_wrong_length(self):
        self.with_media("vga-regs.rs.txt", "0x5F, 0x4F, 0x50, 0x82, 0x54, 0x80, 0xBF, 0x1F, 0x00, 0x41,", "0x5F, 0x4F, 0x50, 0x82, 0x54, 0x80, 0xBF, 0x1F, 0x00,")
        with self.assertRaises(ValueError):
            hw.vga13(self.D, mdlite.Ctx())

    def test_the_loader_explainer_refuses_a_changed_fallback_order(self):
        self.with_media("g-stage2.asm.txt", "call map_e801\n        jnc .map_ok", "call map_88\n        jnc .map_ok")
        with self.assertRaises(ValueError):
            hw.memchain(self.D, mdlite.Ctx())

    def test_the_round_trip_refuses_a_trip_that_does_not_put_the_pics_back(self):
        self.with_media("n3-switch.md", "Puts the PICs back", "Leaves the PICs")
        with self.assertRaises(ValueError):
            hw.bios_roundtrip(self.D, mdlite.Ctx())

    def test_a_link_into_the_source_document_keeps_its_words(self):
        self.with_media("n3-switch.md", "[below](#x)", "[below](#x)")
        html = hw.captable("n3-switch.md", mdlite.Ctx(), "t")
        self.assertNotIn('href="#', html)

    def test_the_decoder_replica_and_packets(self):
        t = hw.kbd_tables()
        self.assertEqual(t["Delete"], [(True, 0x53)])
        ev = hw.decode_replica([0x2A, 0x1E, 0x9E, 0xAA], t)
        self.assertEqual([hw.event_line(e) for e in ev][1], "key: A down mods=shift char='A'")
        p = hw.mouse_packet(-20, 7, 0)
        self.assertEqual(hw.packet_decode(p), (-20, 7, 0))
        self.assertEqual(p, [0x38, 0xEC, 0xF9, 0])

    def test_charts_draw_from_the_data(self):
        data = json.loads(rd(os.path.join(TOOL, "snapshots", "gh.json")))["data"]
        D = {"fonts": [{"file": "desk-sans.otf", "glyphs": 423, "size": 1}], "emoji": {"pixel": 480, "pixel12": 480, "pixel8": 40, "vector": 156, "requested": 244, "requestedDone": 164, "requestedPlanned": 80},
             "roadmap": json.loads(rd(os.path.join(TOOL, "roadmap.json"))), "open": data["openWork"]}
        for fn in (charts.font_matrix, charts.emoji_coverage, charts.track_progress, charts.pr_stack):
            self.assertIn("<svg", fn(D))


class FirstWindowTests(unittest.TestCase):
    """The first window's explainer and the anatomy diagrams are read from files, and refuse data that disagrees with them."""

    def test_the_anatomy_files_are_valid(self):
        import explainers_fw as fw
        for name in ("dos", "desk"):
            self.assertEqual(fw.validate_anatomy(fw.load_anatomy(name)), [], name)

    def test_every_desk_box_has_one_of_three_statuses_and_a_built_box_cites_evidence(self):
        import explainers_fw as fw
        data = fw.load_anatomy("desk")
        self.assertEqual(data["statuses"], ["verified", "unverified", "planned"])
        for b in data["boxes"]:
            self.assertIn(b["status"], data["statuses"])
            if b["status"] != "planned":
                self.assertTrue(b["evidence"], b["id"])
        planned = {b["id"] for b in data["boxes"] if b["status"] == "planned"}
        for need in ("appmodel", "syscalls", "loader", "winown"):
            self.assertIn(need, planned, "the app model, system calls, loader and window ownership must stay planned until built")

    def test_a_bad_box_is_refused(self):
        import copy
        import explainers_fw as fw
        data = fw.load_anatomy("desk")
        bad = copy.deepcopy(data)
        bad["boxes"][6]["status"] = "done"
        self.assertTrue(fw.validate_anatomy(bad))
        bad = copy.deepcopy(data)
        bad["boxes"][6]["evidence"] = []
        self.assertTrue(any("evidence" in p for p in fw.validate_anatomy(bad)))
        bad = copy.deepcopy(data)
        bad["boxes"][6]["evidence"] = ["repo:docs/NO-SUCH-FILE.md"]
        self.assertTrue(any("does not exist" in p for p in fw.validate_anatomy(bad)))
        bad = copy.deepcopy(data)
        bad["boxes"][7]["col"] = bad["boxes"][6]["col"]
        self.assertTrue(any("overlap" in p for p in fw.validate_anatomy(bad)))

    def test_the_frames_explainer_has_six_steps_with_the_digests_of_the_sums(self):
        import explainers_fw as fw
        html = fw.frames({}, mdlite.Ctx())
        self.assertEqual(html.count('class="x-step"'), 6)
        for f, (d8, d4) in fw.result_table().items():
            self.assertIn(d8, html)
            self.assertIn(d4, html)

    def test_the_frames_explainer_refuses_a_digest_that_disagrees_with_the_result_table(self):
        import explainers_fw as fw
        real = fw.result_table
        try:
            fw.result_table = lambda: {k: ("00000000", v[1]) for k, v in real().items()}
            with self.assertRaises(ValueError):
                fw.frames({}, mdlite.Ctx())
        finally:
            fw.result_table = real


FIXTURE_MANUAL = os.path.join(HERE, "fixture", "manual")
SHALLOW = manual.is_shallow(gather.ROOT)  # the CI checks out one commit: no history, so the whole site cannot be built there
FAKE_DATA = {"prs": [{"number": 1, "ci": "passing"}, {"number": 3, "ci": "failing"}], "open": {"prs": [{"number": 4, "state": "in review", "ci": "passing"}]}}
BOXES = {"desk:kernel", "dos:main"}


def manual_copy(mutate=lambda t: t, plan_mutate=lambda p: p):
    """The fixture manual copied to a temporary directory with its chapter text (and plan) changed; returns (directory, plan, chapters)."""
    d = tempfile.mkdtemp()
    plan = plan_mutate(json.loads(rd(os.path.join(FIXTURE_MANUAL, "plan.json"))))
    wr(os.path.join(d, "plan.json"), json.dumps(plan))
    wr(os.path.join(d, "M01-one.md"), mutate(rd(os.path.join(FIXTURE_MANUAL, "M01-one.md"))))
    return d, plan, manual.load_chapters(plan, d)


def manual_problems(mutate=lambda t: t, data=FAKE_DATA):
    d, plan, chapters = manual_copy(mutate)
    return manual.check_all(chapters, plan, gather.ROOT, data, ["one", "two"], BOXES)


class ManualCheckTests(unittest.TestCase):
    """Each rule of the manual's checks, with a twin that breaks it and must be refused for the stated reason."""

    def assertRefused(self, mutate, needle, data=FAKE_DATA):
        problems = manual_problems(mutate, data)
        self.assertTrue(any(needle in p for p in problems), "%r not in %s" % (needle, problems))

    def test_the_fixture_chapter_is_clean(self):
        self.assertEqual(manual_problems(), [])

    @unittest.skipIf(SHALLOW, "a shallow clone cannot tell a missing commit from one it does not have")
    def test_front_matter_and_sections_are_required(self):
        self.assertRefused(lambda t: t.replace("reviewed: 44f0265\n", ""), "reviewed must be a commit hash")
        self.assertRefused(lambda t: t.replace("reviewed: 44f0265", "reviewed: zzzz"), "reviewed must be a commit hash")
        self.assertRefused(lambda t: t.replace("reviewed: 44f0265", "reviewed: " + "1" * 40), "not a commit of this repository")
        self.assertRefused(lambda t: t.replace("log: [one]\n", ""), "front matter has no log")
        self.assertRefused(lambda t: t.replace("## Worked example", "## Another"), "no section 'Worked example'")
        self.assertRefused(lambda t: t.replace("watches: [tools/progress-report/manual.py]", "watches: [no/such/dir]"), "watches no/such/dir")

    def test_a_pointer_to_a_missing_path_or_symbol_fails(self):
        self.assertRefused(lambda t: t.replace("(repo:tools/progress-report/manual.py)", "(repo:tools/progress-report/gone.py)"), "gone.py does not exist")
        self.assertRefused(lambda t: t.replace("manual.py::check_chapter)", "manual.py::check_chapter_renamed)", 1), "check_chapter_renamed")
        self.assertRefused(lambda t: t.replace("## The code\n\n| File | What it holds |\n|---|---|\n| [`manual.py`](repo:tools/progress-report/manual.py) | the checks |\n| [`check_chapter`](sym:tools/progress-report/manual.py::check_chapter) | one chapter's problems |\n", "## The code\n\nNo table.\n"), "The code needs at least one row")

    def test_log_and_anatomy_references_must_resolve(self):
        self.assertRefused(lambda t: t.replace("log: [one]", "log: [nowhere]"), "not a log chapter")
        self.assertRefused(lambda t: t.replace("anatomy: [desk:kernel]", "anatomy: [desk:nothing]"), "not a box")

    def test_state_rules(self):
        self.assertRefused(lambda t: t.replace("state: built-verified\nevidence: pr:1, test", "state: shipped\nevidence: pr:1, test"), "is not one of")
        self.assertRefused(lambda t: t.replace("evidence: pr:1, test:tools/progress-report/tests/test_report.py::test_fixture_manual_site", "evidence: repo:AGENTS.md"), "built-verified needs run evidence")
        self.assertRefused(lambda t: t.replace("evidence: pr:1, repo:AGENTS.md", "evidence: "), "needs evidence")
        self.assertRefused(lambda t: t.replace("evidence: doc:docs/MANUAL-PLAN.md#the-chapter-template", "evidence: repo:AGENTS.md"), "must cite where it is planned")
        self.assertRefused(lambda t: t.replace("doc:docs/MANUAL-PLAN.md#the-chapter-template", "doc:docs/MANUAL-PLAN.md#no-such-heading"), "no heading with that anchor")
        self.assertRefused(lambda t: t.replace("test_fixture_manual_site", "test_zz" + "qq_absent"), "does not appear")
        self.assertRefused(lambda t: t.replace("pr:1, repo:AGENTS.md", "pr:99, repo:AGENTS.md"), "not among the pull requests")
        self.assertRefused(lambda t: t.replace("id: fix.unrun", "id: fix.heap"), "duplicate id")

    def test_a_verified_claim_without_a_passing_merged_pull_request_is_shown_as_unverified(self):
        d, plan, chapters = manual_copy()
        recs = {r["id"]: r for r in chapters[0]["status"]}
        self.assertEqual(manual.effective_state(recs["fix.heap"], FAKE_DATA)[0], "built-verified")
        st, why = manual.effective_state(recs["fix.unrun"], {"prs": [{"number": 1, "ci": "failing"}], "open": {"prs": []}})
        self.assertEqual(st, "built-unverified")
        self.assertIn("#1 is not merged with CI passing", why)
        st, why = manual.effective_state(recs["fix.unrun"], FAKE_DATA)
        self.assertEqual(st, "built-verified", "a merged pull request with CI passing is run evidence")
        recs["fix.heap"]["items"] = [("pr", "4")]
        self.assertEqual(manual.effective_state(recs["fix.heap"], FAKE_DATA)[0], "built-unverified", "an open pull request is not run evidence")

    def test_a_symbol_must_appear_as_a_whole_word_and_a_repo_link_may_carry_a_fragment(self):
        self.assertIsNone(manual.check_item("sym", "tools/progress-report/manual.py::check_chapter", gather.ROOT, FAKE_DATA))
        self.assertIsNotNone(manual.check_item("sym", "tools/progress-report/manual.py::check_chapte", gather.ROOT, FAKE_DATA), "a prefix of a name is not the name")
        self.assertIsNone(manual.check_item("repo", "docs/MANUAL-PLAN.md#build-notes", gather.ROOT, FAKE_DATA))
        self.assertIsNotNone(manual.check_item("repo", "docs/NO-SUCH.md#x", gather.ROOT, FAKE_DATA))
        self.assertRefused(lambda t: t.replace("term: Segment", "term: ++"), "no letters or digits")

    def test_try_commands_must_exist(self):
        self.assertRefused(lambda t: t.replace("$ python3 -I tools/progress-report/tests/test_report.py", "$ python3 tools/no-such.py"), "the script tools/no-such.py does not exist")
        self.assertRefused(lambda t: t.replace("$ python3 -I tools/progress-report/tests/test_report.py", "$ tools/nothing.sh"), "no such script")
        self.assertRefused(lambda t: t.replace("$ python3 -I tools/progress-report/tests/test_report.py", "$ frobnicate --now"), "not a script in the repository or a known tool")
        self.assertRefused(lambda t: t.replace("needs: host: python3\n", ""), "must say what it needs")
        self.assertEqual(manual.check_command("(cd hal && cargo test --locked --quiet)", gather.ROOT), [])
        self.assertEqual(manual.check_command("tools/native-test.sh | tail -3", gather.ROOT), [])
        self.assertTrue(manual.check_command("(cd nowhere && cargo test)", gather.ROOT))
        self.assertEqual(manual.check_command("grep -c \"a|b\" README.md", gather.ROOT), [])
        self.assertEqual(manual.check_command("python3 -m unittest discover", gather.ROOT), [])

    def test_glossary_terms_are_defined_once_and_used(self):
        self.assertRefused(lambda t: t.replace("[segment](term:segment)", "segment"), "defined and never used")
        self.assertRefused(lambda t: t.replace("[segment](term:segment)", "[offset](term:offset)"), "no chapter defines")
        self.assertRefused(lambda t: t.replace("def: A made-up term for the fixture.\n```", "def: A made-up term for the fixture.\n\nterm: Segment\ndef: Again.\n```"), "defined in m-one and in m-one")

    def test_sidebar_rules_apply_to_the_manual(self):
        self.assertRefused(lambda t: t.replace("> Source: [example](https://example.com)\n", ""), "no Source line")
        self.assertRefused(lambda t: t.replace("A sidebar of two sentences. It is only here to be rendered.", "One. Two. Three. Four. Five. Six."), "longer than five sentences")

    def test_the_generated_boxes_are_required(self):
        self.assertRefused(lambda t: t.replace("<!-- manual next -->", ""), "no <!-- manual next -->")

    def test_placeholders_use_the_same_tag_as_the_table(self):
        d, plan, chapters = manual_copy()
        vals = manual.state_values(chapters, FAKE_DATA)
        self.assertEqual(str(vals["state.fix.heap"]), manual.tag_html("built-verified", ""))
        ctx = mdlite.Ctx(vals)
        self.assertIn("mt-verified", mdlite.inline("It is {{state.fix.heap}}.", ctx))
        with self.assertRaises(mdlite.MdError):
            mdlite.inline("{{state.fix.nothing}}", ctx)

    def test_a_pull_request_that_cannot_be_looked_up_is_accepted_only_in_a_shallow_clone(self):
        shallow = dict(FAKE_DATA, prs=[], shallow=True)
        self.assertEqual(manual_problems(data=shallow), [])
        d, plan, chapters = manual_copy()
        rec = [r for r in chapters[0]["status"] if r["id"] == "fix.unrun"][0]
        st, why = manual.effective_state(rec, shallow)
        self.assertEqual(st, "built-unverified")
        self.assertIn("not in this clone", why)
        self.assertTrue(any("not among the pull requests" in p for p in manual_problems(data=dict(FAKE_DATA, prs=[]))))

    def test_the_plan_is_checked(self):
        def plan_problems(f):
            p = json.loads(rd(os.path.join(FIXTURE_MANUAL, "plan.json")))
            f(p)
            return manual.validate_plan(p)
        self.assertEqual(plan_problems(lambda p: None), [])
        self.assertTrue(plan_problems(lambda p: p["chapters"].append(dict(p["chapters"][0]))))
        self.assertTrue(any("exactly once" in x for x in plan_problems(lambda p: p["paths"].__setitem__("learn", ["m-one"]))))
        self.assertTrue(any("exactly once" in x for x in plan_problems(lambda p: p["paths"].__setitem__("find", ["m-one", "m-one", "m-two"]))))
        self.assertTrue(any("unknown chapter" in x for x in plan_problems(lambda p: p["tasks"][0]["chapters"].append("m-nope"))))
        self.assertTrue(any("tools/missing.sh" in x for x in plan_problems(lambda p: p["tasks"][0]["checks"].append("tools/missing.sh"))))
        d = tempfile.mkdtemp()
        plan = json.loads(rd(os.path.join(FIXTURE_MANUAL, "plan.json")))
        plan["chapters"] = plan["chapters"][1:]
        plan["paths"] = {"learn": ["m-two"], "find": ["m-two"]}
        plan["tasks"] = []
        wr(os.path.join(d, "M01-one.md"), rd(os.path.join(FIXTURE_MANUAL, "M01-one.md")))
        with self.assertRaises(mdlite.MdError):
            manual.load_chapters(plan, d)

    def test_staleness_reads_the_history_under_the_watched_paths(self):
        with tempfile.TemporaryDirectory() as d:
            make_repo(d)
            first = git(d, "rev-list", "--max-parents=0", "HEAD").strip()
            c = {"reviewed": first, "watches": ["docs/"]}
            self.assertEqual(manual.staleness(c, d)["state"], "stale")
            self.assertEqual(manual.staleness(dict(c, watches=["nothing/"]), d)["state"], "fresh")
            self.assertEqual(manual.staleness(dict(c, reviewed="1" * 40), d)["state"], "unknown")
            sub = git(d, "log", "--format=%s", first + "..HEAD", "--", "docs/").strip()
            self.assertIn("docs: add x", sub)


class ManualSiteTests(unittest.TestCase):
    """A fixture build with manual chapters: the contents section, the header link, the pager, the backlinks, the reused figures."""

    def build_site(self, mutate=lambda t: t, two=False, chapters_dir=None):
        d, plan, _ = manual_copy(mutate)
        if two:
            text = rd(os.path.join(d, "M01-one.md"))
            text = text.replace("id: m-one", "id: m-two").replace("n: M1", "n: M2").replace("The first fixture chapter", "The second fixture chapter").replace("fix.", "two.")
            text = text.replace("[segment](term:segment)", "segment").replace("```glossary\nterm: Segment\ndef: A made-up term for the fixture.\n```\n\n", "").replace("anatomy: [desk:kernel]", "anatomy: [dos:main]")
            wr(os.path.join(d, "M02-two.md"), text)
        with tempfile.TemporaryDirectory() as r:
            make_repo(r)
            data = gather.gather(r, "main", offline=True)
        chapters = build.load_chapters(chapters_dir or os.path.join(HERE, "fixture", "chapters"), d)
        out = os.path.join(tempfile.mkdtemp(), "site")
        site = build.Site(data, chapters, os.path.join(HERE, "fixture", "pages"), out, assets=False, manual_dir=d)
        site.write()
        return out, chapters, site

    def test_fixture_manual_site(self):
        out, chapters, site = self.build_site()
        self.assertEqual(build.check_site(out, [c["id"] for c in chapters]), [])
        index = rd(os.path.join(out, "index.html"))
        self.assertIn('<h2 id="h-manual">The manual</h2>', index)
        self.assertIn('href="manual.html">Manual</a>', index)
        self.assertIn("1 of 2 chapters are written", index)
        self.assertIn("The second fixture chapter", index)
        self.assertIn("pill-coming", index)
        landing = rd(os.path.join(out, "manual.html"))
        for needle in ("Learn it", "Find it", "The task index", "Run the tests", 'id="row-m-two"', "Preface, from the archivist"):
            self.assertIn(needle, landing)
        page = rd(os.path.join(out, "m-one.html"))
        for needle in ("Manual · M1 · Part I. Foundations", "mt-verified", "mt-unverified", "mt-planned", "Shown as built, not verified", "m-needs", "Terms this chapter defines", "From the stacks",
                       "On the anatomy pages", "Where next", 'href="manual.html#row-m-two"', "pill-coming", "Tasks that lead here", 'id="state-fix.heap"'):
            self.assertIn(needle, page, needle)
        self.assertNotIn("Catalogued", page, "manual pages have no shelves")
        self.assertEqual(site.shelf_counts, {"catalogued": 1, "uncatalogued": 1, "wish": 1}, "the manual's pages add nothing to the shelf counts")
        for f in ("m-glossary.html", "m-maps.html", "m-commands.html", "m-sources.html"):
            self.assertIn("<h1", rd(os.path.join(out, f)))
        self.assertIn('id="term-segment"', rd(os.path.join(out, "m-glossary.html")))
        self.assertIn("python3 -I tools/progress-report/tests/test_report.py", rd(os.path.join(out, "m-commands.html")))
        self.assertIn("https://example.com", rd(os.path.join(out, "m-sources.html")))
        manifest = json.loads(rd(os.path.join(out, "manifest.json")))
        for f in ("manual.html", "m-one.html", "m-glossary.html"):
            self.assertIn(f, manifest["publish"]["files"])

    def test_the_log_page_gains_an_aside_and_nothing_else_changes(self):
        out, chapters, site = self.build_site()
        one = rd(os.path.join(out, "one.html"))
        self.assertIn('<h2 id="in-the-manual">In the manual</h2>', one)
        self.assertIn('href="m-one.html"', one)
        self.assertNotIn("In the manual", rd(os.path.join(out, "two.html")), "a log chapter no manual chapter names has no aside")
        plain = os.path.join(tempfile.mkdtemp(), "plain")
        build.Site(site.data, build.load_chapters(os.path.join(HERE, "fixture", "chapters")), os.path.join(HERE, "fixture", "pages"), plain, assets=False).write()
        strip = lambda t: t.replace('<a href="manual.html">Manual</a>', "")
        self.assertEqual(strip(rd(os.path.join(out, "two.html"))), rd(os.path.join(plain, "two.html")), "no other log page changes")

    def test_the_manual_pager_follows_the_learn_path(self):
        out, chapters, site = self.build_site(two=True)
        self.assertEqual(build.check_site(out, [c["id"] for c in chapters]), [])
        first, second = rd(os.path.join(out, "m-one.html")), rd(os.path.join(out, "m-two.html"))
        self.assertIn('class="next" href="m-two.html"', first)
        self.assertIn('class="prev" href="m-one.html"', second)
        self.assertIn("1 of 2" if False else "2 of 2 chapters are written", rd(os.path.join(out, "index.html")))
        self.assertNotIn("pill-coming", rd(os.path.join(out, "index.html")).split('id="h-manual"')[1].split("</section>")[0])

    def test_the_anatomy_tables_gain_a_manual_column_from_the_same_lists(self):
        import explainers_fw as fw
        out, chapters, site = self.build_site(two=True)
        html = fw.anatomy("desk", site.ctx, "lead")
        self.assertIn('<th scope="col">Manual</th>', html)
        self.assertIn('id="box-desk-kernel"', html)
        self.assertIn('href="m-one.html"', html)
        self.assertNotIn('id="box-dos-main"', html)
        plain = fw.anatomy("desk", mdlite.Ctx(), "lead")
        self.assertNotIn(">Manual<", plain)
        self.assertNotIn("box-desk", plain)
        dos = fw.anatomy("dos", site.ctx, "lead")
        self.assertIn('id="box-dos-main"', dos)
        self.assertIn('href="m-two.html"', dos)

    def test_a_figure_the_log_declares_can_be_reused_and_keeps_one_tile(self):
        out, ch = broken_site(GOOD)
        manual_extra = lambda t: t.replace("## Worked example\n", "## Worked example\n\n<!-- figure arch -->\n") + "\n```figures\nid: m1-arch\nreuse: arch-test\nat: figure arch\n```\n"
        site_dir, chapters, site = self.build_site(manual_extra, chapters_dir=ch)
        self.assertEqual(build.check_site(site_dir, [c["id"] for c in chapters]), [])
        gallery = rd(os.path.join(site_dir, "gallery.html"))
        self.assertEqual(gallery.count('class="tile"'), 1, "a reused figure gets no second tile")
        self.assertIn('id="fig-m1-arch"', rd(os.path.join(site_dir, "m-one.html")))

    def test_a_manual_figure_of_its_own_gets_a_gallery_tile_with_a_preview_and_the_group_filter(self):
        out, ch = broken_site(GOOD)
        own = lambda t: t.replace("## Worked example\n", "## Worked example\n\n<!-- figure arch -->\n") + (
            "\n```figures\nid: m1-own\nkind: diagram\ntrack: native\ntitle: The manual's own diagram\nalt: A layered diagram drawn for the manual from the architecture data.\ncaption: Drawn for the manual.\nat: figure arch\nthumb: svg\n```\n")
        site_dir, chapters, site = self.build_site(own, chapters_dir=ch)
        self.assertEqual(build.check_site(site_dir, [c["id"] for c in chapters]), [])
        gallery = rd(os.path.join(site_dir, "gallery.html"))
        self.assertEqual(gallery.count('<li class="tile"'), 2)
        self.assertIn('data-section="manual"', gallery)
        self.assertIn('id="section-manual"', gallery)
        self.assertIn("Manual M1", gallery)
        self.assertIn("<svg", gallery.split('data-section="manual"')[1].split("</li>")[0], "the preview is drawn from the rendered figure")

    def test_a_reuse_that_names_nothing_or_draws_something_else_is_refused(self):
        out, ch = broken_site(GOOD)
        base = lambda rid, at: (lambda t: t.replace("## Worked example\n", "## Worked example\n\n<!-- figure arch -->\n") + "\n```figures\nid: m1-arch\nreuse: %s\nat: %s\n```\n" % (rid, at))
        with self.assertRaises(mdlite.MdError):
            self.build_site(base("no-such-figure", "figure arch"), chapters_dir=ch)
        with self.assertRaises(mdlite.MdError):
            self.build_site(base("arch-test", "figure tree"), chapters_dir=ch)

    def test_a_manual_chapter_cannot_reuse_an_id_of_the_log(self):
        d, plan, _ = manual_copy()
        wr(os.path.join(d, "M01-one.md"), rd(os.path.join(d, "M01-one.md")).replace("id: m-one", "id: one"))
        plan["chapters"][0]["id"] = "one"
        wr(os.path.join(d, "plan.json"), json.dumps(plan))
        with self.assertRaises(mdlite.MdError):
            build.load_chapters(os.path.join(HERE, "fixture", "chapters"), d)

    def test_a_broken_pointer_breaks_the_build(self):
        with self.assertRaises(mdlite.MdError):
            self.build_site(lambda t: t.replace("(repo:tools/progress-report/manual.py)", "(repo:tools/progress-report/gone.py)"))

    def test_manual_explainers_are_discovered_and_validate_their_own_data(self):
        d = tempfile.mkdtemp()
        wr(os.path.join(d, "demo.py"), "ID = 'demo'\n\n\ndef render(D, ctx):\n    return '<p>demo</p>'\n")
        self.assertEqual(sorted(manual.explainers(d)), ["demo"])
        wr(os.path.join(d, "bad.py"), "def render(D, ctx):\n    return ''\n")
        with self.assertRaises(mdlite.MdError):
            manual.explainers(d)
        wr(os.path.join(d, "bad.py"), "ID = 'demo'\n\n\ndef render(D, ctx):\n    return ''\n")
        with self.assertRaises(mdlite.MdError):
            manual.explainers(d)

    def test_the_real_plan_is_valid_and_the_sidebars_follow_the_log_rules(self):
        plan = manual.load_plan()
        self.assertEqual(len(plan["chapters"]), 22)
        self.assertEqual(len(plan["appendices"]), 4)
        self.assertEqual(manual.validate_plan(plan), [])


class RealManualTests(unittest.TestCase):
    """The written chapters of the manual against the repository: every pointer, command and state is checked as the build checks it."""

    @classmethod
    def setUpClass(cls):
        cls.plan = manual.load_plan()
        cls.chapters = manual.load_chapters(cls.plan)
        cls.data = gather.gather(gather.ROOT, "HEAD", offline=True)
        cls.log_ids = [c["id"] for c in build.load_chapters(REAL)]
        cls.boxes = {"%s:%s" % (n, b["id"]) for n in ("desk", "dos") for b in __import__("explainers_fw").load_anatomy(n)["boxes"]}

    def test_every_written_chapter_passes_the_checks(self):
        self.assertEqual(manual.check_all(self.chapters, self.plan, gather.ROOT, self.data, self.log_ids, self.boxes), [])

    def test_both_paths_list_every_chapter_once_and_no_id_is_shared_with_the_log(self):
        for which in ("learn", "find"):
            self.assertEqual(sorted(self.plan["paths"][which]), sorted(c["id"] for c in self.plan["chapters"]))
        ids = [c["id"] for c in self.plan["chapters"]] + [a["id"] for a in self.plan["appendices"]]
        self.assertFalse(set(ids) & set(self.log_ids))

    @unittest.skipIf(SHALLOW, "the front page needs the merge history, which a shallow clone does not have")
    def test_the_real_manual_builds_into_a_site_that_checks(self):
        chapters = build.load_chapters(REAL, manual.MANUAL_DIR)
        with tempfile.TemporaryDirectory() as out:
            site = build.Site(self.data, chapters, os.path.join(TOOL, "pages"), os.path.join(out, "site"), assets=True, manual_dir=manual.MANUAL_DIR)
            site.write()
            self.assertEqual(build.check_site(os.path.join(out, "site"), [c["id"] for c in chapters]), [])
            index = rd(os.path.join(out, "site", "index.html"))
            section = index.split('id="h-manual"')[1].split("</section>")[0]
            self.assertEqual(section.count("pill-coming"), len(self.plan["chapters"]) - len(self.chapters))


def explainer_module(name):
    import importlib.util
    spec = importlib.util.spec_from_file_location("t_" + name, os.path.join(manual.MANUAL_DIR, "explainers", name + ".py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class BootChaptersTests(unittest.TestCase):
    """The loader and boot chapters (M4, M5) and their explainers against the code and the committed transcripts."""

    @classmethod
    def setUpClass(cls):
        cls.chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}

    def text(self, cid):
        return self.chapters[cid]["raw"]

    def test_the_boot_explainer_is_reused_once_between_the_two_chapters(self):
        names = [" ".join([d["name"]] + d["args"]) for cid in ("m-loader", "m-boot") for d in self.chapters[cid]["blocks"] if d["type"] == "directive"]
        self.assertEqual(names.count("explainer boot"), 1)

    def test_m5_walks_every_line_of_the_recorded_transcript(self):
        serial = rd(os.path.join(TOOL, "media", "native-serial.txt")).splitlines()
        text = self.text("m-boot")
        missing = [l for l in serial if l.strip() and l not in text]
        self.assertEqual(missing, [])

    def test_m5_lists_every_test_mode_with_the_number_and_name_of_the_code(self):
        main = rd(os.path.join(gather.ROOT, "kernel", "src", "main.rs"))
        enum = re.findall(r"^\s+(\w+) = (\d+),$", main[main.index("enum TestMode"):main.index("impl TestMode")], flags=re.M)
        image = rd(os.path.join(gather.ROOT, "tools", "native-image.sh"))
        names = {int(n): m for m, n in re.findall(r"([a-z0-9-]+)\) code=(\d+)", image)}
        rows = [r for r in re.findall(r"^\| (\d+) \| `([a-z0-9-]+)` \| `(\w+)` \|", self.text("m-boot"), flags=re.M)]
        self.assertEqual([(int(n), v) for v, n in enum], [(int(n), v) for n, _, v in rows])
        for n, name, variant in rows:
            self.assertEqual(names[int(n)], name, variant)

    def test_m5_exit_statuses_are_those_of_the_run_script(self):
        run = rd(os.path.join(gather.ROOT, "tools", "native-run.sh"))
        exit_rs = rd(os.path.join(gather.ROOT, "kernel", "src", "exit.rs"))
        for status in ("33", "35", "124"):
            self.assertIn(status, run)
            self.assertIn(status, self.text("m-boot"))
        self.assertIn("0x10", exit_rs)
        self.assertIn("0x11", exit_rs)
        self.assertRegex(self.text("m-boot"), r"\(0x10 << 1\) \| 1 = 33")

    def test_m4_numbers_are_the_ones_the_sources_give(self):
        image = explainer_module("image")
        c = image.constants()
        self.assertEqual((c["STAGE2_SECTORS"], c["KERNEL_LBA"], c["TOTAL_SECTORS"]), (17, 18, 2880))
        kbytes, ksec = image.kernel_sectors(c)
        reads = image.reads(c["KERNEL_LBA"], ksec, c["KERNEL_SEG"], c)
        self.assertIn("%d calls of INT 13h" % len(reads), self.text("m-loader"))
        self.assertEqual(len(reads), 21)
        self.assertIn("%d reads" % (len(reads) + 1), self.text("m-loader"))
        self.assertIn("%d sectors" % ksec, self.text("m-loader"))
        self.assertEqual(639 + 15232, 15871)
        self.assertIn("15871 KiB", self.text("m-loader"))

    def test_the_image_explainer_refuses_overlaps_and_boundary_crossings(self):
        image = explainer_module("image")
        c = image.constants()
        kbytes, ksec = image.kernel_sectors(c)
        r2 = image.reads(c["STAGE2_LBA"], c["STAGE2_SECTORS"], c["STAGE2_SEG"], c)
        rk = image.reads(c["KERNEL_LBA"], ksec, c["KERNEL_SEG"], c)
        self.assertEqual(image.check(c, r2, rk, ksec), [])
        self.assertTrue(any("overlaps" in p for p in image.check(dict(c, KERNEL_LBA=10), r2, rk, ksec)))
        self.assertTrue(any("too large" in p or "more than" in p for p in image.check(c, r2, rk, c["KERNEL_LIMIT"] + 1)))
        bad = [dict(rk[0], start=0x1F000, end=0x21000)] + rk[1:]
        self.assertTrue(any("crosses a 64 KiB boundary" in p for p in image.check(c, r2, bad, ksec)))
        self.assertTrue(any("past the end" in p for p in image.check(dict(c, TOTAL_SECTORS=300), r2, rk, ksec)))

    def test_the_negative_checks_explainer_refuses_a_mode_the_test_does_not_run(self):
        neg = explainer_module("negatives")
        main, image, test, run, enum, codes = neg.load()
        self.assertEqual(neg.check(main, image, test, run, enum, codes), [])
        self.assertTrue(any("does not run the negative mode div0" in p for p in neg.check(main, image, test.replace("div0", "d1v0"), run, enum, codes)))
        self.assertTrue(any("TestMode::DivideError" in p for p in neg.check(main, image, test, run, {k: v for k, v in enum.items() if k != "DivideError"}, codes)))
        self.assertTrue(any("no test mode accounts for" in p for p in neg.check(main, image, test, run, enum, codes, listing=["expected-div0.txt", "expected-mystery.txt"])))
        bad = list(neg.NEGATIVES)
        bad[0] = bad[0][:3] + (["EXCEPTION 0x00: no such line"],) + bad[0][4:]
        self.assertTrue(any("has no line" in p for p in neg.check(main, image, test, run, enum, codes, negatives=bad)))


def load_chapter_tests():
    """Each manual chapter may keep its own checks in tests/test_chapter_NAME.py (self-contained, a plain unittest module); their test
    cases are picked up here so that the one command, `python3 -I tools/progress-report/tests/test_report.py`, runs them all."""
    import glob
    import importlib.util
    for path in sorted(glob.glob(os.path.join(HERE, "test_chapter_*.py"))):
        name = os.path.basename(path)[:-3]
        spec = importlib.util.spec_from_file_location(name, path)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        for attr, obj in vars(mod).items():
            if isinstance(obj, type) and issubclass(obj, unittest.TestCase) and obj.__module__ == name:
                globals()[name + "_" + attr] = obj


load_chapter_tests()


class MapAndProcessorChaptersTests(unittest.TestCase):
    """The map (M1) against the anatomy file, and the processor chapter (M2) against the loader's and the kernel's own values."""

    RANK = {"planned": 0, "unverified": 1, "built-unverified": 1, "verified": 2, "built-verified": 2}

    @classmethod
    def setUpClass(cls):
        cls.chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        cls.boxes = json.loads(rd(os.path.join(TOOL, "anatomy-desk.json")))["boxes"]

    @classmethod
    def coverage(cls, records, boxes):
        """What is wrong between the map's status records and the anatomy boxes: a missing record, or one claiming more than its box."""
        by_id = {r["id"]: r for r in records}
        problems = []
        for b in boxes:
            r = by_id.get("map." + b["id"])
            if r is None:
                problems.append("no status record for the box %s" % b["id"])
            elif cls.RANK[r["state"]] > cls.RANK[b["status"]]:
                problems.append("the record for %s claims %s, above the box's %s" % (b["id"], r["state"], b["status"]))
            elif (b["status"] == "planned") != (r["state"] == "planned"):
                problems.append("the record for %s and its box disagree about being planned" % b["id"])
        return problems

    def test_m1_has_a_record_for_every_box_no_higher_than_the_box(self):
        records = self.chapters["m-map"]["status"]
        self.assertEqual(self.coverage(records, self.boxes), [])
        self.assertEqual(len(self.boxes), len([r for r in records if r["id"].startswith("map.")]))

    def test_m1_coverage_check_refuses_a_missing_box_and_an_inflated_state(self):
        recs = self.chapters["m-map"]["status"]
        self.assertTrue(any("no status record for the box wm" in p for p in self.coverage([r for r in recs if r["id"] != "map.wm"], self.boxes)))
        inflated = [dict(r, state="built-verified") if r["id"] == "map.live" else r for r in recs]
        self.assertTrue(any("claims built-verified, above the box's unverified" in p for p in self.coverage(inflated, self.boxes)))

    def test_m1_declares_the_three_reused_figures(self):
        text = self.chapters["m-map"]["raw"]
        for fid, at in (("architecture", "figure arch"), ("anatomy-desk", "figure anatomy-desk"), ("anatomy-dos", "figure anatomy-dos")):
            self.assertIn("reuse: %s\nat: %s" % (fid, at), text)

    def test_m1_counts_of_workspace_members_are_the_manifests(self):
        hal = rd(os.path.join(gather.ROOT, "hal", "Cargo.toml"))
        shell = rd(os.path.join(gather.ROOT, "shell", "Cargo.toml"))
        self.assertEqual(len(re.findall(r'"[^"]+"', re.search(r"members = \[(.*?)\]", hal).group(1))), 9)
        self.assertEqual(len(re.findall(r'"[^"]+"', re.search(r"members = \[(.*?)\]", shell).group(1))), 6)

    def test_m2_defines_the_terms_it_promises(self):
        terms = {t["id"] for t in self.chapters["m-cpu"]["glossary"]}
        for t in ("segment", "offset", "linear-address", "a20", "descriptor", "selector", "gdt", "cr0", "protected-mode", "real-mode", "ring"):
            self.assertIn(t, terms)

    def test_m2_worked_example_uses_the_loaders_own_a20_addresses(self):
        asm = rd(os.path.join(gather.ROOT, "boot", "stage2.asm"))
        self.assertIn("mov si, 0x0500", asm)
        self.assertIn("mov di, 0x0510", asm)
        self.assertIn("not ax", asm)
        self.assertEqual((0x0000 * 16 + 0x0500, 0xFFFF * 16 + 0x0510), (0x00500, 0x100500))
        self.assertEqual(0x100500 & ~0x100000, 0x00500)
        text = self.chapters["m-cpu"]["raw"]
        for needle in ("`0000:0500`", "`FFFF:0510`", "`0x00500`", "`0x100500`"):
            self.assertIn(needle, text)

    def test_m2_decodes_the_descriptors_the_loader_and_the_kernel_hold(self):
        asm = rd(os.path.join(gather.ROOT, "boot", "stage2.asm"))
        gdt_rs = rd(os.path.join(gather.ROOT, "kernel", "src", "gdt.rs"))
        text = self.chapters["m-cpu"]["raw"]
        for d in ("00CF9A000000FFFF", "00CF92000000FFFF"):
            self.assertIn("0x" + d, asm)
            self.assertIn("0x" + "_".join(d[i:i + 4] for i in range(0, 16, 4)), gdt_rs)
            self.assertIn("0x" + d, text)
        code = int("00CF9A000000FFFF", 16).to_bytes(8, "little")
        self.assertEqual(code.hex(" ").upper(), "FF FF 00 00 00 9A CF 00")
        self.assertIn("`FF FF 00 00 00 9A CF 00`", text)
        self.assertEqual((0xFFFFF + 1) * 4096, 4 << 30)
        self.assertEqual(3 * 8 - 1, 0x17)

    def test_m2_cr0_value_is_the_sum_of_the_bits_it_names(self):
        self.assertEqual(1 + 2 + 16 + 32, 0x33)
        for name in ("expected-div0.txt", "expected-gp.txt"):
            self.assertIn("0x00000033", rd(os.path.join(gather.ROOT, "tests", "native", name)))
        fpu = rd(os.path.join(gather.ROOT, "kernel", "src", "fpu.rs"))
        self.assertIn("(1u32 << 1) | (1 << 5)", fpu)
        self.assertIn("!((1u32 << 2) | (1 << 3))", fpu)

    def test_m2_idt_error_code_mask_lists_the_vectors_it_names(self):
        mask = 0x60227D00
        self.assertEqual([v for v in range(32) if mask >> v & 1], [8, 10, 11, 12, 13, 14, 17, 21, 29, 30])
        self.assertIn("0x60227D00", rd(os.path.join(gather.ROOT, "kernel", "src", "idt.rs")))
        self.assertIn("0x60227D00", self.chapters["m-cpu"]["raw"])

    def test_the_boot_chapters_link_the_processor_terms(self):
        self.assertIn("](term:gdt)", self.chapters["m-loader"]["raw"])
        self.assertIn("](term:a20)", self.chapters["m-boot"]["raw"])


if __name__ == "__main__":
    unittest.main()
