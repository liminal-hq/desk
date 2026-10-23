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

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import arch  # noqa: E402
import explainers as ex  # noqa: E402
import gather  # noqa: E402
import mdlite  # noqa: E402
from mdlite import Ctx, esc, inline  # noqa: E402

ARTIFACT_URL = "https://claude.ai/artifact/UiyueA93UpACK4UnMMh5as"
TITLE = "Desk Progress Report"
DESCRIPTION = ("The archive of the Desk project, chapter by chapter: what is catalogued, what is built but unverified, and what is only a wish, "
               "with the evidence for each. There is no operating system yet.")
FONTS = ["desk-title", "desk-ui", "desk-small", "desk-fixed", "desk-sans", "desk-sans-bold", "desk-sans-italic", "desk-serif",
         "desk-serif-italic", "desk-mono", "desk-mono-bold"]
SHOTS = [("01-v4-800.jpg", "01-v4-800.jpg"), ("02-v4-16.jpg", "02-v4-16.jpg"), ("v4-menu.jpg", "v4-menu.jpg"), ("files-resize.png", "files-resize.jpg")]
HEADLINE = ["prs", "ciGreen", "commitsAll", "files", "emojiPixel", "emojiVector", "bunTests", "transcripts", "s1Scenarios", "kernelLines", "rtLines", "docLines"]
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
        meta["blocks"] = mdlite.parse_blocks(body)
        meta.setdefault("n", "")
        meta.setdefault("section", "journey")
        meta["prs"] = [int(x) for x in meta.get("prs", [])]
        chapters.append(meta)
    ids = [c["id"] for c in chapters]
    if len(set(ids)) != len(ids):
        raise mdlite.MdError("duplicate chapter ids")
    return chapters


def num(v):
    return "{:,}".format(v) if isinstance(v, int) else str(v)


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


NAV_IDS = set()


def site_header(current):
    def link(i, href, text):
        if i != "index" and i not in NAV_IDS:
            return ""
        return '<a href="%s"%s>%s</a>' % (href, ' aria-current="page"' if current == i else "", text)
    return ('<a class="skip" href="#main">Skip to the content</a>\n<header class="site"><div class="wrap site-in"><a class="brand" href="index.html">Desk Progress Report</a>'
            '<nav aria-label="Site">%s%s%s%s</nav></div></header>' % (link("index", "index.html", "Contents"), link("path", "path.html", "The path"),
                                                                   link("findings", "findings.html", "Findings"), link("archive", "archive.html", "How this is made")))


def _old_site_header(current):
    return ('<a class="skip" href="#main">Skip to the content</a>\n<header class="site"><div class="wrap site-in"><a class="brand" href="index.html">Desk Progress Report</a>'
            '<nav aria-label="Site"><a href="index.html"%s>Contents</a><a href="path.html"%s>The path</a><a href="findings.html"%s>Findings</a><a href="archive.html"%s>How this is made</a></nav></div></header>' % (
                ' aria-current="page"' if current == "index" else "", ' aria-current="page"' if current == "path" else "",
                ' aria-current="page"' if current == "findings" else "", ' aria-current="page"' if current == "archive" else ""))


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

def pr_table(data, numbers, ctx):
    rows = []
    by = {p["number"]: p for p in data["prs"]}
    for n in numbers:
        p = by.get(n)
        if not p:
            rows.append('<tr><td>#%d</td><td colspan="5">not found among the merged pull requests</td></tr>' % n)
            continue
        url = p.get("url") or ctx.href("pr:%d" % n)
        rows.append('<tr><td><a href="%s" target="_blank" rel="noopener">#%d</a></td><td>%s</td><td><code>%s</code> · %s %s</td><td class="n">%d</td><td class="n">+%s</td><td>%s</td></tr>' % (
            esc(url), n, esc(p["title"]), p["mergeCommit"], ex.day_label(p["mergedAt"]), ex.hhmm(p["mergedAt"]), p["commitCount"], num(p["added"]),
            ("CI passed" if p["ci"] == "passing" else "CI " + p["ci"])))
    return ('<div class="tbl"><table><caption class="sr">Pull requests behind this chapter</caption><thead><tr><th scope="col">PR</th><th scope="col">Title</th><th scope="col">Merge commit</th>'
            '<th scope="col" class="n">Commits</th><th scope="col" class="n">Lines added</th><th scope="col">Checks</th></tr></thead><tbody>%s</tbody></table></div>'
            '<p class="how">From <code>git log --first-parent</code> and <code>git diff --numstat</code> on the merge commit; CI from <code>gh run list</code> on that commit.</p>') % "".join(rows)


def facts_strip(data, keys):
    cells = []
    for k in keys:
        m = data["metrics"][k]
        cells.append('<div class="fact"><b class="num">%s</b><span>%s</span><details><summary>how measured</summary><p>%s</p></details></div>' % (
            esc(num(m["value"])), esc(m["label"]), inline(m["how"], Ctx())))
    return '<div class="facts">%s</div>' % "".join(cells)


