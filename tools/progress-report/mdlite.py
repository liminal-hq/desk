# A small markdown reader for the report's chapter files: front matter, paragraphs, headings, lists, tables, quotes and directives.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# The dialect is deliberately tiny and follows the repository's markdown rules (one line per paragraph). Inline: **bold**, *em*,
# `code`, [text](url) and {{m.key}} placeholders. Links may use the schemes chapter:ID, pr:N, commit:HASH and repo:PATH, which the
# builder resolves. Block directives are HTML comments on their own line: <!-- explainer ID -->, <!-- figure ID -->,
# <!-- facts KEY ... -->. A block quote that starts with **From the stacks** is a sidebar; `### Catalogued`, `### Uncatalogued` and
# `### Wish list` followed by a list are the three shelves.
import html
import re

SHELF_NAMES = {"catalogued": "catalogued", "uncatalogued": "uncatalogued", "wish list": "wish"}


class MdError(Exception):
    pass


def esc(s):
    return html.escape(str(s), quote=True)


def front_matter(text, name="<chapter>"):
    lines = text.splitlines()
    if not lines or lines[0] != "---":
        raise MdError("%s: no front matter" % name)
    meta, i = {}, 1
    while i < len(lines) and lines[i] != "---":
        m = re.match(r"^([A-Za-z][\w-]*):\s*(.*)$", lines[i])
        if not m:
            raise MdError("%s: bad front matter line %r" % (name, lines[i]))
        key, val = m.group(1), m.group(2).strip()
        if val.startswith("[") and val.endswith("]"):
            val = [v.strip() for v in val[1:-1].split(",") if v.strip()]
        meta[key] = val
        i += 1
    if i >= len(lines):
        raise MdError("%s: front matter is not closed" % name)
    return meta, "\n".join(lines[i + 1:])


class Ctx:
    """What inline text needs to resolve: placeholders and the chapter and pull request link targets."""

    def __init__(self, values=None, chapters=(), repo_url="https://github.com/liminal-hq/desk", prs=()):
        self.values, self.chapters, self.repo_url = values or {}, set(chapters), repo_url.rstrip("/")
        self.prs = {p["number"]: p for p in prs}
        self.used_chapters = set()

    def value(self, key):
        if key not in self.values:
            raise MdError("unknown placeholder {{%s}}" % key)
        v = self.values[key]
        if isinstance(v, int):
            return "{:,}".format(v)
        return str(v)

    def href(self, target):
        scheme, _, rest = target.partition(":")
        if scheme == "chapter":
            page, _, frag = rest.partition("#")
            if page not in self.chapters:
                raise MdError("link to a chapter that does not exist: %s" % target)
            self.used_chapters.add(page)
            return page + ".html" + ("#" + frag if frag else "")
        if scheme == "pr":
            return "%s/pull/%d" % (self.repo_url, int(rest))
        if scheme == "commit":
            return "%s/commit/%s" % (self.repo_url, rest)
        if scheme == "repo":
            return "%s/blob/main/%s" % (self.repo_url, rest)
        return target


def _text(s, ctx):
    s = re.sub(r"\{\{([\w.]+)\}\}", lambda m: ctx.value(m.group(1)), s)
    out = []
    for i, part in enumerate(re.split(r"(`[^`]+`)", s)):
        if i % 2:
            out.append("<code>%s</code>" % esc(part[1:-1]))
            continue
        part = esc(part)
        part = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", part)
        part = re.sub(r"(?<![\w*])\*([^*\s][^*]*?)\*(?![\w*])", r"<em>\1</em>", part)
        out.append(part)
    return "".join(out)


def inline(s, ctx):
    out = []
    for i, part in enumerate(re.split(r"(\[[^\]]+\]\([^)\s]+\))", s)):
        if i % 2:
            m = re.match(r"\[([^\]]+)\]\(([^)\s]+)\)$", part)
            target = m.group(2)
            ext = ' target="_blank" rel="noopener"' if re.match(r"^(https?://|pr:|commit:|repo:)", target) else ""
            out.append('<a href="%s"%s>%s</a>' % (esc(ctx.href(target)), ext, _text(m.group(1), ctx)))
        else:
            out.append(_text(part, ctx))
    return "".join(out)


