# The first window's explainer and the two anatomy diagrams, each driven by files in the repository.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# `frames` steps through the six frames of tests/shell/first-window.scn with the digests of tests/shell/first-window.sums and the
# damage counts of the native transcripts, and refuses to build if those disagree with the result table of docs/FIRST-WINDOW.md.
# `anatomy` draws a layered diagram from a small JSON file (anatomy-dos.json, anatomy-desk.json): every box has a status and
# evidence, so promoting a box when something lands is a change to the data and not a redraw.
import hashlib
import json
import os
import re

import gather
from explainers import stepper
from mdlite import Raw, esc, inline

HERE = os.path.dirname(os.path.abspath(__file__))
FRAMES = ["open", "moved", "focus", "dragged", "typed", "closed"]

# What each frame is, from the scenario's own steps and comments in tests/shell/first-window.scn.
TEXT = {
    "open": ("Two windows open", "The scenario opens Notes (60, 60, 300 x 180) and Welcome to Desk (220, 150, 340 x 200), puts the pointer at (320, 240) and takes the first frame. Everything is new, so the whole screen is repainted."),
    "moved": ("The pointer moves, nothing else does", "The pointer moves by (100, 60), from (320, 240) to (420, 300). No window changed, so nothing is repainted: only the two 12 x 19 cursor rectangles are presented (widened to 8-pixel columns at depth 4)."),
    "focus": ("A click focuses the back window", "A click on Notes' title bar focuses and raises it. The halo moves to Notes and Welcome dims."),
    "dragged": ("A drag moves it by (40, 30)", "The title bar is pressed, the pointer moves by (40, 30) and the button is released. Only the area the window left and entered is repainted."),
    "typed": ("Typing reaches the field", "A click in Welcome's content focuses it again, and the four keys of \"desk\" go to its text field."),
    "closed": ("The close box closes it", "A click on Welcome's close box closes it and focuses Notes."),
}
ALT = {
    "open": "The first frame: a near-black desktop with a dimmer Notes window reading 'The first window.' behind the focused Welcome to Desk window, which has an orange halo, the prompt 'Type something:', an empty text field with a caret, and the arrow cursor in the middle of the screen.",
    "moved": "The same two windows as the first frame, with the arrow cursor moved to the lower right of the Welcome window.",
    "focus": "Notes has come to the front with the orange halo and the cursor on its title bar; the Welcome window is dimmed behind it, mostly hidden.",
    "dragged": "Notes, still in front and haloed, now 40 pixels to the right and 30 pixels lower than before, with the cursor on its title bar; the dimmed Welcome window is behind it.",
    "typed": "Welcome to Desk is in front again with its orange halo, the text field holding 'desk', and Notes dimmer behind it.",
    "closed": "Only the Notes window remains, with its orange halo, on the near-black desktop, and the arrow cursor at the right where the close box was.",
}


def read(path):
    with open(os.path.join(gather.ROOT, path), encoding="utf-8") as f:
        return f.read()


def sums():
    """{(frame, depth): (index digest, colour digest)} from the .sums file, which every platform must reproduce."""
    out = {}
    for line in read("tests/shell/first-window.sums").splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        name, depth, size, idx, col = line.split()
        out[(name, int(depth))] = (idx, col)
    return out


def damage(depth):
    """{frame: (repainted, presented, rectangles)} from the native transcript at that depth."""
    out = {}
    for line in read("tests/native/expected-shell-%d.txt" % depth).splitlines():
        m = re.match(r"shell: damage (\w+): (\d+) pixels repainted, (\d+) presented in (\d+) rectangles", line)
        if m:
            out[m.group(1)] = (int(m.group(2)), int(m.group(3)), int(m.group(4)))
    return out


def result_table():
    """{frame: (depth 8 colour, depth 4 colour)} from the Result section of docs/FIRST-WINDOW.md."""
    doc = read("docs/FIRST-WINDOW.md")
    doc = doc[doc.index("## Result"):]
    out = {}
    for m in re.finditer(r"^\| (\w+) \| `([0-9a-f]{8})` \| `([0-9a-f]{8})` \|$", doc, flags=re.M):
        out[m.group(1)] = (m.group(2), m.group(3))
    return out


