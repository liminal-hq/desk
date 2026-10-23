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
                    self.assertIn(b["name"], ("explainer", "figure", "facts", "table"))
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


if __name__ == "__main__":
    unittest.main()
