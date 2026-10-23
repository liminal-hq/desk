# The figure registry: every picture, diagram, chart and explainer the archive shows is declared once, in the chapter that holds it, and
# the gallery, the chapters' anchors and the front page's strip of recent figures are all built from those declarations.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# A chapter declares its figures in a fenced block that starts with ```figures: records separated by a blank line, each line `key: value`.
#
#   id: pit-test                      a short unique name; the anchor on the chapter page is fig-<id> unless `anchor` says otherwise
#   kind: explainer                   screenshot, specimen, diagram, chart, explainer or history
#   track: shared                     hosted, native, shared or design
#   title: The PIT divisor, step by step
#   alt: ...                          what a reader who cannot see the figure needs (required; a sentence, not a filename)
#   caption: ...                      one line for the tile
#   at: explainer pit                 the directive in the chapter that draws it (its text without the comment marks); the generator wraps
#                                     the directive's output in the anchor. A screenshot instead names `anchor` of an element it draws itself.
#   thumb: svg                        img:PATH (a file in the site), svg (the first drawing in the figure), svgs (all drawings, up to 3),
#                                     steps (a card built from an explainer's step titles) or specimen (the first rows of the specimen figure)
#   state: in review                  optional; merged, in review, in progress, planned or none. The default is the state of the chapter's
#                                     pull requests, and merged for a chapter that has none (its subject is in the history of main).
#   date: 2026-10-23                  optional; the chapter's date by default
#
# Nothing here is hand-listed in the gallery: the gallery page reads the registry. `check()` fails the build when a record is incomplete, when a
# directive it names is not in the chapter, when its anchor or image does not exist, or when a figure has no alt text.
import re

from mdlite import MdError, esc

KINDS = ("screenshot", "specimen", "diagram", "chart", "explainer", "history")
TRACKS = ("hosted", "native", "shared", "design")
STATES = ("merged", "in review", "in progress", "planned", "none")
REQUIRED = ("id", "kind", "track", "title", "alt", "caption", "thumb")
# directives that draw something and so must be registered; the others (facts strips, tables of text, cards) are not figures
DRAWN = {"explainer", "figure"}
NOT_FIGURES = {("figure", "nexts"), ("figure", "conflicts")}


def parse_block(text, chapter):
    """The records of one ```figures block."""
    out, cur = [], {}
    for line in text.splitlines() + [""]:
        if not line.strip():
            if cur:
                out.append(cur)
                cur = {}
            continue
        m = re.match(r"^([a-z]+):\s*(.*)$", line)
        if not m:
            raise MdError("%s: bad line in a figures block: %r" % (chapter, line))
        cur[m.group(1)] = m.group(2).strip()
    for r in out:
        r["chapter"] = chapter
    return out


def take_blocks(blocks, chapter):
    """Splits a chapter's blocks into the blocks to render and the figure records declared in them."""
    keep, recs = [], []
    for b in blocks:
        if b["type"] == "code" and b.get("lang") == "figures":
            recs += parse_block(b["text"], chapter)
        else:
            keep.append(b)
    return keep, recs


def directive_key(name, args):
    return " ".join([name] + list(args))


def validate(records, chapters):
    """Checks every record on its own and against the chapters; returns the records in chapter order, each with its chapter's index."""
    ids = [c["id"] for c in chapters]
    seen = set()
    for r in records:
        where = "%s: figure %r" % (r["chapter"], r.get("id"))
        for k in REQUIRED:
            if not r.get(k):
                raise MdError("%s has no %s" % (where, k))
        if r["id"] in seen:
            raise MdError("%s: duplicate figure id" % where)
        seen.add(r["id"])
        if not re.match(r"^[a-z0-9][a-z0-9-]*$", r["id"]):
            raise MdError("%s: the id must be lower case letters, digits and hyphens" % where)
        if r["kind"] not in KINDS:
            raise MdError("%s: unknown kind %r" % (where, r["kind"]))
        if r["track"] not in TRACKS:
            raise MdError("%s: unknown track %r" % (where, r["track"]))
        if r.get("state") and r["state"] not in STATES:
            raise MdError("%s: unknown state %r" % (where, r["state"]))
        if len(r["alt"].split()) < 4 or re.search(r"\.(png|jpe?g|svg)$", r["alt"], re.I):
            raise MdError("%s: alt text must describe the figure in a sentence" % where)
        if r["chapter"] not in ids:
            raise MdError("%s: its chapter does not exist" % where)
        if not r.get("at") and not r.get("anchor"):
            raise MdError("%s: needs `at` (the directive that draws it) or `anchor`" % where)
        t = r["thumb"]
        if not (t in ("svg", "svgs", "steps", "specimen") or t.startswith("img:")):
            raise MdError("%s: unknown thumb %r" % (where, t))
        r["anchor"] = r.get("anchor") or "fig-" + r["id"]
        r["index"] = ids.index(r["chapter"])
    return sorted(records, key=lambda r: (r["index"], records.index(r)))