def checked():
    s, table = sums(), result_table()
    if sorted(table) != sorted(FRAMES):
        raise ValueError("the result table of docs/FIRST-WINDOW.md does not list the six frames")
    for f in FRAMES:
        if (s[(f, 8)][1], s[(f, 4)][1]) != table[f]:
            raise ValueError("the digests of tests/shell/first-window.sums for %s disagree with the result table of docs/FIRST-WINDOW.md" % f)
    scn = [l.split()[1] for l in read("tests/shell/first-window.scn").splitlines() if l.startswith("frame ")]
    if scn != FRAMES:
        raise ValueError("the scenario's frames are %s, not the six this explainer describes" % scn)
    media = os.path.join(HERE, "media")

    def sha(p):
        with open(p, "rb") as fh:
            return hashlib.sha256(fh.read()).hexdigest()

    kept = [("img-fw-%s-8.png" % f, os.path.join("tests", "shell", "golden", "first-window", "%s-8.png" % f)) for f in FRAMES]
    kept += [("img-fw-native-8-%s.png" % f, os.path.join("docs", "images", "first-window", "native-8-%s.png" % f)) for f in ("dragged", "typed")]
    for mine, theirs in kept:
        if sha(os.path.join(media, mine)) != sha(os.path.join(gather.ROOT, theirs)):
            raise ValueError("media/%s is no longer byte-identical to %s" % (mine, theirs))
    for f in ("typed", "dragged"):
        if sha(os.path.join(media, "img-fw-%s-8.png" % f)) != sha(os.path.join(gather.ROOT, "docs", "images", "first-window", "dos-8-%s.png" % f)):
            raise ValueError("the kept DOS read-back of %s is no longer byte-identical to the golden" % f)
    return s, table


def img(name, alt, caption):
    return ('<figure class="shot fw-shot"><div class="screen"><img src="img/%s" alt="%s" width="640" height="480" loading="lazy"></div><figcaption>%s</figcaption></figure>' % (name, esc(alt), caption))


def frames(D, ctx):
    s, _ = checked()
    d8, d4 = damage(8), damage(4)
    steps, stage = [], []
    for i, f in enumerate(FRAMES):
        title, text = TEXT[f]
        a8, a4 = d8[f], d4[f]
        facts = ("Frame `%s`. Depth 8 colour digest `%s` (index `%s`); depth 4 colour digest `%s`. Damage at depth 8: %s pixels repainted, %s presented in %d rectangle%s; at depth 4: %s presented in %d." % (
            f, s[(f, 8)][1], s[(f, 8)][0], s[(f, 4)][1], "{:,}".format(a8[0]), "{:,}".format(a8[1]), a8[2], "" if a8[2] == 1 else "s", "{:,}".format(a4[1]), a4[2]))
        if f in ("dragged", "typed"):
            who = "Kept pictures: the host golden, which is byte-identical to the DOS read-back, and the native screendump."
        else:
            who = "Kept picture: the host golden only; the DOS and native frames are held to the same digest and were not kept as pictures."
        steps.append(("%d. %s" % (i + 1, title), Raw("%s %s %s" % (inline(text, ctx), inline(facts, ctx), esc(who)))))
        pics = img("img-fw-%s-8.png" % f, ALT[f], "Host golden, depth 8%s" % (" (the DOS read-back is the same file, byte for byte)" if f in ("dragged", "typed") else ""))
        if f in ("dragged", "typed"):
            pics += img("img-fw-native-8-%s.png" % f, ALT[f] + " (QEMU's screendump of the native kernel.)", "Native kernel, QEMU screendump, depth 8")
        stage.append('<div class="fw-frame" data-from="%d" data-to="%d"><div class="fw-pics">%s</div></div>' % (i, i, pics))
    cap = ('A step-through of <code>tests/shell/first-window.scn</code>. The digests are those of <code>tests/shell/first-window.sums</code>, the damage counts those of <code>tests/native/expected-shell-8.txt</code> and <code>expected-shell-4.txt</code>, and the generator refuses to build if the digests disagree with the result table of <code>docs/FIRST-WINDOW.md</code>. '
           'The pictures are the real files: the goldens in <code>tests/shell/golden/first-window/</code> and the native screendumps in <code>docs/images/first-window/</code>. Only the <code>dragged</code> and <code>typed</code> frames were kept from the native track.')
    return stepper("firstwindow", "The six frames of the first window, step by step", '<div class="fw-stage">%s</div>' % "".join(stage), steps, cap, ctx, "x-wide")