def parse_blocks(body):
    """Lines to a list of blocks: dicts with a 'type'."""
    lines = body.splitlines()
    blocks, i = [], 0
    while i < len(lines):
        line = lines[i]
        if not line.strip():
            i += 1
        elif line.startswith("```"):
            lang = line[3:].strip()
            j = i + 1
            while j < len(lines) and not lines[j].startswith("```"):
                j += 1
            blocks.append({"type": "code", "lang": lang, "text": "\n".join(lines[i + 1:j])})
            i = j + 1
        elif re.match(r"^#{2,4} ", line):
            level = len(line.split(" ")[0])
            blocks.append({"type": "h", "level": level, "text": line[level + 1:].strip()})
            i += 1
        elif line.startswith("<!--"):
            m = re.match(r"^<!--\s*(\w+)\s*(.*?)\s*-->$", line)
            if not m:
                raise MdError("bad directive: %s" % line)
            blocks.append({"type": "directive", "name": m.group(1), "args": m.group(2).split()})
            i += 1
        elif line.startswith("- "):
            items = []
            while i < len(lines) and lines[i].startswith("- "):
                items.append(lines[i][2:])
                i += 1
            blocks.append({"type": "ul", "items": items})
        elif re.match(r"^\d+\. ", line):
            items = []
            while i < len(lines) and re.match(r"^\d+\. ", lines[i]):
                items.append(re.sub(r"^\d+\. ", "", lines[i]))
                i += 1
            blocks.append({"type": "ol", "items": items})
        elif line.startswith("|"):
            rows = []
            while i < len(lines) and lines[i].startswith("|"):
                rows.append([c.strip() for c in lines[i].strip().strip("|").split("|")])
                i += 1
            if len(rows) < 2 or not all(re.match(r"^:?-+:?$", c) for c in rows[1]):
                raise MdError("table without a separator row near %r" % rows[0][:1])
            blocks.append({"type": "table", "head": rows[0], "rows": rows[2:]})
        elif line.startswith(">"):
            quote = []
            while i < len(lines) and lines[i].startswith(">"):
                quote.append(lines[i][1:].strip())
                i += 1
            blocks.append({"type": "quote", "lines": [q for q in quote if q]})
        else:
            blocks.append({"type": "p", "text": line.strip()})
            i += 1
    return blocks


def shelf_of(block):
    return SHELF_NAMES.get(block["text"].lower()) if block["type"] == "h" and block["level"] == 3 else None


def render(blocks, ctx, directive):
    """Blocks to HTML. `directive(name, args)` returns the HTML of an explainer, a figure or a facts strip."""
    out, shelves, i = [], [], 0
    shelf_items = {}

    def flush():
        nonlocal shelves
        if shelves:
            out.append('<div class="shelves">' + "".join(shelves) + "</div>")
            shelves = []

    while i < len(blocks):
        b = blocks[i]
        shelf = shelf_of(b)
        if shelf and i + 1 < len(blocks) and blocks[i + 1]["type"] == "ul":
            items = blocks[i + 1]["items"]
            shelf_items[shelf] = [inline(x, ctx) for x in items]
            shelves.append('<section class="shelf shelf-%s" aria-labelledby="sh-%s-%d"><h3 id="sh-%s-%d">%s</h3><ul>%s</ul></section>' % (
                shelf, shelf, len(out), shelf, len(out), esc(b["text"]),
                "".join("<li>%s</li>" % inline(x, ctx) for x in items)))
            i += 2
            continue
        flush()
        t = b["type"]
        if t == "p":
            out.append("<p>%s</p>" % inline(b["text"], ctx))
        elif t == "h":
            hid = re.sub(r"[^a-z0-9]+", "-", b["text"].lower()).strip("-")
            out.append('<h%d id="%s">%s</h%d>' % (b["level"], hid, inline(b["text"], ctx), b["level"]))
        elif t == "ul":
            out.append("<ul>%s</ul>" % "".join("<li>%s</li>" % inline(x, ctx) for x in b["items"]))
        elif t == "ol":
            out.append("<ol>%s</ol>" % "".join("<li>%s</li>" % inline(x, ctx) for x in b["items"]))
        elif t == "code":
            out.append('<pre class="term"><code>%s</code></pre>' % esc(b["text"]))
        elif t == "table":
            head = "".join("<th scope=\"col\">%s</th>" % inline(c, ctx) for c in b["head"])
            rows = "".join("<tr>%s</tr>" % "".join("<td>%s</td>" % inline(c, ctx) for c in r) for r in b["rows"])
            out.append('<div class="tbl"><table><thead><tr>%s</tr></thead><tbody>%s</tbody></table></div>' % (head, rows))
        elif t == "quote":
            first = b["lines"][0] if b["lines"] else ""
            if first.startswith("**From the stacks"):
                m = re.match(r"^\*\*From the stacks(?:\s*[:—.-]\s*)?(.*?)\*\*\s*(.*)$", first)
                title = (m.group(1) if m else "").strip(" .:—")
                text = m.group(2) if m else first
                paras = ["<p>%s</p>" % inline(text, ctx)] + ["<p>%s</p>" % inline(x, ctx) for x in b["lines"][1:] if not x.startswith("Source")]
                srcs = [x for x in b["lines"][1:] if x.startswith("Source")]
                src_html = "".join('<p class="src">%s</p>' % inline(x, ctx) for x in srcs)
                out.append('<aside class="stacks" aria-label="From the stacks: background history, not evidence"><div class="stacks-tag">From the stacks%s</div>%s%s<p class="stacks-note">History and trivia for context. It is not evidence about the Desk.</p></aside>' % (
                    (" · " + esc(title)) if title else "", "".join(paras), src_html))
            else:
                out.append("<blockquote>%s</blockquote>" % "".join("<p>%s</p>" % inline(x, ctx) for x in b["lines"]))
        elif t == "directive":
            out.append(directive(b["name"], b["args"]))
        i += 1
    flush()
    return "\n".join(out), shelf_items
