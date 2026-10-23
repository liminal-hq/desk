#!/usr/bin/env python3
# Builds the Desk progress report as a multi-page static site: an index, a page per chapter, shared CSS and JS, and a manifest.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 tools/progress-report/build.py [--data data.json] [--out dist/progress-report] [--chapters DIR] [--pages DIR]
#                                               [--no-assets] [--check]
#
# Standard library only. Without --data the data is gathered from the repository first (see gather.py). The chapters are
# markdown files with a small front matter (chapters/NN-id.md); the front page text is pages/front.md. --check validates what was
# written: balanced tags, unique ids, internal links and anchors that resolve, every chapter reachable from the index.
import hashlib
import html.parser
import json
import os
import re
import shutil
import sys

USAGE = "usage: build.py [--data FILE] [--out DIR] [--chapters DIR] [--pages DIR] [--no-assets] [--check]"
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import arch  # noqa: E402
import explainers as ex  # noqa: E402
import charts  # noqa: E402
import explainers_hw as hw  # noqa: E402
import figures  # noqa: E402
import gather  # noqa: E402
import mdlite  # noqa: E402
from mdlite import Ctx, esc, inline  # noqa: E402

ARTIFACT_URL = "https://claude.ai/artifact/UiyueA93UpACK4UnMMh5as"
TITLE = "Desk Progress Report"
DESCRIPTION = ("The archive of the Desk project, chapter by chapter and figure by figure: what is catalogued, what is built but unverified, and what is only a wish, "
               "with the evidence and a state tag from gh for each. A work in progress. There is no operating system yet.")
FONTS = ["desk-title", "desk-ui", "desk-small", "desk-fixed", "desk-sans", "desk-sans-bold", "desk-sans-italic", "desk-serif",
         "desk-serif-italic", "desk-mono", "desk-mono-bold"]
SHOTS = [("01-v4-800.jpg", "01-v4-800.jpg"), ("02-v4-16.jpg", "02-v4-16.jpg"), ("v4-menu.jpg", "v4-menu.jpg"), ("files-resize.png", "files-resize.jpg")]
HEADLINE = ["prs", "ciGreen", "commitsAll", "files", "emojiPixel", "emojiVector", "bunTests", "transcripts", "s1Scenarios", "kernelLines", "rtLines", "docLines"]
HEADLINE_OPEN = ["prsOpen", "openCi", "commitsBranches", "hostTests", "branchTranscripts", "halLines"]
TAGS = {"merged": "merged", "in review": "review", "in progress": "progress", "planned": "planned", "closed": "closed"}
SHELF_TEXT = {
    "catalogued": ("Catalogued", "Checked, and the entry says how: a merged pull request, a passing CI run, a byte-for-byte transcript, a measured number."),
    "uncatalogued": ("Uncatalogued", "Built, but not verified: it has not run on real hardware, or in an emulator other than the one used, or no one has looked."),
    "wish": ("Wish list", "Planned or only an idea. Nothing here is built."),
}


# ---- reading ------------------------------------------------------------------------------------------------------------------