def digest_table(D, ctx):
    s, _ = checked()
    d8, d4 = damage(8), damage(4)
    rows = "".join("<tr><th scope=\"row\"><code>%s</code></th><td><code>%s</code></td><td><code>%s</code></td><td class=\"num\">%s</td><td class=\"num\">%s</td></tr>" % (
        f, s[(f, 8)][1], s[(f, 4)][1], "{:,}".format(d8[f][1]), "{:,}".format(d4[f][1])) for f in FRAMES)
    return ('<figure><div class="tbl"><table><caption class="sr">The six-bit colour digest of each frame at each depth, and the pixels presented</caption><thead><tr><th scope="col">Frame</th><th scope="col">Depth 8 colour digest</th><th scope="col">Depth 4 colour digest</th><th scope="col">Pixels presented, depth 8</th><th scope="col">Pixels presented, depth 4</th></tr></thead><tbody>%s</tbody></table></div>'
            '<figcaption>The host goldens, the DOS read-back and the native screendumps all reproduce the digests. Read from <code>tests/shell/first-window.sums</code> and the native transcripts and checked against <code>docs/FIRST-WINDOW.md</code>.</figcaption></figure>') % rows


# ---- the anatomy diagrams ------------------------------------------------------------------------------------------------------

LAYER_H, GAP, LABEL_W, COL_W, TOP = 66, 18, 118, 71, 8
KIND = {"verified": "built", "unverified": "review", "planned": "plan", "external": "ext"}
WORD = {"verified": "built and verified (in an emulator or on the host)", "unverified": "built, not verified", "planned": "planned, wish list", "external": "not ours"}


def load_anatomy(name):
    with open(os.path.join(HERE, "anatomy-%s.json" % name), encoding="utf-8") as f:
        return json.load(f)


def validate_anatomy(data):
    """Every box has one of the file's statuses, a place that fits the twelve columns, and (unless it is planned or not ours) evidence."""
    problems = []
    ids = [b["id"] for b in data["boxes"]]
    if len(set(ids)) != len(ids):
        problems.append("duplicate box ids")
    for b in data["boxes"]:
        if b["status"] not in data["statuses"]:
            problems.append("%s: status %r is not one of %s" % (b["id"], b["status"], data["statuses"]))
        if b["status"] in ("verified", "unverified") and not b.get("evidence"):
            problems.append("%s: a built box needs evidence" % b["id"])
        for e in b.get("evidence", []):
            if not re.match(r"^(pr:\d+|commit:[0-9a-f]{7,40}|repo:\S+)$", e):
                problems.append("%s: evidence %r is not pr:N, commit:HASH or repo:PATH" % (b["id"], e))
            elif e.startswith("repo:") and not os.path.exists(os.path.join(gather.ROOT, e[5:])):
                problems.append("%s: evidence %s does not exist in the tree" % (b["id"], e))
        if not (0 <= b["layer"] < len(data["layers"]) and b["col"] >= 0 and b["col"] + b["span"] <= 12):
            problems.append("%s: does not fit the grid" % b["id"])
        for k in ("label", "sub", "text"):
            if not b.get(k):
                problems.append("%s: no %s" % (b["id"], k))
    by = {b["id"]: b for b in data["boxes"]}
    for a in data.get("arrows", []):
        if a["from"] not in by or a["to"] not in by:
            problems.append("arrow %s to %s names a box that does not exist" % (a["from"], a["to"]))
    for layer in range(len(data["layers"])):
        row = sorted((b for b in data["boxes"] if b["layer"] == layer), key=lambda b: b["col"])
        for x, y in zip(row, row[1:]):
            if x["col"] + x["span"] > y["col"]:
                problems.append("%s and %s overlap" % (x["id"], y["id"]))
    return problems


def wrap(s, n):
    words, lines, cur = s.split(), [], ""
    for w in words:
        if cur and len(cur) + 1 + len(w) > n:
            lines.append(cur)
            cur = w
        else:
            cur = (cur + " " + w).strip()
    return lines + ([cur] if cur else [])


def evidence_links(items, ctx):
    out = []
    for e in items:
        kind, _, rest = e.partition(":")
        out.append(inline("[%s](%s)" % ("#" + rest if kind == "pr" else (rest[:7] if kind == "commit" else rest), e), ctx))
    return ", ".join(out)


