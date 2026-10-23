# The report's charts, drawn as inline SVG from the gathered data: the font matrix, emoji coverage, progress by track and the stack of open
# pull requests. Each is one claim, labelled directly, with its numbers in text as well so nothing depends on colour.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Colours come from classes in site.css (tokens that follow the theme). The status colours are the reserved ones of the roadmap's icons, so a
# state looks the same here as in the tree.
import re

from mdlite import esc

STATE_NAMES = {"done": "catalogued", "done-emu": "catalogued in an emulator", "review": "built, in review", "progress": "partly done",
               "planned": "planned", "research": "idea", "decision": "open decision"}
STATE_ORDER = ["done", "done-emu", "review", "progress", "planned", "research", "decision"]


def _fig(svg, caption, cls=""):
    return '<figure class="chartfig %s"><div class="chart scroll" tabindex="0" aria-label="Chart, scrolls sideways when narrow"><div style="min-width:640px">%s</div></div><figcaption>%s</figcaption></figure>' % (cls, svg, caption)


# ---- the font matrix -------------------------------------------------------------------------------------------------------------------

FAMILIES = ["title", "ui", "small", "fixed", "fixed-dos", "mono", "sans", "serif"]
STYLES = [("", "Regular"), ("bold", "Bold"), ("italic", "Italic"), ("bold-italic", "Bold italic"), ("round", "Round")]


def font_matrix(D):
    by = {f["file"]: f for f in D["fonts"]}
    cw, ch, x0, y0 = 118, 34, 110, 40
    W, H = x0 + len(STYLES) * cw + 6, y0 + len(FAMILIES) * ch + 36
    o = ['<svg viewBox="0 0 %d %d" role="img" class="diagram" aria-label="Matrix of the Desk font families against their styles. Each filled cell holds the number of glyphs in the OpenType file.">' % (W, H)]
    for j, (_, name) in enumerate(STYLES):
        o.append('<text x="%d" y="26" text-anchor="middle" font-size="12" class="d-text2">%s</text>' % (x0 + j * cw + cw / 2, name))
    have = 0
    for i, fam in enumerate(FAMILIES):
        y = y0 + i * ch
        o.append('<text x="%d" y="%d" text-anchor="end" font-size="13" class="a-t1">Desk %s</text>' % (x0 - 10, y + 22, esc(fam.replace("-", " ").title())))
        for j, (sty, _) in enumerate(STYLES):
            name = "desk-%s%s%s.otf" % (fam, ("-" + sty) if sty and sty != "round" else "", "-round" if sty == "round" else "")
            f = by.get(name)
            x = x0 + j * cw
            if f:
                have += 1
                o.append('<g><title>%s: %s glyphs, %s bytes</title><rect x="%d" y="%d" width="%d" height="%d" rx="4" class="a-built"/>'
                         '<text x="%d" y="%d" text-anchor="middle" font-size="13" class="num a-t1">%s</text></g>' % (esc(name), "{:,}".format(f["glyphs"]), "{:,}".format(f["size"]), x + 3, y + 3, cw - 6, ch - 6, x + cw / 2, y + 22, "{:,}".format(f["glyphs"])))
            else:
                o.append('<rect x="%d" y="%d" width="%d" height="%d" rx="4" class="a-plan" stroke-dasharray="3 3" stroke-width="1"/>' % (x + 3, y + 3, cw - 6, ch - 6))
    o.append('<text x="%d" y="%d" font-size="12" class="d-muted">Cell: glyphs in the OpenType file. A dashed cell is a style the family does not have.</text></svg>' % (x0, H - 8))
    cap = ("The %d OpenType files of the Desk's outline and pixel families against their styles, with the glyph count of each, read from the files under <code>fonts/generated/</code> in the tree of <code>main</code>. The four <code>.DFN</code> bitmap files for DOS are not in the matrix." % have)
    return _fig("".join(o), cap)


# ---- emoji coverage --------------------------------------------------------------------------------------------------------------------