def unregistered(chapter_blocks, records, chapter):
    """The drawing directives of a chapter that no record claims."""
    claimed = {r.get("at") for r in records if r["chapter"] == chapter}
    out = []
    for b in chapter_blocks:
        if b["type"] == "directive" and b["name"] in DRAWN and (b["name"], b["args"][0] if b["args"] else "") not in NOT_FIGURES:
            if directive_key(b["name"], b["args"]) not in claimed:
                out.append(directive_key(b["name"], b["args"]))
    return out


# ---- previews --------------------------------------------------------------------------------------------------------------------------

def prefix_ids(svg, prefix):
    """Makes the ids inside an inline drawing unique on the gallery page, and takes it out of the tab order and the reading order: the
    tile's own link and text stand for it."""
    ids = set(re.findall(r'\bid="([^"]+)"', svg))
    for i in ids:
        svg = svg.replace('id="%s"' % i, 'id="%s%s"' % (prefix, i)).replace("url(#%s)" % i, "url(#%s%s)" % (prefix, i)).replace('href="#%s"' % i, 'href="#%s%s"' % (prefix, i))
    svg = re.sub(r'\s(tabindex|role|aria-label|data-id)="[^"]*"', "", svg)
    svg = re.sub(r"<title>.*?</title>", "", svg, flags=re.S)
    # only the root element's own size is dropped (the tile scales the drawing); the width and height of the shapes inside must stay
    root = re.match(r"<svg\b[^>]*>", svg)
    if root:
        tag = re.sub(r'\s(width|height)="[^"]*"', "", root.group(0))
        tag = tag.replace("<svg", '<svg aria-hidden="true" focusable="false"', 1)
        svg = tag + svg[root.end():]
    return svg


def svgs_of(html):
    """The outermost <svg> elements of a piece of HTML, nested drawings (the roadmap's state icons) kept inside their parents."""
    out, depth, start = [], 0, None
    for m in re.finditer(r"<svg\b|</svg>", html):
        if m.group(0) == "</svg>":
            depth -= 1
            if depth == 0 and start is not None:
                out.append(html[start:m.end()])
                start = None
        else:
            if depth == 0:
                start = m.start()
            depth += 1
    return out


def steps_card(title, html):
    """A static card for an explainer: its name, how many steps it has and its first step titles, from the rendered explainer."""
    titles = [re.sub(r"<[^>]+>", "", t) for t in re.findall(r'<h4 class="x-title">(.*?)</h4>', html, flags=re.S)]
    items = "".join('<li><span>%d</span>%s</li>' % (i + 1, t) for i, t in enumerate(titles[:5]))
    more = '<p class="pv-more">and %d more steps</p>' % (len(titles) - 5) if len(titles) > 5 else ""
    return ('<div class="pv-card" aria-hidden="true"><div class="pv-head"><b>%d steps</b><span class="pv-play">&#9654; Play</span></div><ol>%s</ol>%s</div>' % (len(titles), items, more))


def specimen_card(html):
    rows = re.findall(r'<div class="spec-row">.*?</div></div>', html, flags=re.S)[:3]
    return '<div class="pv-spec spec-dark" aria-hidden="true">%s</div>' % "".join(rows)


def preview(rec, html, prefix, thumbs):
    """The HTML of a tile's picture. `thumbs` maps an image path in the site to its thumbnail path (or itself)."""
    t = rec["thumb"]
    if t.startswith("img:"):
        src = t[4:]
        return '<img src="%s" alt="%s" loading="lazy" decoding="async">' % (esc(thumbs.get(src, src)), esc(rec["alt"]))
    if t in ("svg", "svgs"):
        found = svgs_of(html)
        if not found:
            raise MdError("%s: thumb %s but the figure draws no svg" % (rec["id"], t))
        if t == "svg":
            return '<div class="pv-svg">%s</div>' % prefix_ids(found[0], prefix)
        return '<div class="pv-svg pv-multi">%s</div>' % "".join(prefix_ids(s, "%s%d-" % (prefix, i)) for i, s in enumerate(found[:3]))
    if t == "steps":
        return steps_card(rec["title"], html)
    if t == "specimen":
        return specimen_card(html)
    raise MdError("unknown thumb %s" % t)