def read_text(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


def load_chapters(directory):
    chapters = []
    for name in sorted(os.listdir(directory)):
        if not name.endswith(".md"):
            continue
        meta, body = mdlite.front_matter(read_text(os.path.join(directory, name)), name)
        for key in ("id", "title", "summary"):
            if not meta.get(key):
                raise mdlite.MdError("%s: front matter needs %s" % (name, key))
        meta["file"] = name
        meta["blocks"], meta["figs"] = figures.take_blocks(mdlite.parse_blocks(body), meta["id"])
        meta.setdefault("n", "")
        meta.setdefault("section", "journey")
        try:
            meta["prs"] = [int(x) for x in meta.get("prs", [])]
        except ValueError:
            raise mdlite.MdError("%s: prs must be pull request numbers" % name)
        chapters.append(meta)
    ids = [c["id"] for c in chapters]
    if len(set(ids)) != len(ids):
        raise mdlite.MdError("duplicate chapter ids")
    return chapters


def num(v):
    return "{:,}".format(v) if isinstance(v, int) else str(v)


def coming_rows(data):
    """The chapters that are expected and not written: each names a branch, and the state comes from the pull request on that branch
    if there is one (from gh), else 'in progress' as the owner reported it."""
    path = os.path.join(HERE, "pages", "coming.json")
    rows = gather.read_json(path) if os.path.exists(path) else []
    by_branch = {o["branch"]: o for o in data["open"]["prs"]}
    for p in data["prs"]:
        if p.get("branch"):
            by_branch.setdefault(p["branch"], {"number": p["number"], "state": "merged", "head": p["mergeCommit"], "ci": p["ci"]})
    out = []
    for r in rows:
        o = by_branch.get(r["branch"])
        out.append(dict(r, pr=o["number"] if o else None, state=o["state"] if o else "in progress", ci=o["ci"] if o else None))
    return out


def tag(state):
    return '<span class="pill pill-%s">%s</span>' % (TAGS.get(state, "planned"), esc(state))


def values_of(data):
    vals = {"m." + k: m["value"] for k, m in data["metrics"].items() if m["value"] is not None}
    for p in data["prs"]:
        n = p["number"]
        vals.update({"pr%d.added" % n: p["added"], "pr%d.commits" % n: p["commitCount"], "pr%d.merge" % n: p["mergeCommit"], "pr%d.time" % n: ex.hhmm(p["mergedAt"])})
    for f in data["fonts"]:
        stem = f["file"].rsplit(".", 1)[0].replace("-", "_")
        if f["file"].endswith(".ttf"):
            vals["kern." + stem] = f.get("kernPairs", 0)
            vals["glyphs." + stem] = f.get("glyphs", 0)
    vals["emoji.requested"] = data["emoji"]["requested"] or 0
    vals["emoji.requestedDone"] = data["emoji"]["requestedDone"] or 0
    vals["emoji.requestedPlanned"] = data["emoji"]["requestedPlanned"] or 0
    vals["emoji.pixel12"] = data["emoji"]["pixel12"] or 0
    vals["emoji.pixel8"] = data["emoji"]["pixel8"] or 0
    for k, v in data["loc"].items():
        vals["loc." + k] = v
    vals["kernelBytes"] = data["measured"].get("nativeKernelBytes", 0)
    for k, v in data["docSizes"].items():
        vals["size." + re.sub(r"\W", "_", k)] = v
    for o in data["open"]["prs"]:
        n = o["number"]
        vals.update({"pr%d.added" % n: o["ghAdditions"] or 0, "pr%d.commits" % n: o["commitCount"], "pr%d.head" % n: o["head"], "pr%d.state" % n: o["state"],
                     "pr%d.ci" % n: o["ci"], "pr%d.ahead" % n: o["ahead"]})
    subject = [o for o in data["open"]["prs"] if not o["isReport"]]
    for t in data["open"].get("tips", []):
        vals["pr%d.hostTests" % t["number"]] = t["hostTests"]
    boot = [c for c in data["open"].get("conflicts", []) if "boot/stage2.asm" in c["files"]]
    vals["conflicts.boot"] = ", ".join("#%d with #%d" % (c["a"], c["b"]) for c in boot) or "no pair"
    vals["conflicts.pairs"] = len(data["open"].get("conflicts", []))
    vals["open.numbers"] = ", ".join("#%d" % o["number"] for o in subject)
    vals["open.count"] = len(subject)
    vals["merged.since"] = ", ".join("#%d" % n for n in data["mergeDay"]["since"]) or "nothing"
    vals["merged.sincecount"] = len(data["mergeDay"]["since"])
    vals["open.reports"] = ", ".join("#%d" % o["number"] for o in data["open"]["prs"] if o["isReport"])
    for c in coming_rows(data):
        vals["coming.%s.pr" % c["id"]] = ("#%d" % c["pr"]) if c.get("pr") else "no pull request yet"
        vals["coming.%s.state" % c["id"]] = c["state"]
    vals["head"] = data["head"] or ""
    vals["generated"] = data["generated"][:10]
    return vals


# ---- page shell ---------------------------------------------------------------------------------------------------------------

def fragment(title, body, description=DESCRIPTION, root=""):
    """The published index: the Artifact skeleton adds the doctype, head and body, so this carries only title, metas and content."""
    return ('<title>%s</title>\n<meta name="claude:viewport-fit-sides" content="cover">\n<meta name="description" content="%s">\n'
            '<link rel="stylesheet" href="%ssite.css">\n%s\n<script src="%ssite.js" defer></script>\n') % (esc(title), esc(description), root, body, root)


def document(title, body, description=DESCRIPTION):
    return ('<!doctype html>\n<html lang="en-CA">\n<head>\n<meta charset="utf-8">\n'
            '<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">\n'
            '<meta name="claude:viewport-fit-sides" content="cover">\n<title>%s</title>\n<meta name="description" content="%s">\n'
            '<link rel="stylesheet" href="site.css">\n</head>\n<body>\n%s\n<script src="site.js" defer></script>\n</body>\n</html>\n') % (esc(title), esc(description), body)


def site_header(current, ids):
    def link(i, href, text):
        if i != "index" and i not in ids:
            return ""
        return '<a href="%s"%s>%s</a>' % (href, ' aria-current="page"' if current == i else "", text)
    return ('<a class="skip" href="#main">Skip to the content</a>\n<header class="site"><div class="wrap site-in"><a class="brand" href="index.html">Desk Progress Report</a>'
            '<nav aria-label="Site">%s%s%s%s%s</nav></div></header>' % (link("index", "index.html", "Contents"), link("gallery", "gallery.html", "Gallery"), link("path", "path.html", "The path"),
                                                                   link("findings", "findings.html", "Findings"), link("archive", "archive.html", "How this is made")))


def site_footer(data, prev, nxt):
    nav = ""
    if prev or nxt:
        nav = '<nav class="pager" aria-label="Chapters">%s%s</nav>' % (
            ('<a class="prev" href="%s.html" rel="prev"><span>Previous</span><b>%s</b></a>' % (prev["id"], esc(label_of(prev)))) if prev else '<span></span>',
            ('<a class="next" href="%s.html" rel="next"><span>Next</span><b>%s</b></a>' % (nxt["id"], esc(label_of(nxt)))) if nxt else '<span></span>')
    src = data["sources"]
    return ('%s<footer class="site"><div class="wrap"><p>Generated %s from <code>%s</code> at <code>%s</code> by <code>tools/progress-report/</code>. Pull request and CI data: %s. '
            'The prose is the archivist\'s; the numbers are computed, and each says how.</p></div></footer>') % (
        nav, esc(data["generated"][:10]), esc(data["ref"]), esc(data["head"] or "?"),
        "read from <code>gh</code> now" if src["gh"] == "gh" else ("read from the committed snapshot of <code>gh</code>" if src["gh"] == "snapshot" else "not available"))


def label_of(c):
    return ("%s. %s" % (c["n"], c["title"])) if c.get("n") != "" else c["title"]


# ---- shared blocks ------------------------------------------------------------------------------------------------------------

def pr_records(data):
    """Every pull request the report knows, merged or open, by number, in one shape."""
    out = {}
    for p in data["prs"]:
        out[p["number"]] = {"number": p["number"], "title": p["title"], "state": "merged", "commit": p["mergeCommit"], "when": "%s %s" % (ex.day_label(p["mergedAt"]), ex.hhmm(p["mergedAt"])),
                            "commits": p["commitCount"], "added": p["added"], "ci": p["ci"], "url": p.get("url"), "stack": [], "base": p.get("base")}
    for o in data["open"]["prs"]:
        out[o["number"]] = {"number": o["number"], "title": o["title"], "state": o["state"], "commit": o["head"], "when": "opened " + ex.day_label(o["createdAt"]) if o.get("createdAt") else "",
                            "commits": o["commitCount"], "added": o["ghAdditions"] or 0, "ci": o["ci"], "url": o.get("url"), "stack": o["stackedOn"], "base": o["base"]}
    return out


def chapter_state(data, numbers):
    """merged when every pull request is merged, in progress when one is a draft, in review when one is open, else planned."""
    recs = pr_records(data)
    states = [recs[n]["state"] for n in numbers if n in recs]
    if not numbers or len(states) != len(numbers):
        return "planned"
    if all(x == "merged" for x in states):
        return "merged"
    return "in progress" if "in progress" in states else "in review"


def pr_table(data, numbers, ctx):
    rows = []
    recs = pr_records(data)
    any_open = False
    for n in numbers:
        p = recs.get(n)
        if not p:
            rows.append('<tr><td>#%d</td><td colspan="6">not found among the pull requests</td></tr>' % n)
            continue
        url = p.get("url") or ctx.href("pr:%d" % n)
        merged = p["state"] == "merged"
        any_open = any_open or not merged
        stack = ('<div class="sub">stacked on %s; the base branch is <code>%s</code></div>' % (", ".join("#%d" % k for k in p["stack"]), esc(p["base"]))) if p["stack"] else (
            '' if merged else '<div class="sub">based on <code>%s</code></div>' % esc(p["base"]))
        checks = ("CI passed on the merge commit" if p["ci"] == "passing" else "CI " + p["ci"]) if merged else (
            ("CI green on the head commit; not merged" if p["ci"] == "passing" else "CI %s on the head commit; not merged" % p["ci"]))
        rows.append('<tr><td><a href="%s" target="_blank" rel="noopener">#%d</a></td><td>%s%s</td><td>%s</td><td><code>%s</code><div class="sub">%s</div></td><td class="n">%d</td><td class="n">+%s%s</td><td>%s</td></tr>' % (
            esc(url), n, esc(p["title"]), stack, tag(p["state"]), p["commit"], esc(p["when"]), p["commits"], num(p["added"]), "" if merged else " †", esc(checks)))
    note = ('<p class="how">Merged: from <code>git log --first-parent</code> and <code>git diff --numstat</code> on the merge commit; CI from <code>gh run list</code> on that commit. Open: state, head commit and CI from <code>gh pr list</code> and the branch in <code>git</code>; the commit count leaves out commits that came from other open branches this one contains.%s</p>' % (
        " † Lines added are GitHub's diff against the pull request's base branch, which for a stacked pull request includes the work it is stacked on." if any_open else ""))
    return ('<div class="tbl"><table><caption class="sr">Pull requests behind this chapter</caption><thead><tr><th scope="col">PR</th><th scope="col">Title</th><th scope="col">State</th><th scope="col">Commit</th>'
            '<th scope="col" class="n">Commits</th><th scope="col" class="n">Lines added</th><th scope="col">Checks</th></tr></thead><tbody>%s</tbody></table></div>%s') % ("".join(rows), note)


def facts_strip(data, keys):
    cells = []
    for k in keys:
        m = data["metrics"][k]
        cells.append('<div class="fact"><b class="num">%s</b><span>%s</span><details><summary>how measured</summary><p>%s</p></details></div>' % (
            esc(num(m["value"]) if m["value"] is not None else "not available"), esc(m["label"]), inline(m["how"], Ctx())))
    return '<div class="facts">%s</div>' % "".join(cells)


def metric_tile(data, k):
    return facts_strip(data, [k]).replace('class="facts"', 'class="facts one"')


# ---- the site -----------------------------------------------------------------------------------------------------------------

class Site:
    def __init__(self, data, chapters, pages_dir, out, assets=True):
        self.data, self.chapters, self.pages_dir, self.out, self.assets = data, chapters, pages_dir, out, assets
        self.ids = [c["id"] for c in chapters]
        self.repo_url = ((data.get("repo") or {}).get("url") or "https://github.com/liminal-hq/desk")
        self.ctx = Ctx(values_of(data), self.ids, self.repo_url, data["prs"])
        self.shelf_counts = {"catalogued": 0, "uncatalogued": 0, "wish": 0}
        self.shelf_items = []
        self.pages = {}
        allrecs = [r for c in chapters for r in c["figs"]]
        for r in allrecs:
            r["anchor_given"] = bool(r.get("anchor"))
        self.registry = figures.validate(allrecs, chapters)
        claimed = {r.get("at") for r in self.registry}
        for c in chapters:
            for key in figures.unregistered(c["blocks"], [r for r in self.registry if r["chapter"] == c["id"]], c["id"]):
                if key not in claimed:
                    raise mdlite.MdError("%s: the directive %r draws something and no figures block claims it" % (c["id"], key))
        for r in self.registry:
            if r.get("at"):
                ch = [c for c in chapters if c["id"] == r["chapter"]][0]
                if r["at"] not in [figures.directive_key(b["name"], b["args"]) for b in ch["blocks"] if b["type"] == "directive"]:
                    raise mdlite.MdError("%s: figure %s names the directive %r, which the chapter does not contain" % (r["chapter"], r["id"], r["at"]))
        self.fig_at = {(r["chapter"], r["at"]): r for r in self.registry if r.get("at") and not r.get("anchor_given")}
        self.fig_html = {}
        self.thumbs = {}

    # explainer and figure registry
    def directive(self, chapter):
        d, ctx = self.data, self.ctx
        media = os.path.join(HERE, "media")

        def run(name, args):
            html = inner(name, args)
            rec = self.fig_at.get((chapter["id"], figures.directive_key(name, args)))
            if rec:
                self.fig_html[rec["id"]] = html
                return '<div class="figwrap" id="%s">%s</div>' % (rec["anchor"], html)
            for r in self.registry:
                if r["chapter"] == chapter["id"] and r.get("at") == figures.directive_key(name, args):
                    self.fig_html[r["id"]] = html
            return html

        def inner(name, args):
            if name == "explainer":
                xid = args[0]
                if xid == "boot":
                    return ex.boot(d, ctx, read_text(os.path.join(media, "native-serial.txt")), d["transcripts"]["nativeExpected"])
                if xid == "modules":
                    return ex.modules(d, ctx)
                if xid == "pipeline":
                    return ex.pipeline(d, ctx)
                if xid == "mergeday":
                    return ex.mergeday(d, ctx, [int(a) for a in args[1:]])
                hwx = {"pit": hw.pit_test, "input": hw.input_replay, "vga13": hw.vga13, "vbeattrs": hw.vbe_attrs, "pipelines": hw.pipelines, "memchain": hw.memchain, "biscall": hw.bios_roundtrip}
                if xid in hwx:
                    return hwx[xid](d, ctx)
                raise mdlite.MdError("unknown explainer %s in %s" % (xid, chapter["id"]))
            if name == "table":
                if len(args) != 1 or not os.path.exists(os.path.join(media, args[0])):
                    raise mdlite.MdError("unknown captured table %s in %s" % (args, chapter["id"]))
                return hw.captable(args[0], ctx, args[0]) + '<p class="how">%s</p>' % hw.cap_note(d, [args[0]])
            if name == "figure" and args[0] == "shots":
                return hw.shots(d, args[1:])
            if name == "figure" and args[0] in ("font-matrix", "emoji-coverage", "track-progress", "pr-stack"):
                return {"font-matrix": charts.font_matrix, "emoji-coverage": charts.emoji_coverage, "track-progress": charts.track_progress, "pr-stack": charts.pr_stack}[args[0]](d)
            if name == "figure" and args[0] == "open-conflicts":
                return charts.open_conflicts(d)
            if name == "figure" and args[0] == "bios-cost":
                return hw.bios_cost(d, ctx)
            if name == "figure" and args[0] in ("hal-layers", "bootinfo", "std-sizes"):
                return {"hal-layers": lambda: '<figure><div class="chart scroll" tabindex="0" aria-label="HAL layers diagram, scrolls sideways"><div style="min-width:760px">%s</div></div><figcaption>The layers and crate layout of the HAL design (PR #8) and of what the later pull requests added on top of it. Solid blue: built and in an open pull request, not merged. Dashed: designed or sketched only. Nothing here has run on real hardware, and the platforms are the only code that touches the machine.</figcaption></figure>' % hw.hal_layers(d, ctx),
                                                  "bootinfo": lambda: hw.bootinfo(d, ctx), "std-sizes": lambda: hw.std_sizes(d, ctx)}[args[0]]()
            if name == "figure":
                fid = args[0]
                if fid == "hero":
                    return '<figure class="banner" aria-label="The Desk banner">%s</figure>' % read_text(os.path.join(gather.ROOT, "assets", "hero.svg")) if os.path.exists(os.path.join(gather.ROOT, "assets", "hero.svg")) else ""
                table = {"arch": lambda: '<figure><div class="chart scroll" tabindex="0" aria-label="Architecture diagram, scrolls sideways"><div style="min-width:760px">%s</div></div><figcaption>Layers from <code>docs/DIRECTION.md</code>. Solid means merged and run in an emulator; it does not mean tried on a real machine.</figcaption></figure>' % arch.svg(),
                         "tree": lambda: ex.tree(d), "specimens": ex.specimens, "contact": ex.contact, "faces": lambda: ex.faces_figure(d),
                         "design": ex.design_shots, "nexts": lambda: ex.nexts(d), "qemu": ex.qemu_figure, "conflicts": lambda: ex.conflicts_table(d, ctx)}
                if fid not in table:
                    raise mdlite.MdError("unknown figure %s in %s" % (fid, chapter["id"]))
                return table[fid]()
            if name == "facts":
                for k in args:
                    if k not in d["metrics"]:
                        raise mdlite.MdError("unknown metric %s in %s" % (k, chapter["id"]))
                return facts_strip(d, args)
            raise mdlite.MdError("unknown directive %s in %s" % (name, chapter["id"]))
        return run

    def render_chapter(self, i):
        c = self.chapters[i]
        body, shelves = mdlite.render(c["blocks"], self.ctx, self.directive(c))
        for k, items in shelves.items():
            self.shelf_counts[k] += len(items)
            for it in items:
                self.shelf_items.append((k, c, it))
        prev = self.chapters[i - 1] if i > 0 else None
        nxt = self.chapters[i + 1] if i + 1 < len(self.chapters) else None
        eyebrow = []
        if c.get("n") != "":
            eyebrow.append("Chapter %s" % c["n"])
        elif c.get("kicker"):
            eyebrow.append(c["kicker"])
        if c.get("date"):
            eyebrow.append(self.date_label(c["date"]))
        if c["prs"]:
            eyebrow.append(", ".join("PR #%d" % n for n in c["prs"]))
        evidence = ""
        if c["prs"]:
            evidence = '<section class="evidence"><h2 id="the-pull-requests">The pull requests</h2>%s</section>' % pr_table(self.data, c["prs"], self.ctx)
        staterow = ""
        if c["prs"]:
            recs = pr_records(self.data)
            parts = []
            for n in c["prs"]:
                r = recs.get(n)
                if r:
                    parts.append("#%d %s%s" % (n, r["state"], "" if r["state"] == "merged" else (", CI " + ("green" if r["ci"] == "passing" else r["ci"]) + ", not merged yet")))
            staterow = '<p class="state-row">%s <span>%s</span></p>' % (tag(chapter_state(self.data, c["prs"])), esc("; ".join(parts)))
        main = ('<main id="main" class="wrap chapter"><article><header class="chead"><div class="eyebrow">%s</div><h1>%s</h1>%s<p class="lead">%s</p></header>'
                '<div class="prose">%s</div>%s</article></main>') % (esc(" · ".join(eyebrow)), esc(c["title"]), staterow, inline(c["summary"], self.ctx), body, evidence)
        title = "%s · %s" % (TITLE, c["title"])
        return c["id"] + ".html", document(title, site_header(c["id"] if c["id"] in ("path", "findings", "archive") else "", self.ids + self.extra_ids()) + main + site_footer(self.data, prev, nxt))

    @staticmethod
    def date_label(iso):
        import datetime
        d = datetime.date.fromisoformat(iso)
        return "%d %s %d" % (d.day, d.strftime("%B"), d.year)

    def extra_ids(self):
        return ["gallery"] if self.registry else []

    def fig_state(self, rec):
        if rec.get("state"):
            return rec["state"]
        ch = [c for c in self.chapters if c["id"] == rec["chapter"]][0]
        return chapter_state(self.data, ch["prs"]) if ch["prs"] else "merged"

    def make_thumbs(self):
        """Small copies of the screenshots the gallery shows, made with Pillow when it is installed; without it the tiles point at the
        full images, which the page scales. Returns ({site path: thumbnail path}, [files written])."""
        if not self.assets:
            return {}, []
        try:
            from PIL import Image
        except ImportError:
            return {}, []
        out, written = {}, []
        for src in sorted({r["thumb"][4:] for r in self.registry if r["thumb"].startswith("img:")}):
            fp = os.path.join(self.out, src)
            if not os.path.exists(fp):
                continue
            im = Image.open(fp)
            if im.width > 1000 and im.height > 1000:
                im = im.crop((0, 0, 560, 420))
            mode = im.mode
            im.thumbnail((480, 360), Image.LANCZOS)
            jpg = src.lower().endswith((".jpg", ".jpeg"))
            if mode == "P" and not jpg:
                im = im.convert("RGB").quantize(colors=64, method=Image.MEDIANCUT)
            name = "thumbs/%s.%s" % (os.path.splitext(os.path.basename(src))[0], "jpg" if jpg else "png")
            os.makedirs(os.path.join(self.out, "thumbs"), exist_ok=True)
            (im.convert("RGB") if jpg else im).save(os.path.join(self.out, name), **({"quality": 78, "optimize": True} if jpg else {"optimize": True}))
            if os.path.getsize(os.path.join(self.out, name)) >= os.path.getsize(fp):
                os.remove(os.path.join(self.out, name))
                continue
            out[src] = name
            written.append(name)
        return out, written

    def tile(self, rec, i):
        ch = [c for c in self.chapters if c["id"] == rec["chapter"]][0]
        state = self.fig_state(rec)
        chap = ("Chapter %s" % ch["n"]) if ch.get("n") != "" else ch["title"]
        pv = figures.preview(rec, self.fig_html.get(rec["id"], ""), "g%d-" % i, self.thumbs)
        alt = "" if rec["thumb"].startswith("img:") else '<span class="sr">%s</span>' % esc(rec["alt"])
        when = self.date_label(rec.get("date") or ch["date"])
        return ('<li class="tile" data-kind="%s" data-track="%s" data-i="%d" data-date="%s"><a class="tile-link" href="%s.html#%s">'
                '<div class="tile-pv">%s</div><div class="tile-body"><b class="tile-title">%s</b>%s<span class="tile-cap">%s</span>'
                '<span class="tile-meta"><span class="kind">%s</span><span>%s</span>%s<span>%s</span></span></div></a></li>') % (
            rec["kind"], rec["track"], i, esc(rec.get("date") or ch["date"]), rec["chapter"], esc(rec["anchor"]), pv, esc(rec["title"]), alt, esc(rec["caption"]),
            esc(rec["kind"]), esc(chap), tag(state) if state != "none" else '<span class="pill pill-overview">overview</span>', esc(when))

    def render_gallery(self):
        recs = self.registry
        kinds = [k for k in figures.KINDS if any(r["kind"] == k for r in recs)]
        tracks = [t for t in figures.TRACKS if any(r["track"] == t for r in recs)]

        def chips(group, values):
            return "".join('<a class="chip" id="%s-%s" href="#%s-%s" data-group="%s" data-value="%s" aria-pressed="false">%s <span class="muted">%d</span></a>' % (
                group, v, group, v, group, v, esc(v.title() if group == "track" else v), sum(1 for r in recs if r[group] == v)) for v in values)
        tiles = "".join(self.tile(r, i) for i, r in enumerate(recs))
        rows = []
        for i, r in enumerate(recs):
            ch = [c for c in self.chapters if c["id"] == r["chapter"]][0]
            st = self.fig_state(r)
            rows.append('<tr data-kind="%s" data-track="%s"><td><a href="%s.html#%s">%s</a></td><td>%s</td><td>%s</td><td>%s</td><td>%s</td><td class="nw">%s</td></tr>' % (
                r["kind"], r["track"], r["chapter"], esc(r["anchor"]), esc(r["title"]), esc(r["kind"]), esc(r["track"]), esc(ch["title"]), tag(st) if st != "none" else "overview",
                esc(self.date_label(r.get("date") or ch["date"]))))
        body = ('<main id="main" class="wrap gallery"><header class="chead"><div class="eyebrow">Gallery · %d figures</div><h1>Gallery</h1>'
                '<p class="lead">Every picture, diagram, chart and step-through in the archive, to browse first. Each tile links to the place in a chapter where its figure lives, and the story behind it. The tiles are built from the figure registry that each chapter declares, so a figure added to a chapter appears here by itself.</p></header>'
                '<form class="controls" id="galleryControls" aria-label="Filter and sort the gallery" onsubmit="return false">'
                '<div class="chiprow" role="group" aria-label="Kind"><span class="chip-h">Kind</span><a class="chip" id="kind-all" href="#kind-all" data-group="kind" data-value="" aria-pressed="true">All <span class="muted">%d</span></a>%s</div>'
                '<div class="chiprow" role="group" aria-label="Track"><span class="chip-h">Track</span><a class="chip" id="track-all" href="#track-all" data-group="track" data-value="" aria-pressed="true">All</a>%s</div>'
                '<div class="chiprow" role="group" aria-label="Sort and view"><span class="chip-h">Order</span>'
                '<button type="button" class="chip" data-sort="chapter" aria-pressed="true">Chapter order</button><button type="button" class="chip" data-sort="new" aria-pressed="false">Newest first</button>'
                '<span class="chip-h">View</span><button type="button" class="chip" data-view="grid" aria-pressed="true">Tiles</button><button type="button" class="chip" data-view="list" aria-pressed="false">List</button></div>'
                '<p class="count" id="galleryCount" aria-live="polite">Showing all %d figures.</p></form>'
                '<ul class="tiles" id="tiles">%s</ul>'
                '<details class="fig-listbox" id="figList"><summary>List view: every figure as text</summary><div class="tbl"><table><caption class="sr">Every figure, with its kind, track, chapter, state and date</caption><thead><tr><th scope="col">Figure</th><th scope="col">Kind</th><th scope="col">Track</th><th scope="col">Chapter</th><th scope="col">State</th><th scope="col">Date</th></tr></thead><tbody>%s</tbody></table></div></details>'
                '<p class="how">Tags come from <code>gh</code>: a figure takes the state of its chapter\'s pull requests, and a figure of the history of <code>main</code> is merged. A figure marked overview is generated from the whole roadmap or repository and has no single pull request. Previews of step-throughs are static: play them in the chapter.</p>'
                '</main>') % (len(recs), len(recs), chips("kind", kinds), chips("track", tracks), len(recs), tiles, "".join(rows))
        title = "%s · Gallery" % TITLE
        self.pages["gallery.html"] = document(title, site_header("gallery", self.ids + self.extra_ids()) + body + site_footer(self.data, None, None))

    def recent_strip(self):
        recs = list(enumerate(self.registry))
        recs.sort(key=lambda t: (t[1].get("date") or [c for c in self.chapters if c["id"] == t[1]["chapter"]][0]["date"], t[0]), reverse=True)
        picked, per = [], {}
        for i, r in recs:
            if per.get(r["chapter"], 0) >= 2:
                continue
            per[r["chapter"]] = per.get(r["chapter"], 0) + 1
            picked.append((i, r))
            if len(picked) == 6:
                break
        return ('<ul class="tiles recent">%s</ul><p class="how"><a href="gallery.html">Browse all %d figures in the gallery</a>, filtered by kind and track.</p>' % (
            "".join(self.tile(r, 100 + i) for i, r in picked), len(self.registry)))

    def render_index(self):
        d, ctx = self.data, self.ctx
        blocks = mdlite.parse_blocks(read_text(os.path.join(self.pages_dir, "front.md")))
        intro, _ = mdlite.render(blocks, ctx, lambda n, a: "")
        last = d["prs"][-1] if d["prs"] else None
        last_html = ("<code>%s</code>, %s %s: %s" % (last["mergeCommit"], ex.day_label(last["mergedAt"]), ex.hhmm(last["mergedAt"]), esc(last["title"]))) if last else "none"
        subject = [o for o in d["open"]["prs"] if not o["isReport"]]
        green = [o for o in subject if o["ci"] == "passing"]
        pend = [o for o in subject if o["ci"] != "passing"]
        open_html = ("<b>%d open, none merged.</b> %s. CI is green on the head commit of %d of %d%s." % (
            len(subject), ", ".join("#%d" % o["number"] for o in subject), len(green), len(subject),
            (" (not yet green: %s)" % ", ".join("#%d is %s" % (o["number"], o["ci"]) for o in pend)) if pend else "")) if subject else "none"
        later = d["mergeDay"]["since"]
        since = ("Since merge day only %s has merged (the generator of this report); the other pull requests wait." % ", ".join("#%d" % n for n in later)) if later else ("Nothing has been merged since merge day." if subject else "")
        panel = ('<dl class="status"><div><dt>Is there an operating system?</dt><dd><b>No.</b> What exists is a boot-to-kernel skeleton and a hosted skeleton on DOS, both merged and run in emulators. In open pull requests there are shared drivers, and a first window with a cursor drawn by the native kernel in QEMU. There is no shared Desk core yet.</dd></div>'
                 '<div><dt>The First Window milestone</dt><dd><b>Not reached.</b> It is a window drawn by the <em>shared</em> core and moved by a mouse, on both tracks. So far each track has its own demo, and neither uses a shared core.</dd></div>'
                 '<div><dt>Open pull requests</dt><dd>%s</dd></div>'
                 '<div><dt>Last merge into main</dt><dd>%s<div class="sub">%s</div></dd></div><div><dt>CI on the merge commits</dt><dd>%s</dd></div>'
                 '<div><dt>On real hardware</dt><dd><b>Not tried.</b> Every run so far was in QEMU, headless DOSBox Staging or a browser (the list under Not verified says what that leaves open).</dd></div>'
                 '<div><dt>Pull requests merged</dt><dd>%s, as merge commits, from <code>%s</code>.</dd></div></dl>') % (
            open_html, last_html, since, esc(str(d["metrics"]["ciGreen"]["value"])), esc(num(d["metrics"]["prs"]["value"])), esc(d["ref"]))
        # the shelves summary comes from the chapters, so render them first
        for i in range(len(self.chapters)):
            self.pages.update([self.render_chapter(i)])
        if self.registry:
            self.render_gallery()
        sh = []
        for k in ("catalogued", "uncatalogued", "wish"):
            name, what = SHELF_TEXT[k]
            items = [(c, it) for kk, c, it in self.shelf_items if kk == k]
            sh.append('<section class="shelf shelf-%s"><h3>%s <span class="count">%d</span></h3><p>%s</p><details><summary>All %d entries, by chapter</summary><ul>%s</ul></details></section>' % (
                k, name, len(items), esc(what), len(items), "".join('<li><a href="%s.html">%s</a>: %s</li>' % (c["id"], esc(c["title"]), it) for c, it in items)))
        toc = []
        coming = coming_rows(d)
        placed = False

        def coming_html():
            out = []
            for r in coming:
                out.append('<tr class="coming"><td class="n">·</td><td><b>%s</b> <span class="pill pill-coming">coming</span><div class="sub">%s</div></td><td class="nw">not written</td><td class="nw">%s</td><td class="nw">%s</td></tr>' % (
                    esc(r["title"]), inline(r["summary"], ctx), tag(r["state"]), esc(("#%d" % r["pr"]) if r["pr"] else "no pull request yet")))
            return "".join(out)
        for c in self.chapters:
            if c.get("section") == "reference" and not placed:
                toc.append(coming_html())
                placed = True
            when = self.date_label(c["date"]) if c.get("date") else ""
            toc.append('<tr><td class="n">%s</td><td><a href="%s.html"><b>%s</b></a><div class="sub">%s</div></td><td class="nw">%s</td><td class="nw">%s</td><td class="nw">%s</td></tr>' % (
                esc(c["n"]) if c.get("n") != "" else "", c["id"], esc(c["title"]), inline(c["summary"], ctx), esc(when),
                tag(chapter_state(d, c["prs"])) if c["prs"] and c.get("section") != "reference" else "",
                esc(", ".join("#%d" % n for n in c["prs"]))))
        if not placed:
            toc.append(coming_html())
        if self.registry:
            toc.append('<tr><td class="n">·</td><td><a href="gallery.html"><b>Gallery</b></a><div class="sub">Every figure in the archive as a tile, linked to the chapter where it lives: %d figures, from %d chapters.</div></td><td class="nw"></td><td class="nw"></td><td class="nw"></td></tr>' % (
                len(self.registry), len({r["chapter"] for r in self.registry})))
        road = d["roadmap"]
        cols = []
        for b in ("foundations", "hosted", "convergence", "native"):
            info = [x for x in road["branches"] if x["id"] == b][0]
            ns = [n for n in road["nodes"] if (n["branch"] == b or (b == "convergence" and n["branch"] == "milestone"))]
            order = {"done": 0, "done-emu": 1, "review": 2, "progress": 3, "planned": 4, "research": 5, "decision": 6}
            lis = "".join('<li>%s<span>%s %s</span></li>' % (ex.state_icon(n["state"], 16), esc(n["label"] + ((" · " + ", ".join("#%d" % k for k in n["prs"])) if n.get("prs") else "")), tag(n["tag"]))
                          for n in sorted(ns, key=lambda n: order[n["state"]]))
            cols.append('<section class="track"><h3>%s</h3><ul class="state-list">%s</ul><p class="next"><b>Next:</b> %s</p></section>' % (esc(info["label"]), lis, esc(info["next"])))
        used = ex.used_states(road["nodes"])
        nv = gather.read_json(os.path.join(HERE, "not-verified.json"))
        notver = "".join("<li><b>%s</b> %s</li>" % (esc(x["what"]), inline(x["why"], ctx)) for x in nv)
        hero = self.directive({"id": "index"})("figure", ["hero"])
        strip = ('<section aria-labelledby="h-recent"><h2 id="h-recent">Recent figures</h2><p class="sub col">The newest pictures and step-throughs, from the figure registry. Each tile links to where it lives.</p>%s</section>' % self.recent_strip()) if self.registry else ""
        body = ('%s<main id="main" class="wrap front">'
                '<header class="hero">%s<div class="eyebrow">Liminal HQ · Desk · The archive · generated %s</div><h1>Desk Progress Report</h1>'
                '<div class="prose lead-block">%s</div></header>'
                '<section aria-labelledby="h-status"><h2 id="h-status">Status panel</h2>%s</section>'
                '%s'
                '<section aria-labelledby="h-shelves"><h2 id="h-shelves">The three shelves</h2><p class="sub col">Everything in the archive sits on one of three shelves, and nothing is moved up a shelf until it has earned it. These are counts of entries on the chapter pages, not a measure of the work.</p><div class="shelves">%s</div></section>'
                '<section aria-labelledby="h-toc"><h2 id="h-toc">The chapters</h2><div class="tbl"><table class="toc"><caption class="sr">Chapters, with date and pull requests</caption><thead><tr><th scope="col" class="n">No.</th><th scope="col">Chapter</th><th scope="col">Date</th><th scope="col">State</th><th scope="col">Pull requests</th></tr></thead><tbody>%s</tbody></table></div></section>'
                '<section aria-labelledby="h-cols"><h2 id="h-cols">Status by track</h2><div class="glance">%s</div>%s</section>'
                '<section aria-labelledby="h-nums"><h2 id="h-nums">Headline numbers</h2><p class="sub col">Each number is computed by the generator from the repository or <code>gh</code>. The test figures are counts of test declarations and expected transcripts, not an official total.</p><h3>Merged into main</h3>%s<h3>In open pull requests, not merged</h3><p class="sub col">Read from the open branches, so they change when a branch does. None of this is on main.</p>%s</section>'
                '<section aria-labelledby="h-nv"><h2 id="h-nv">Not verified</h2><p class="sub col">What nobody has checked yet. Anything in the archive that depends on one of these stays on the uncatalogued shelf.</p><ul class="notver">%s</ul></section>'
                '</main>') % (site_header("index", self.ids + self.extra_ids()), hero, esc(d["generated"][:10]), intro, panel, strip, "".join(sh), "".join(toc), "".join(cols), ex.legend(used), facts_strip(d, HEADLINE), facts_strip(d, HEADLINE_OPEN), notver)
        return body

    def write(self):
        out = self.out
        if os.path.exists(out):
            # Only a directory this tool wrote (it holds our manifest) or an empty one is cleared; anything else is somebody's files.
            if os.listdir(out) and not os.path.exists(os.path.join(out, "manifest.json")):
                raise ValueError("refusing to clear %s: it is not empty and has no manifest.json from an earlier build" % out)
            shutil.rmtree(out)
        os.makedirs(os.path.join(out, "_publish"))
        copied = self.copy_assets() if self.assets else []
        self.thumbs, thumb_files = self.make_thumbs()
        copied += thumb_files
        index_body = self.render_index()
        footer = site_footer(self.data, None, self.chapters[0] if self.chapters else None)
        files = {}
        files["index.html"] = document(TITLE, index_body + footer)
        files["_publish/index.html"] = fragment(TITLE, index_body + footer)
        files.update(self.pages)
        files["site.css"] = read_text(os.path.join(HERE, "templates", "site.css"))
        files["site.js"] = read_text(os.path.join(HERE, "templates", "site.js"))
        for path, text in files.items():
            with open(os.path.join(out, path), "w", encoding="utf-8") as f:
                f.write(text)
        self.write_manifest(sorted(files) + copied)
        return files

    def copy_assets(self):
        out, root = self.out, gather.ROOT
        copied = []

        def cp(src, dst):
            if not os.path.exists(src):
                print("warning: missing asset %s" % src, file=sys.stderr)
                return
            os.makedirs(os.path.dirname(os.path.join(out, dst)), exist_ok=True)
            shutil.copyfile(src, os.path.join(out, dst))
            copied.append(dst)
        for f in FONTS:
            cp(os.path.join(root, "fonts", "generated", f + ".otf"), "fonts/%s.otf" % f)
        for s in (16, 12, 8):
            cp(os.path.join(root, "fonts", "generated", "emoji", "pixel-%d.png" % s), "img/pixel-%d.png" % s)
        cp(os.path.join(HERE, "media", "qemu-boot.png"), "img/qemu-boot.png")
        for name in sorted(os.listdir(os.path.join(HERE, "media"))):
            if name.startswith("img-") and name.endswith(".png"):
                cp(os.path.join(HERE, "media", name), "img/" + name)
        for src, dst in SHOTS:
            cp(os.path.join(root, "design", "screenshots", src), "shots/" + dst)
        return copied

    def write_manifest(self, paths):
        out = self.out
        entries = []
        for p in paths:
            fp = os.path.join(out, p)
            with open(fp, "rb") as f:
                blob = f.read()
            entries.append({"path": p, "bytes": len(blob), "sha256": hashlib.sha256(blob).hexdigest()})
        published = {p: p for p in paths if p not in ("index.html", "_publish/index.html", "manifest.json")}
        try:
            inside = os.path.commonpath([os.path.abspath(out), gather.ROOT]) == gather.ROOT
        except ValueError:
            inside = False
        rel = os.path.relpath(out, gather.ROOT) if inside else os.path.abspath(out)
        total_bytes = sum(e["bytes"] for e in entries)
        manifest = {
            "title": TITLE, "artifact": ARTIFACT_URL, "description": DESCRIPTION,
            "publish": {
                "action": "publish", "url": ARTIFACT_URL, "file_path": rel + "/_publish/index.html", "root": rel,
                "files": published, "description": DESCRIPTION,
                "notes": "Omit icon and title on a republish. file_path is the index as a fragment (the Artifact skeleton supplies the doctype and head); every other page is a full document served as it is.",
            },
            "limits": {"perFileBytes": 16 * 1024 * 1024, "filesPerPublish": 255, "totalBytesPerPublish": 64 * 1024 * 1024},
            "totals": {"files": len(entries), "bytes": total_bytes, "note": "every file written except manifest.json itself"},
            "files": entries,
        }
        with open(os.path.join(out, "manifest.json"), "w", encoding="utf-8") as f:
            json.dump(manifest, f, indent=1)
            f.write("\n")


# ---- checking -----------------------------------------------------------------------------------------------------------------

VOID = {"meta", "link", "br", "hr", "img", "input", "col", "area", "base", "source", "track", "wbr", "path", "rect", "circle", "line", "polygon", "polyline", "ellipse", "stop", "use"}


class Scan(html.parser.HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.stack, self.ids, self.hrefs, self.problems, self.svg = [], [], [], [], 0
        self.dupes = []

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if "id" in a:
            if a["id"] in self.ids:
                self.dupes.append(a["id"])
            self.ids.append(a["id"])
        if tag in ("a", "link", "script", "img") and (a.get("href") or a.get("src")):
            self.hrefs.append((tag, a.get("href") or a.get("src")))
        if tag not in VOID:
            self.stack.append(tag)

    def handle_startendtag(self, tag, attrs):
        a = dict(attrs)
        if "id" in a:
            self.ids.append(a["id"])
        if tag in ("img", "link") and (a.get("href") or a.get("src")):
            self.hrefs.append((tag, a.get("href") or a.get("src")))

    def handle_endtag(self, tag):
        if tag in VOID:
            return
        if not self.stack or self.stack[-1] != tag:
            self.problems.append("unexpected </%s> (open: %s)" % (tag, "/".join(self.stack[-4:])))
            if tag in self.stack:
                while self.stack and self.stack.pop() != tag:
                    pass
        else:
            self.stack.pop()


def check_site(out, expect_chapters=()):
    problems = []
    pages = sorted(f for f in os.listdir(out) if f.endswith(".html"))
    scans = {}
    for name in pages:
        text = read_text(os.path.join(out, name))
        s = Scan()
        s.feed(text)
        scans[name] = s
        for p in s.problems:
            problems.append("%s: %s" % (name, p))
        if s.stack:
            problems.append("%s: unclosed tags %s" % (name, "/".join(s.stack)))
        for d in s.dupes:
            problems.append("%s: duplicate id %s" % (name, d))
        if not text.lstrip().lower().startswith("<!doctype html>"):
            problems.append("%s: no doctype" % name)
        if "<title>" not in text or 'name="viewport"' not in text:
            problems.append("%s: missing title or viewport meta" % name)
        if text.count("<h1") != 1:
            problems.append("%s: expected exactly one h1, found %d" % (name, text.count("<h1")))
    for name, s in scans.items():
        for tag, href in s.hrefs:
            if re.match(r"^(https?:|mailto:|data:)", href) or href.startswith("javascript:"):
                continue
            path, _, frag = href.partition("#")
            page = path or name
            if not os.path.exists(os.path.join(out, page)):
                problems.append("%s: broken link %s" % (name, href))
            elif frag and page in scans and frag not in scans[page].ids:
                problems.append("%s: anchor #%s not found in %s" % (name, frag, page))
    if "index.html" in scans:
        linked = {h.partition("#")[0] for _, h in scans["index.html"].hrefs}
        for c in expect_chapters:
            if c + ".html" not in linked:
                problems.append("index.html: chapter %s is not linked" % c)
    for must in ("site.css", "site.js", "manifest.json"):
        if not os.path.exists(os.path.join(out, must)):
            problems.append("missing %s" % must)
    return problems


# ---- command line -------------------------------------------------------------------------------------------------------------

def main(argv):
    opts = {"--data": None, "--out": os.path.join(gather.ROOT, "dist", "progress-report"), "--chapters": os.path.join(HERE, "chapters"),
            "--pages": os.path.join(HERE, "pages")}
    flags, i = set(), 1
    while i < len(argv):
        if argv[i] in opts and i + 1 < len(argv):
            opts[argv[i]] = argv[i + 1]
            i += 2
        elif argv[i] in ("--no-assets", "--check"):
            flags.add(argv[i])
            i += 1
        else:
            print(USAGE, file=sys.stderr)
            return 2
    try:
        data = gather.read_json(opts["--data"]) if opts["--data"] else gather.gather()
        chapters = load_chapters(opts["--chapters"])
        site = Site(data, chapters, opts["--pages"], opts["--out"], assets="--no-assets" not in flags)
        files = site.write()
    except (mdlite.MdError, ValueError, KeyError, OSError) as e:
        print("build: %s: %s" % (type(e).__name__, e), file=sys.stderr)
        return 1
    print("wrote %d pages to %s" % (len([f for f in files if f.endswith(".html")]), opts["--out"]))
    if "--check" in flags:
        problems = check_site(opts["--out"], [c["id"] for c in chapters])
        for p in problems:
            print("check: " + p, file=sys.stderr)
        if problems:
            return 1
        print("check: ok")
    for w in list(data.get("warnings", [])) + ["missing: " + m for m in data.get("missing", [])]:
        print("warning: " + w, file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