def emoji_coverage(D):
    e = D["emoji"]
    rows = [("Pixel, 16 px", e["pixel"], "c1"), ("Pixel, 12 px", e["pixel12"], "c1"), ("Pixel, 8 px", e["pixel8"], "c1"), ("Vector (colour font)", e["vector"], "c3")]
    mx = max(r[1] for r in rows)
    L, R = 170, 520
    H = 40 + len(rows) * 30 + 70
    o = ['<svg viewBox="0 0 760 %d" role="img" class="diagram" aria-label="Bars of the emoji drawn in each form: %s. Below, the Gboard request of %d distinct sequences, %d done and %d planned.">' % (
        H, "; ".join("%s %d" % (r[0], r[1]) for r in rows), e["requested"], e["requestedDone"], e["requestedPlanned"])]
    for i, (name, v, c) in enumerate(rows):
        y = 20 + i * 30
        w = v / mx * R
        o.append('<text x="%d" y="%d" text-anchor="end" font-size="13" class="a-t1">%s</text><rect x="%d" y="%d" width="%.1f" height="18" rx="2" class="sz-f sz-%s"/><text x="%.1f" y="%d" font-size="12" class="num d-text2">%d</text>' % (
            L - 10, y + 14, esc(name), L, y, w, c[1], L + w + 6, y + 14, v))
    y = 40 + len(rows) * 30
    tot = e["requested"]
    d_w, p_w = e["requestedDone"] / tot * R, e["requestedPlanned"] / tot * R
    o.append('<text x="%d" y="%d" text-anchor="end" font-size="13" class="a-t1">Gboard request</text>' % (L - 10, y + 14))
    o.append('<rect x="%d" y="%d" width="%.1f" height="18" rx="2" class="a-built"/><rect x="%.1f" y="%d" width="%.1f" height="18" rx="2" class="a-plan" stroke-dasharray="4 3"/>' % (L, y, d_w, L + d_w + 2, y, max(p_w - 2, 0)))
    o.append('<text x="%.1f" y="%d" text-anchor="middle" font-size="12" class="num a-t1">%d done</text><text x="%.1f" y="%d" text-anchor="middle" font-size="12" class="num a-t2">%d planned</text>' % (
        L + d_w / 2, y + 14, e["requestedDone"], L + d_w + p_w / 2, y + 14, e["requestedPlanned"]))
    o.append('<text x="%d" y="%d" font-size="12" class="d-muted">%d distinct sequences requested; the vector set holds %d of the same emoji.</text></svg>' % (L, y + 44, tot, e["vector"]))
    cap = "How many emoji the Desk has drawn, by form, and how much of the %d-sequence request is done. Counts are read from the headers of the <code>.dem</code> files, <code>docs/EMOJI.md</code> and <code>docs/EMOJI-ROADMAP.md</code> on <code>main</code>." % tot
    return _fig("".join(o), cap)


# ---- progress by track -----------------------------------------------------------------------------------------------------------------

def track_progress(D):
    road = D["roadmap"]
    labels = {b["id"]: b["label"] for b in road["branches"]}
    labels["milestone"] = "Milestone"
    order = ["foundations", "hosted", "convergence", "native", "games", "milestone"]
    counts = {}
    for n in road["nodes"]:
        if n["branch"] == "root":
            continue
        counts.setdefault(n["branch"], {}).setdefault(n["state"], 0)
        counts[n["branch"]][n["state"]] += 1
    L, R, bh = 190, 500, 22
    mx = max(sum(c.values()) for c in counts.values())
    H = 60 + len(order) * 34 + 40
    o = ['<svg viewBox="0 0 800 %d" role="img" class="diagram" aria-label="Steps of the roadmap by track, split by state: %s.">' % (H, " ".join("%s: %s." % (labels[b], ", ".join("%d %s" % (counts[b][s], STATE_NAMES[s]) for s in STATE_ORDER if s in counts[b])) for b in order if b in counts))]
    x = 20
    for s in STATE_ORDER:
        if any(s in counts[b] for b in counts):
            o.append('<rect x="%d" y="10" width="12" height="12" rx="2" class="pg pg-%s"/><text x="%d" y="21" font-size="11.5" class="d-text2">%s</text>' % (x, s, x + 17, STATE_NAMES[s]))
            x += 30 + 6.3 * len(STATE_NAMES[s])
    for i, b in enumerate(order):
        if b not in counts:
            continue
        y = 50 + i * 34
        o.append('<text x="%d" y="%d" text-anchor="end" font-size="13" class="a-t1">%s</text>' % (L - 10, y + 16, esc(labels[b])))
        cx = L
        unit = R / mx
        for s in STATE_ORDER:
            n = counts[b].get(s, 0)
            if not n:
                continue
            w = n * unit
            o.append('<g><title>%s: %d %s</title><rect x="%.1f" y="%d" width="%.1f" height="%d" class="pg pg-%s"/><text x="%.1f" y="%d" text-anchor="middle" font-size="12" class="num pg-t">%d</text></g>' % (
                esc(labels[b]), n, STATE_NAMES[s], cx + 1, y, max(w - 2, 2), bh, s, cx + w / 2, y + 16, n))
            cx += w
        o.append('<text x="%.1f" y="%d" font-size="12" class="num d-text2">%d steps</text>' % (cx + 8, y + 16, sum(counts[b].values())))
    o.append("</svg>")
    cap = "Every step of <code>roadmap.json</code>, by track and by state. A step is only catalogued if its pull requests are merged with CI green; steps in open pull requests count as built and in review. Nothing here has run on real hardware."
    return _fig("".join(o), cap)