def anatomy(name, ctx, alt_lead):
    data = load_anatomy(name)
    problems = validate_anatomy(data)
    if problems:
        raise ValueError("anatomy-%s.json: %s" % (name, "; ".join(problems)))
    n = len(data["layers"])
    W, H = LABEL_W + 12 * COL_W + 6, TOP + n * (LAYER_H + GAP)
    pos = {}
    o = ['<svg viewBox="0 0 %d %d" role="img" aria-label="%s" class="diagram anatomy"><defs><marker id="an-%s" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L10,5 L0,10 z" class="d-arrow"/></marker></defs>' % (
        W, H, esc(alt_lead), name)]
    for i, label in enumerate(data["layers"]):
        y = TOP + i * (LAYER_H + GAP)
        o.append('<text x="0" y="%d" font-size="12" class="d-muted">%s</text>' % (y + LAYER_H / 2 + 4, esc(label if len(label) < 17 else label[:16] + "…")))
    for b in data["boxes"]:
        x, y, w = LABEL_W + b["col"] * COL_W, TOP + b["layer"] * (LAYER_H + GAP), b["span"] * COL_W - 6
        pos[b["id"]] = (x, y, w)
        kind = KIND[b["status"]]
        dash = ' stroke-dasharray="6 4"' if b["status"] == "planned" else ""
        o.append('<g><title>%s: %s. %s</title><rect x="%d" y="%d" width="%d" height="%d" rx="6" class="a-%s" stroke-width="1.6"%s/>' % (esc(b["label"]), WORD[b["status"]], esc(b["sub"]), x, y, w, LAYER_H, kind, dash))
        per = max(8, int(w / 6.3))
        lab = wrap(b["label"], int(w / 7.4))[:2]
        sub = wrap(b["sub"], per)[:2]
        lines = [(t, "a-t1 a-t1-%s" % kind, 12.5) for t in lab] + [(t, "a-t2", 11) for t in sub]
        y0 = y + LAYER_H / 2 - (len(lines) - 1) * 7.5 + 4
        for k, (t, cls, size) in enumerate(lines):
            o.append('<text x="%g" y="%g" text-anchor="middle" font-size="%g" class="%s">%s</text>' % (x + w / 2, y0 + k * 15, size, cls, esc(t)))
        o.append("</g>")
    for a in data.get("arrows", []):
        (x1, y1, w1), (x2, y2, w2) = pos[a["from"]], pos[a["to"]]
        planned = "planned" in (next(b for b in data["boxes"] if b["id"] == a["from"])["status"], next(b for b in data["boxes"] if b["id"] == a["to"])["status"])
        if y1 == y2:
            xa, xb = (x1 + w1, x2) if x1 < x2 else (x1, x2 + w2)
            ya = yb = y1 + LAYER_H / 2
        else:
            xa, xb = x1 + w1 / 2, x2 + w2 / 2
            ya, yb = (y1 + LAYER_H, y2) if y1 < y2 else (y1, y2 + LAYER_H)
        o.append('<line x1="%g" y1="%g" x2="%g" y2="%g" class="d-line" stroke-width="1.3"%s marker-end="url(#an-%s)"/>' % (xa, ya, xb, yb, ' stroke-dasharray="4 3"' if planned else "", name))
    o.append("</svg>")
    legend = ('<ul class="m-key"><li><span class="sw a-sw-built"></span> built and verified (emulator or host; nothing on real hardware)</li><li><span class="sw a-sw-review"></span> built, not verified</li>'
              '<li><span class="sw a-sw-plan"></span> planned (dashed): wish list</li>%s</ul>' % ('<li><span class="sw a-sw-ext"></span> not ours</li>' if "external" in data["statuses"] else ""))
    links = {bid: v for (n, bid), v in getattr(ctx, "manual_boxes", {}).items() if n == name}

    def manual_cell(b):
        return "<td>%s</td>" % ", ".join('<a href="%s.html">%s</a>' % (m["id"], esc(m["label"])) for m in links.get(b["id"], [])) if links else ""
    rows = "".join('<tr%s><th scope="row">%s<div class="sub">%s</div></th><td><span class="an-st an-%s">%s</span></td><td>%s</td><td>%s</td>%s</tr>' % (
        (' id="box-%s-%s"' % (name, b["id"])) if links else "", esc(b["label"]), esc(data["layers"][b["layer"]]), b["status"], esc(WORD[b["status"]]), inline(b["text"], ctx), evidence_links(b.get("evidence", []), ctx) or "—", manual_cell(b)) for b in data["boxes"])
    extra_head = '<th scope="col">Manual</th>' if links else ""
    table = ('<details class="an-text"><summary>The same diagram as text: every box, its status and its evidence</summary><div class="tbl"><table><caption class="sr">Boxes of the diagram with status and evidence</caption>'
             '<thead><tr><th scope="col">Box</th><th scope="col">Status</th><th scope="col">What it is</th><th scope="col">Evidence</th>%s</tr></thead><tbody>%s</tbody></table></div></details>') % (extra_head, rows)
    return ('<figure><div class="chart scroll" tabindex="0" aria-label="%s, scrolls sideways"><div style="min-width:820px">%s</div></div>%s<figcaption>%s</figcaption>%s</figure>') % (
        esc(data["title"]), "".join(o), legend,
        "Drawn from <code>tools/progress-report/anatomy-%s.json</code>, last updated %s: each box has a status and evidence in that file, and the generator refuses a built box without evidence." % (name, esc(data["updated"])), table)