def metric_tile(data, k):
    return facts_strip(data, [k]).replace('class="facts"', 'class="facts one"')


# ---- the site -----------------------------------------------------------------------------------------------------------------

class Site:
    def __init__(self, data, chapters, pages_dir, out, assets=True):
        self.data, self.chapters, self.pages_dir, self.out, self.assets = data, chapters, pages_dir, out, assets
        self.ids = [c["id"] for c in chapters]
        NAV_IDS.clear()
        NAV_IDS.update(self.ids)
        self.repo_url = ((data.get("repo") or {}).get("url") or "https://github.com/liminal-hq/desk")
        self.ctx = Ctx(values_of(data), self.ids, self.repo_url, data["prs"])
        self.shelf_counts = {"catalogued": 0, "uncatalogued": 0, "wish": 0}
        self.shelf_items = []
        self.pages = {}

    # explainer and figure registry
    def directive(self, chapter):
        d, ctx = self.data, self.ctx
        media = os.path.join(HERE, "media")

        def run(name, args):
            if name == "explainer":
                xid = args[0]
                if xid == "boot":
                    return ex.boot(d, ctx, read_text(os.path.join(media, "native-serial.txt")), d["transcripts"]["nativeExpected"])
                if xid == "modules":
                    return ex.modules(d, ctx)
                if xid == "pipeline":
                    return ex.pipeline(d, ctx)
                if xid == "mergeday":
                    return ex.mergeday(d, ctx)
                raise mdlite.MdError("unknown explainer %s in %s" % (xid, chapter["id"]))
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
        main = ('<main id="main" class="wrap chapter"><article><header class="chead"><div class="eyebrow">%s</div><h1>%s</h1><p class="lead">%s</p></header>'
                '<div class="prose">%s</div>%s</article></main>') % (esc(" · ".join(eyebrow)), esc(c["title"]), inline(c["summary"], self.ctx), body, evidence)
        title = "%s · %s" % (TITLE, c["title"])
        return c["id"] + ".html", document(title, site_header(c["id"] if c["id"] in ("path", "findings", "archive") else "") + main + site_footer(self.data, prev, nxt))

    @staticmethod
    def date_label(iso):
        import datetime
        d = datetime.date.fromisoformat(iso)
        return "%d %s %d" % (d.day, d.strftime("%B"), d.year)

    def render_index(self):
        d, ctx = self.data, self.ctx
        blocks = mdlite.parse_blocks(read_text(os.path.join(self.pages_dir, "front.md")))
        intro, _ = mdlite.render(blocks, ctx, lambda n, a: "")
        last = d["prs"][-1] if d["prs"] else None
        last_html = ("<code>%s</code>, %s %s: %s" % (last["mergeCommit"], ex.day_label(last["mergedAt"]), ex.hhmm(last["mergedAt"]), esc(last["title"]))) if last else "none"
        panel = ('<dl class="status"><div><dt>Is there an operating system?</dt><dd><b>No.</b> There is a boot-to-kernel skeleton and a hosted skeleton on DOS, both run in emulators.</dd></div>'
                 '<div><dt>Last merge into main</dt><dd>%s</dd></div><div><dt>CI on the merge commits</dt><dd>%s</dd></div>'
                 '<div><dt>On real hardware</dt><dd><b>Not tried.</b> Every run so far was in QEMU or DOSBox Staging.</dd></div>'
                 '<div><dt>Open branches</dt><dd>Only <code>main</code>.</dd></div></dl>') % (last_html, esc(str(d["metrics"]["ciGreen"]["value"])))
        # the shelves summary comes from the chapters, so render them first
        for i in range(len(self.chapters)):
            self.pages.update([self.render_chapter(i)])
        sh = []
        for k in ("catalogued", "uncatalogued", "wish"):
            name, what = SHELF_TEXT[k]
            items = [(c, it) for kk, c, it in self.shelf_items if kk == k]
            sh.append('<section class="shelf shelf-%s"><h3>%s <span class="count">%d</span></h3><p>%s</p><details><summary>All %d entries, by chapter</summary><ul>%s</ul></details></section>' % (
                k, name, len(items), esc(what), len(items), "".join('<li><a href="%s.html">%s</a>: %s</li>' % (c["id"], esc(c["title"]), it) for c, it in items)))
        toc = []
        for c in self.chapters:
            when = self.date_label(c["date"]) if c.get("date") else ""
            toc.append('<tr><td class="n">%s</td><td><a href="%s.html"><b>%s</b></a><div class="sub">%s</div></td><td class="nw">%s</td><td class="nw">%s</td></tr>' % (
                esc(c["n"]) if c.get("n") != "" else "", c["id"], esc(c["title"]), inline(c["summary"], ctx), esc(when),
                esc(", ".join("#%d" % n for n in c["prs"]))))
        road = d["roadmap"]
        cols = []
        for b in ("foundations", "hosted", "native"):
            info = [x for x in road["branches"] if x["id"] == b][0]
            ns = [n for n in road["nodes"] if n["branch"] == b]
            order = {"done": 0, "done-emu": 1, "progress": 2, "review": 3, "planned": 4, "research": 5, "decision": 6}
            lis = "".join('<li>%s<span>%s</span></li>' % (ex.state_icon(n["state"], 16), esc(n["label"] + ((" · #%d" % n["pr"]) if n.get("pr") else ""))) for n in sorted(ns, key=lambda n: order[n["state"]]))
            cols.append('<section class="track"><h3>%s</h3><ul class="state-list">%s</ul><p class="next"><b>Next:</b> %s</p></section>' % (esc(info["label"]), lis, esc(info["next"])))
        used = [s for s in ["done", "done-emu", "progress", "planned", "research", "decision"] if any(n["state"] == s for n in road["nodes"])]
        nv = gather.read_json(os.path.join(HERE, "not-verified.json"))
        notver = "".join("<li><b>%s</b> %s</li>" % (esc(x["what"]), inline(x["why"], ctx)) for x in nv)
        hero = self.directive({"id": "index"})("figure", ["hero"])
        body = ('%s<main id="main" class="wrap front">'
                '<header class="hero">%s<div class="eyebrow">Liminal HQ · Desk · The archive · generated %s</div><h1>Desk Progress Report</h1>'
                '<div class="prose lead-block">%s</div></header>'
                '<section aria-labelledby="h-status"><h2 id="h-status">Status panel</h2>%s</section>'
                '<section aria-labelledby="h-shelves"><h2 id="h-shelves">The three shelves</h2><p class="sub col">Everything in the archive sits on one of three shelves, and nothing is moved up a shelf until it has earned it. These are counts of entries on the chapter pages, not a measure of the work.</p><div class="shelves">%s</div></section>'
                '<section aria-labelledby="h-toc"><h2 id="h-toc">The chapters</h2><div class="tbl"><table class="toc"><caption class="sr">Chapters, with date and pull requests</caption><thead><tr><th scope="col" class="n">No.</th><th scope="col">Chapter</th><th scope="col">Date</th><th scope="col">Pull requests</th></tr></thead><tbody>%s</tbody></table></div></section>'
                '<section aria-labelledby="h-cols"><h2 id="h-cols">Three columns of status</h2><div class="glance">%s</div>%s</section>'
                '<section aria-labelledby="h-nums"><h2 id="h-nums">Headline numbers</h2><p class="sub col">Each number is computed by the generator from the repository or <code>gh</code>. The test figures are counts of test declarations and expected transcripts, not an official total.</p>%s</section>'
                '<section aria-labelledby="h-nv"><h2 id="h-nv">Not verified</h2><p class="sub col">What nobody has checked yet. Anything in the archive that depends on one of these stays on the uncatalogued shelf.</p><ul class="notver">%s</ul></section>'
                '</main>') % (site_header("index"), hero, esc(d["generated"][:10]), intro, panel, "".join(sh), "".join(toc), "".join(cols), ex.legend(used), facts_strip(d, HEADLINE), notver)
        return body

    def write(self):
        out = self.out
        shutil.rmtree(out, ignore_errors=True)
        os.makedirs(os.path.join(out, "_publish"))
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
        copied = self.copy_assets() if self.assets else []
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
        rel = os.path.relpath(out, gather.ROOT) if out.startswith(gather.ROOT) else out
        manifest = {
            "title": TITLE, "artifact": ARTIFACT_URL, "description": DESCRIPTION,
            "publish": {
                "action": "publish", "url": ARTIFACT_URL, "file_path": rel + "/_publish/index.html", "root": rel,
                "files": published, "description": DESCRIPTION,
                "notes": "Omit icon and title on a republish. file_path is the index as a fragment (the Artifact skeleton supplies the doctype and head); every other page is a full document served as it is.",
            },
            "limits": {"perFileBytes": 16 * 1024 * 1024, "filesPerPublish": 255, "totalBytesPerPublish": 64 * 1024 * 1024},
            "totals": {"files": len(entries) + 1, "bytes": sum(e["bytes"] for e in entries)},
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
        if argv[i] in opts:
            opts[argv[i]] = argv[i + 1]
            i += 2
        elif argv[i] in ("--no-assets", "--check"):
            flags.add(argv[i])
            i += 1
        else:
            print(__doc__, file=sys.stderr)
            return 2
    data = gather.read_json(opts["--data"]) if opts["--data"] else gather.gather()
    chapters = load_chapters(opts["--chapters"])
    site = Site(data, chapters, opts["--pages"], opts["--out"], assets="--no-assets" not in flags)
    try:
        files = site.write()
    except (mdlite.MdError, ValueError) as e:
        print("build: %s" % e, file=sys.stderr)
        return 1
    print("wrote %d pages to %s" % (len([f for f in files if f.endswith(".html")]), opts["--out"]))
    if "--check" in flags:
        problems = check_site(opts["--out"], [c["id"] for c in chapters])
        for p in problems:
            print("check: " + p, file=sys.stderr)
        if problems:
            return 1
        print("check: ok")
    for w in data.get("warnings", []):
        print("warning: " + w, file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