# ---- the stack of open pull requests ---------------------------------------------------------------------------------------------------

def pr_stack(D):
    prs = sorted(D["open"]["prs"], key=lambda p: p["number"])
    if not prs:
        return '<figure class="chartfig"><p class="how">No pull request is open: every branch the generator knows has been merged, or no pull request data was available.</p></figure>'
    by_branch = {p["branch"]: p for p in prs}
    depth = {}

    def dep(p):
        if p["number"] in depth:
            return depth[p["number"]]
        base = by_branch.get(p["base"])
        depth[p["number"]] = 1 + (dep(base) if base else 0)
        return depth[p["number"]]
    for p in prs:
        dep(p)
    cols = {}
    for p in sorted(prs, key=lambda p: (depth[p["number"]], p["number"])):
        cols.setdefault(depth[p["number"]], []).append(p)
    BW, BH, CG, RG = 190, 58, 70, 14
    maxrows = max(len(v) for v in cols.values())
    W, H = 20 + 70 + (len(cols)) * (BW + CG), 20 + maxrows * (BH + RG)
    pos = {}
    for d, ps in cols.items():
        for i, p in enumerate(ps):
            pos[p["number"]] = (90 + (d - 1) * (BW + CG), 10 + i * (BH + RG))
    o = ['<svg viewBox="0 0 %d %d" role="img" class="diagram" aria-label="The open pull requests and the branch each is based on: %s. Dashed lines join a pull request to open branches it contains.">' % (
        W, H, "; ".join("#%d is based on %s" % (p["number"], ("#%d" % by_branch[p["base"]]["number"]) if p["base"] in by_branch else p["base"]) for p in prs)),
        '<defs><marker id="pr-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" class="d-arrow"/></marker></defs>']
    mid = H / 2
    o.append('<rect x="0" y="%g" width="64" height="40" rx="6" class="a-ext"/><text x="32" y="%g" text-anchor="middle" font-size="13" class="a-t1 a-t1-ext">main</text>' % (mid - 20, mid + 4))
    for p in prs:
        x, y = pos[p["number"]]
        base = by_branch.get(p["base"])
        if base:
            bx, by_ = pos[base["number"]]
            o.append('<path d="M%d,%g C%g,%g %g,%g %d,%g" fill="none" class="d-line" stroke-width="1.5" marker-end="url(#pr-arrow)"/>' % (x, y + BH / 2, x - 30, y + BH / 2, bx + BW + 30, by_ + BH / 2, bx + BW + 2, by_ + BH / 2))
        else:
            o.append('<path d="M%d,%g C%g,%g %g,%g 66,%g" fill="none" class="d-line" stroke-width="1.5" marker-end="url(#pr-arrow)"/>' % (x, y + BH / 2, x - 30, y + BH / 2, 80, mid, mid))
        for k in p["stackedOn"]:
            if base and k == base["number"]:
                continue
            ox, oy = pos[k]
            o.append('<path d="M%d,%g C%g,%g %g,%g %d,%g" fill="none" class="d-line" stroke-width="1.2" stroke-dasharray="4 3"/>' % (x + BW / 2, y, x + BW / 2, y - 16, ox + BW / 2 + 40, oy + BH + 20, ox + BW / 2, oy + BH))
    for p in prs:
        x, y = pos[p["number"]]
        kind = "plan" if p["isReport"] else "review"
        title = p["title"] if len(p["title"]) <= 30 else p["title"][:29] + "…"
        o.append('<g><title>#%d %s: %s, CI %s, based on %s</title><rect x="%d" y="%d" width="%d" height="%d" rx="6" class="a-%s" stroke-width="1.6"/>'
                 '<text x="%d" y="%d" font-size="13" class="a-t1 a-t1-%s">#%d</text><text x="%d" y="%d" font-size="11.5" class="a-t2">%s</text><text x="%d" y="%d" font-size="11.5" class="a-t2">%s · CI %s</text></g>' % (
                     p["number"], esc(p["title"]), p["state"], p["ci"], esc(p["base"]), x, y, BW, BH, kind, x + 10, y + 19, kind, p["number"], x + 10, y + 35, esc(title), x + 10, y + 51, esc(p["state"]), esc("green" if p["ci"] == "passing" else p["ci"])))
    o.append("</svg>")
    cap = ("The open pull requests as the generator reads them from <code>gh</code> and <code>git</code>. A solid arrow points to the branch a pull request is based on; a dashed line joins it to other open branches it contains by merge. The two pull requests that carry this report are drawn dashed.")
    return _fig("".join(o), cap)
