# The Manual's data: its plan, its chapters, their state tables, glossary and commands, the checks that keep every pointer true, and staleness.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# A manual chapter is `manual/MNN-id.md`: a front matter, prose, and three kinds of fenced block that the generator reads as data:
#
#   ```status      one record per mechanism: id, label, state, evidence (and an optional note)
#   ```try         the commands a reader can run, each with the output it printed, and what it needs
#   ```glossary    one record per term the chapter defines: term, def
#
# Nothing here draws a page; build.py does that with the helpers at the end of this file. Everything that can be wrong about a chapter
# (a path that no longer exists, a symbol that was renamed, a state that claims more than its evidence shows, a command that does not
# exist) is found by `check_chapter`, which the build and the tests both call, so the manual cannot drift quietly from the code.
import importlib.util
import json
import os
import re
import shlex
import subprocess

import gather
import mdlite
from mdlite import MdError, Raw, esc, inline

HERE = os.path.dirname(os.path.abspath(__file__))
MANUAL_DIR = os.path.join(HERE, "manual")

STATES = ("built-verified", "built-unverified", "planned", "external")
STATE_WORD = {"built-verified": "Built, verified", "built-unverified": "Built, not verified", "planned": "Planned", "external": "Not ours"}
STATE_CLASS = {"built-verified": "verified", "built-unverified": "unverified", "planned": "planned", "external": "external"}
STATE_WHAT = {
    "built-verified": "Built, and something ran: a test, or a merged pull request with CI passing. In an emulator or on the host, never on real hardware.",
    "built-unverified": "Built, but no test or merged pull request with CI passing shows it running.",
    "planned": "Not built. It is drawn or described in a design document.",
    "external": "Someone else's code that the project relies on.",
}
EVIDENCE_KINDS = ("pr", "commit", "repo", "sym", "test", "doc")
REQUIRED_FRONT = ("id", "n", "part", "title", "summary", "date", "reviewed", "watches", "log", "anatomy")
REQUIRED_SECTIONS = ("What it is", "How it works", "Why it is this way", "The code", "Worked example", "State of this layer", "Try it yourself",
                     "Proved in the log", "Where next", "Revisions")
FENCES = ("status", "try", "glossary")
# What a "Try it yourself" command may start with when it is not a script in the repository.
TOOLS = {"cargo", "python3", "docker", "bun", "git", "grep", "od", "xxd", "ls", "wc", "sha256sum", "head", "tail", "cat", "sed", "cut", "tr", "sort", "uniq", "test", "echo", "nasm", "qemu-system-i386", "stat", "file", "diff", "cmp"}
INTERPRETERS = {"python3", "bun"}


# ---- small readers ------------------------------------------------------------------------------------------------------------

def slug(text):
    """The anchor GitHub gives a heading: lower case, punctuation dropped, spaces to hyphens."""
    s = re.sub(r"[^\w\s-]", "", text.lower().replace("`", ""))
    return re.sub(r"\s+", "-", s.strip())


def read_text(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


def parse_records(text, where):
    """Records separated by blank lines, each line `key: value`; a line that starts with a space continues the value before it."""
    out, cur, last = [], {}, None
    for line in text.splitlines() + [""]:
        if not line.strip():
            if cur:
                out.append(cur)
                cur, last = {}, None
            continue
        if line[0] == " " and last:
            cur[last] += " " + line.strip()
            continue
        m = re.match(r"^([a-z]+):\s*(.*)$", line)
        if not m:
            raise MdError("%s: bad line in a block: %r" % (where, line))
        last = m.group(1)
        if last in cur:
            raise MdError("%s: key %s given twice in one record" % (where, last))
        cur[last] = m.group(2).strip()
    return out


def split_evidence(text):
    """`pr:6, sym:kernel/src/heap.rs::Heap` to [("pr", "6"), ("sym", "kernel/src/heap.rs::Heap")]."""
    items = []
    for part in re.split(r",\s*", text.strip()):
        if not part:
            continue
        kind, sep, rest = part.partition(":")
        items.append((kind, rest if sep else ""))
    return items


def parse_status(text, chapter):
    recs = parse_records(text, chapter + " status")
    for r in recs:
        for k in ("id", "label", "state"):
            if not r.get(k):
                raise MdError("%s: a status record needs %s (%r)" % (chapter, k, r.get("id") or r))
        r["chapter"] = chapter
        r["items"] = split_evidence(r.get("evidence", ""))
    return recs


def parse_glossary(text, chapter):
    recs = parse_records(text, chapter + " glossary")
    for r in recs:
        for k in ("term", "def"):
            if not r.get(k):
                raise MdError("%s: a glossary record needs %s (%r)" % (chapter, k, r))
        r["id"] = slug(r["term"])
        r["chapter"] = chapter
    return recs


def parse_try(text, chapter):
    """A try block: `needs:` and `ran:` lines, then commands (`$ cmd`) each followed by the output it printed."""
    needs = ran = ""
    cmds, cur = [], None
    for line in text.splitlines():
        if cur is None and line.startswith("needs:"):
            needs = line[6:].strip()
        elif cur is None and line.startswith("ran:"):
            ran = line[4:].strip()
        elif line.startswith("$ "):
            cur = {"cmd": line[2:].strip(), "out": []}
            cmds.append(cur)
        elif cur is not None:
            cur["out"].append(line)
        elif line.strip():
            raise MdError("%s: a try block starts with needs:, ran: or a command, not %r" % (chapter, line))
    for c in cmds:
        while c["out"] and not c["out"][-1].strip():
            c["out"].pop()
    return {"needs": needs, "ran": ran, "cmds": cmds, "chapter": chapter}


# ---- the plan -----------------------------------------------------------------------------------------------------------------

def load_plan(directory=MANUAL_DIR):
    path = os.path.join(directory, "plan.json")
    if not os.path.exists(path):
        return None
    with open(path, encoding="utf-8") as f:
        plan = json.load(f)
    problems = validate_plan(plan)
    if problems:
        raise MdError("manual/plan.json: " + "; ".join(problems))
    return plan


def validate_plan(plan, root=None):
    problems = []
    parts = [p["id"] for p in plan["parts"]]
    ids = [c["id"] for c in plan["chapters"]] + [a["id"] for a in plan.get("appendices", [])]
    if len(set(ids)) != len(ids):
        problems.append("duplicate ids")
    nums = [c["n"] for c in plan["chapters"]]
    if len(set(nums)) != len(nums):
        problems.append("duplicate M-numbers")
    for c in plan["chapters"]:
        if not c["id"].startswith("m-"):
            problems.append("%s: ids start with m-" % c["id"])
        if c["part"] not in parts:
            problems.append("%s: unknown part %s" % (c["id"], c["part"]))
        for k in ("title", "summary", "n"):
            if not c.get(k):
                problems.append("%s: no %s" % (c["id"], k))
    chapter_ids = [c["id"] for c in plan["chapters"]]
    for name in ("learn", "find"):
        order = plan["paths"].get(name, [])
        if sorted(order) != sorted(chapter_ids) or len(set(order)) != len(order):
            problems.append("the %s path must list every chapter exactly once" % name)
    for t in plan.get("tasks", []):
        for cid in t["chapters"]:
            if cid not in chapter_ids:
                problems.append("task %r names the unknown chapter %s" % (t["task"], cid))
        if not t.get("checks"):
            problems.append("task %r names no check" % t["task"])
        for cmd in t.get("checks", []):
            problems += ["task %r: %s" % (t["task"], p) for p in check_command(cmd, root or gather.ROOT)]
    return problems


def load_chapters(plan, directory=MANUAL_DIR):
    """The manual's written chapters, in the order of the plan's M-numbers, each with its blocks and the three fenced kinds read out."""
    chapters = []
    by_id = {c["id"]: c for c in plan["chapters"]}
    if not os.path.isdir(directory):
        return chapters
    for name in sorted(os.listdir(directory)):
        if not (name.endswith(".md") and re.match(r"^M\d\d-", name)):
            continue
        text = read_text(os.path.join(directory, name))
        meta, body = mdlite.front_matter(text, name)
        for k in ("id", "title", "summary"):
            if not meta.get(k):
                raise MdError("%s: front matter needs %s" % (name, k))
        if meta["id"] not in by_id:
            raise MdError("%s: the id %s is not in manual/plan.json" % (name, meta["id"]))
        meta["file"] = name
        meta["section"] = "manual"
        meta["raw"] = body
        meta["prs"] = []
        for key in ("watches", "log", "anatomy"):
            if key in meta and not isinstance(meta[key], list):
                meta[key] = [meta[key]] if meta[key] else []
        # `part` and `n` come from the plan too, so a chapter cannot disagree with it
        for key in ("n", "part"):
            if meta.get(key) not in (None, "") and meta[key] != by_id[meta["id"]][key]:
                raise MdError("%s: %s is %r in the plan and %r in the chapter" % (name, key, by_id[meta["id"]][key], meta[key]))
            meta[key] = by_id[meta["id"]][key]
        from figures import take_blocks
        meta["blocks"], meta["figs"] = take_blocks(mdlite.parse_blocks(body), meta["id"])
        meta["status"], meta["glossary"], meta["try"] = [], [], []
        for b in meta["blocks"]:
            if b["type"] == "code" and b["lang"] == "status":
                meta["status"] += parse_status(b["text"], meta["id"])
            elif b["type"] == "code" and b["lang"] == "glossary":
                meta["glossary"] += parse_glossary(b["text"], meta["id"])
            elif b["type"] == "code" and b["lang"] == "try":
                meta["try"].append(parse_try(b["text"], meta["id"]))
        chapters.append(meta)
    order = {c["id"]: i for i, c in enumerate(plan["chapters"])}
    chapters.sort(key=lambda c: order[c["id"]])
    return chapters


# ---- evidence and state -------------------------------------------------------------------------------------------------------

def known_prs(data):
    """number -> (state, ci) for every pull request the report knows."""
    out = {}
    for p in data.get("prs", []):
        out[p["number"]] = ("merged", p.get("ci"))
    for o in data.get("open", {}).get("prs", []):
        out.setdefault(o["number"], (o["state"], o.get("ci")))
    return out


def git(root, *args):
    return subprocess.run(["git", "-C", root] + list(args), capture_output=True, text=True)


def has_commit(root, rev):
    return git(root, "cat-file", "-e", rev + "^{commit}").returncode == 0


def is_shallow(root):
    return git(root, "rev-parse", "--is-shallow-repository").stdout.strip() == "true"


def file_text(root, rel):
    p = os.path.join(root, rel)
    if not os.path.isfile(p):
        return None
    with open(p, encoding="utf-8", errors="replace") as f:
        return f.read()


def check_item(kind, rest, root, data):
    """None when the evidence item resolves, else what is wrong with it."""
    if kind == "pr":
        if not rest.isdigit():
            return "pr:%s is not a pull request number" % rest
        if int(rest) not in known_prs(data):
            if data.get("shallow"):
                return None  # the pull request history is not in a shallow clone, so the number cannot be looked up
            return "pr:%s is not among the pull requests the report knows" % rest
        return None
    if kind == "commit":
        if not re.match(r"^[0-9a-f]{7,40}$", rest):
            return "commit:%s is not a hexadecimal hash" % rest
        if not has_commit(root, rest) and not is_shallow(root):
            return "commit:%s is not in this repository" % rest
        return None
    if kind == "repo":
        return None if rest and os.path.exists(os.path.join(root, rest)) else "repo:%s does not exist in the tree" % rest
    if kind in ("sym", "test"):
        path, sep, name = rest.partition("::")
        if not sep or not name:
            return "%s:%s needs PATH::NAME" % (kind, rest)
        text = file_text(root, path)
        if text is None:
            return "%s:%s: the file does not exist" % (kind, rest)
        return None if name in text else "%s:%s: %r does not appear in %s" % (kind, rest, name, path)
    if kind == "doc":
        path, sep, anchor = rest.partition("#")
        if not sep or not anchor:
            return "doc:%s needs PATH#ANCHOR" % rest
        text = file_text(root, path)
        if text is None:
            return "doc:%s: the file does not exist" % rest
        heads = [slug(m.group(1)) for m in re.finditer(r"^#{1,6}\s+(.*?)\s*#*\s*$", text, flags=re.M)]
        return None if anchor in heads else "doc:%s: no heading with that anchor" % rest
    return "%s: unknown evidence kind (use one of %s)" % (kind, ", ".join(EVIDENCE_KINDS))


def pr_ran(data, number):
    s = known_prs(data).get(number)
    return bool(s) and s[0] == "merged" and s[1] == "passing"


def run_evidence(rec, data):
    """The evidence items that show something ran: a test, or a merged pull request whose CI passed."""
    out = []
    for kind, rest in rec["items"]:
        if kind == "test" or (kind == "pr" and rest.isdigit() and pr_ran(data, int(rest))):
            out.append((kind, rest))
    return out


def effective_state(rec, data):
    """(state, reason): the state the record may show. A built-verified claim without run evidence drops to built-unverified and says why."""
    if rec["state"] == "built-verified" and not run_evidence(rec, data):
        prs = [int(r) for k, r in rec["items"] if k == "pr" and r.isdigit()]
        if prs:
            why = ", ".join("#%d is %s" % (n, "not merged with CI passing" if n in known_prs(data) else ("not known, because the pull request history is not in this clone" if data.get("shallow") else "not known")) for n in prs)
        else:
            why = "it cites no test and no merged pull request"
        return "built-unverified", "Shown as built, not verified: " + why + "."
    return rec["state"], ""


def check_status(recs, root, data, chapter):
    problems = []
    seen = set()
    for r in recs:
        who = "%s status %s" % (chapter, r["id"])
        if not re.match(r"^[a-z0-9][a-z0-9.-]*$", r["id"]):
            problems.append("%s: the id must be lower case letters, digits, dots and hyphens" % who)
        if r["id"] in seen:
            problems.append("%s: duplicate id" % who)
        seen.add(r["id"])
        if r["state"] not in STATES:
            problems.append("%s: state %r is not one of %s" % (who, r["state"], ", ".join(STATES)))
            continue
        kinds = [k for k, _ in r["items"]]
        if r["state"] in ("built-verified", "built-unverified") and not r["items"]:
            problems.append("%s: a built record needs evidence" % who)
        if r["state"] == "built-verified" and r["items"] and not any(k == "test" or k == "pr" for k in kinds):
            problems.append("%s: built-verified needs run evidence (a test: or a pr: item)" % who)
        if r["state"] == "planned" and not any((k == "doc") or (k == "repo" and rest.startswith("docs/")) for k, rest in r["items"]):
            problems.append("%s: a planned record must cite where it is planned (doc: or repo:docs/...)" % who)
        for kind, rest in r["items"]:
            p = check_item(kind, rest, root, data)
            if p:
                problems.append("%s: %s" % (who, p))
    return problems


# ---- commands -----------------------------------------------------------------------------------------------------------------

def split_segments(cmd):
    """The commands of a shell line as word lists, split on | || && ; outside quotes. A line that cannot be read raises ValueError."""
    lex = shlex.shlex(cmd, posix=True, punctuation_chars=True)
    lex.whitespace_split = True
    segs, cur = [], []
    for w in lex:
        if w in ("|", "||", "&&", ";"):
            segs.append(cur)
            cur = []
        else:
            cur.append(w)
    return segs + [cur]


def check_command(cmd, root):
    """The problems with one command of a try block: it must start with a tool we know or a script that exists in the repository, and an
    interpreter's script must exist as well."""
    problems = []
    cmd = cmd.strip()
    if cmd.startswith("(") and cmd.endswith(")"):
        cmd = cmd[1:-1].strip()
    cwd = root
    m = re.match(r"^cd\s+(\S+)\s*&&\s*(.*)$", cmd)
    if m:
        cwd = os.path.join(root, m.group(1))
        if not os.path.isdir(cwd):
            return ["cd %s: no such directory in the repository" % m.group(1)]
        cmd = m.group(2)
    try:
        segments = split_segments(cmd)
    except ValueError:
        return problems + ["cannot read the command %r" % cmd]
    for words in segments:
        seg = list(words)
        while words and re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", words[0]):
            words.pop(0)
        if not words:
            continue
        first = words[0]
        if first in TOOLS:
            if first in INTERPRETERS:
                script = None
                skip = False
                for w in words[1:]:
                    if skip:
                        skip = False
                        continue
                    if w in ("-c", "-e", "-m"):
                        script = ""
                        break
                    if w.startswith("-"):
                        continue
                    script = w
                    break
                if script is None:
                    problems.append("%s: no script named" % " ".join(seg))
                elif script and not os.path.exists(os.path.join(cwd, script)):
                    problems.append("%s: the script %s does not exist" % (first, script))
            continue
        path = first
        if "/" in path or path.endswith((".sh", ".py")):
            if not os.path.isfile(os.path.join(cwd, path)) and not os.path.isfile(os.path.join(root, path)):
                problems.append("%s: no such script in the repository" % first)
        else:
            problems.append("%s: not a script in the repository or a known tool (%s)" % (first, ", ".join(sorted(TOOLS))))
    return problems


# ---- the checks for a whole chapter -------------------------------------------------------------------------------------------

def sections(c):
    return [b["text"] for b in c["blocks"] if b["type"] == "h" and b["level"] == 2]


def code_rows(c):
    """The rows of the first table under the heading "The code"."""
    blocks, rows = c["blocks"], []
    for i, b in enumerate(blocks):
        if b["type"] == "h" and b["level"] == 2 and b["text"] == "The code":
            for nxt in blocks[i + 1:]:
                if nxt["type"] == "h" and nxt["level"] == 2:
                    break
                if nxt["type"] == "table":
                    rows += nxt["rows"]
    return rows


def pointers(text):
    """The sym: and repo: link targets of a chapter's text: [("sym", "PATH::NAME"), ("repo", "PATH")]."""
    out = []
    for m in re.finditer(r"\]\((sym|repo|test|doc):([^)\s]+)\)", text):
        out.append((m.group(1), m.group(2)))
    return out


def check_chapter(c, plan, root, data, log_ids, boxes):
    """Every problem with one chapter, as a list of strings: the template's checked items."""
    problems = []
    cid = c["id"]
    for k in REQUIRED_FRONT:
        if c.get(k) in (None, "") and k not in ("watches", "log", "anatomy"):
            problems.append("%s: front matter has no %s" % (cid, k))
    for k in ("watches", "log", "anatomy"):
        if k not in c:
            problems.append("%s: front matter has no %s" % (cid, k))
    if not re.match(r"^\d{4}-\d{2}-\d{2}$", c.get("date", "")):
        problems.append("%s: date must be ISO" % cid)
    rev = c.get("reviewed", "")
    if not re.match(r"^[0-9a-f]{7,40}$", rev):
        problems.append("%s: reviewed must be a commit hash" % cid)
    elif has_commit(root, rev):
        if git(root, "merge-base", "--is-ancestor", rev, "HEAD").returncode != 0:
            problems.append("%s: reviewed %s is not an ancestor of HEAD" % (cid, rev))
    elif not is_shallow(root):
        problems.append("%s: reviewed %s is not a commit of this repository" % (cid, rev))
    for p in c.get("watches", []):
        if not os.path.exists(os.path.join(root, p)):
            problems.append("%s: watches %s, which does not exist" % (cid, p))
    heads = sections(c)
    for s in REQUIRED_SECTIONS:
        if s not in heads:
            problems.append("%s: no section %r" % (cid, s))
    if not c.get("status"):
        problems.append("%s: no status block" % cid)
    problems += check_status(c.get("status", []), root, data, cid)
    rows = code_rows(c)
    if not any(re.search(r"\]\((sym|repo):", " ".join(r)) for r in rows):
        problems.append("%s: The code needs at least one row with a sym: or repo: link" % cid)
    for kind, rest in pointers(c["raw"]):
        if kind in ("sym", "test", "doc"):
            p = check_item(kind, rest if kind != "doc" else rest, root, data)
        else:
            p = check_item("repo", rest, root, data)
        if p:
            problems.append("%s: link %s" % (cid, p))
    for i in c.get("log", []):
        if i not in log_ids:
            problems.append("%s: log names %s, which is not a log chapter" % (cid, i))
    for ref in c.get("anatomy", []):
        if ref not in boxes:
            problems.append("%s: anatomy names %s, which is not a box" % (cid, ref))
    if not c.get("try"):
        problems.append("%s: no try block" % cid)
    for t in c.get("try", []):
        if not t["cmds"]:
            problems.append("%s: a try block has no command" % cid)
        if not t["needs"]:
            problems.append("%s: a try block must say what it needs" % cid)
        for cmd in t["cmds"]:
            problems += ["%s: try: %s" % (cid, p) for p in check_command(cmd["cmd"], root)]
    stacks = [b for b in c["blocks"] if b["type"] == "quote" and b["lines"] and b["lines"][0].startswith("**From the stacks")]
    if len(stacks) > 2:
        problems.append("%s: at most two From the stacks sidebars" % cid)
    for b in stacks:
        if not any(l.startswith("Source") for l in b["lines"][1:]):
            problems.append("%s: a sidebar has no Source line" % cid)
        if len(re.findall(r"[.!?](?:\s|$)", b["lines"][0])) > 5:
            problems.append("%s: a sidebar is longer than five sentences" % cid)
    names = {d["name"] + " " + " ".join(d["args"]) for d in c["blocks"] if d["type"] == "directive"}
    for need in ("manual proved", "manual next"):
        if need not in names:
            problems.append("%s: no <!-- %s --> directive" % (cid, need))
    return problems


def check_all(chapters, plan, root, data, log_ids, boxes):
    problems = []
    ids = set()
    for c in chapters:
        problems += check_chapter(c, plan, root, data, log_ids, boxes)
        for r in c.get("status", []):
            if r["id"] in ids:
                problems.append("status id %s is used in more than one chapter" % r["id"])
            ids.add(r["id"])
    problems += check_terms(chapters)
    return problems


def check_terms(chapters):
    """Each term is defined once; each is used (linked with term:ID) at least once; every term: link names a defined term."""
    problems, defined = [], {}
    for c in chapters:
        for t in c.get("glossary", []):
            if t["id"] in defined:
                problems.append("the term %r is defined in %s and in %s" % (t["term"], defined[t["id"]], c["id"]))
            defined[t["id"]] = c["id"]
    used = set()
    for c in chapters:
        for m in re.finditer(r"\]\(term:([^)\s]+)\)", c["raw"]):
            used.add(m.group(1))
            if m.group(1) not in defined:
                problems.append("%s: links to the term %s, which no chapter defines" % (c["id"], m.group(1)))
    for t, cid in defined.items():
        if t not in used:
            problems.append("%s: the term %s is defined and never used" % (cid, t))
    return problems


def terms_of(chapters):
    return {t["id"]: t for c in chapters for t in c.get("glossary", [])}


# ---- state, drawn -------------------------------------------------------------------------------------------------------------

def tag_html(state, reason=""):
    return '<span class="mt mt-%s" title="%s">%s</span>' % (STATE_CLASS[state], esc(reason or STATE_WHAT[state]), esc(STATE_WORD[state]))


def state_values(chapters, data):
    """The `{{state.ID}}` placeholders: the same tag the table shows, so prose and table cannot disagree."""
    out = {}
    for c in chapters:
        for r in c["status"]:
            st, why = effective_state(r, data)
            out["state." + r["id"]] = Raw(tag_html(st, why))
    return out


def state_counts(c, data):
    counts = {s: 0 for s in STATES}
    for r in c["status"]:
        counts[effective_state(r, data)[0]] += 1
    return counts


def counts_text(counts):
    words = (("built-verified", "verified"), ("built-unverified", "not verified"), ("planned", "planned"), ("external", "not ours"))
    return ", ".join("%d %s" % (counts[s], w) for s, w in words if counts[s]) or "no state rows"


def evidence_md(items, data):
    """One evidence list as inline markdown."""
    out = []
    for kind, rest in items:
        if kind == "pr" and rest.isdigit():
            s = known_prs(data).get(int(rest))
            note = "" if not s else (" (merged, CI passing)" if pr_ran(data, int(rest)) else " (%s, CI %s)" % (s[0], s[1] or "unknown"))
            out.append("[#%s](pr:%s)%s" % (rest, rest, note))
        elif kind == "commit":
            out.append("[`%s`](commit:%s)" % (rest[:7], rest))
        elif kind == "repo":
            out.append("[`%s`](repo:%s)" % (rest, rest))
        elif kind in ("sym", "test"):
            path, _, name = rest.partition("::")
            out.append("%s [`%s`](%s:%s) `%s`" % ("test" if kind == "test" else "symbol", path, kind, rest, name))
        elif kind == "doc":
            path, _, anchor = rest.partition("#")
            out.append("[`%s#%s`](doc:%s)" % (path, anchor, rest))
        else:
            out.append("`%s:%s`" % (kind, rest))
    return ", ".join(out)


def render_status(text, chapter, data, ctx):
    recs = parse_status(text, chapter)
    rows = []
    for r in recs:
        st, why = effective_state(r, data)
        note = r.get("note", "")
        if why:
            note = (note + " " if note else "") + why
        rows.append('<tr id="state-%s"><th scope="row">%s<div class="sub"><code>%s</code></div></th><td>%s</td><td>%s</td><td>%s</td></tr>' % (
            esc(r["id"]), inline(r["label"], ctx), esc(r["id"]), tag_html(st, why), inline(evidence_md(r["items"], data), ctx), inline(note, ctx) if note else ""))
    counts = state_counts({"status": recs}, data)
    return ('<div class="tbl m-state"><table><caption class="sr">The state of each mechanism in this layer, with its evidence</caption><thead><tr><th scope="col">Mechanism</th><th scope="col">State</th>'
            '<th scope="col">Evidence</th><th scope="col">Note</th></tr></thead><tbody>%s</tbody></table></div>'
            '<p class="how">%s. &#8220;Verified&#8221; here always means in an emulator or on the host; nothing has run on real hardware. A mechanism is shown as built and verified only when its evidence includes a test or a merged pull request with CI passing.</p>') % (
        "".join(rows), esc(counts_text(counts)))


def render_try(text, chapter, ctx):
    t = parse_try(text, chapter)
    blocks = []
    for c in t["cmds"]:
        out = "\n".join(c["out"])
        blocks.append('<div class="m-cmd"><pre class="term m-term"><code><span class="m-prompt">$ </span>%s%s</code></pre></div>' % (esc(c["cmd"]), ("\n" + esc(out)) if out else ""))
    head = '<p class="m-needs"><b>Needs:</b> %s%s</p>' % (esc(t["needs"]), (" <span class=\"m-ran\">Ran: %s.</span>" % esc(t["ran"])) if t["ran"] else "")
    return '<div class="m-try">%s%s</div>' % (head, "".join(blocks))


def render_glossary(text, chapter, ctx):
    recs = parse_glossary(text, chapter)
    items = "".join('<div id="term-%s"><dt>%s</dt><dd>%s</dd></div>' % (esc(r["id"]), esc(r["term"]), inline(r["def"], ctx)) for r in recs)
    return ('<details class="m-terms"><summary>Terms this chapter defines (%d)</summary><dl>%s</dl></details>' % (len(recs), items))


# ---- staleness ----------------------------------------------------------------------------------------------------------------

def staleness(c, root=None):
    """{"state": "fresh" | "stale" | "unknown", ...}: commits since `reviewed` that touched the paths the chapter watches."""
    root = root or gather.ROOT
    rev = c.get("reviewed", "")
    if not rev or not has_commit(root, rev):
        return {"state": "unknown", "why": "the commit it was reviewed against is not in this clone, which may be a shallow one"}
    watches = c.get("watches") or []
    if not watches:
        return {"state": "fresh", "commits": []}
    r = git(root, "log", "--format=%h%x09%s", "%s..HEAD" % rev, "--", *watches)
    if r.returncode != 0:
        return {"state": "unknown", "why": "git could not read the history"}
    commits = []
    for line in r.stdout.splitlines():
        h, _, s = line.partition("\t")
        m = re.search(r"\(#(\d+)\)\s*$", s)
        commits.append({"hash": h, "subject": s, "pr": int(m.group(1)) if m else None})
    return {"state": "stale" if commits else "fresh", "commits": commits}


def stale_banner(c, stale, ctx):
    if stale["state"] == "unknown":
        return '<p class="m-stale m-stale-unknown"><b>Could not tell whether this chapter is out of date:</b> %s.</p>' % esc(stale["why"])
    if stale["state"] == "fresh":
        return ""
    prs = sorted({x["pr"] for x in stale["commits"] if x["pr"]})
    where = ", ".join("<code>%s</code>" % esc(p) for p in c["watches"])
    return ('<p class="m-stale"><b>May be out of date:</b> %d commit%s since it was reviewed (at <code>%s</code>) touched %s%s.</p>' % (
        len(stale["commits"]), "" if len(stale["commits"]) == 1 else "s", esc(c["reviewed"]), where,
        (" (pull requests %s)" % ", ".join("#%d" % p for p in prs)) if prs else ""))


def report(chapters, data, root=None):
    """The text of `build.py --manual-report`: per chapter, what changed under its watched paths, and what could be promoted."""
    lines = []
    for c in chapters:
        st = staleness(c, root)
        lines.append("%s %s (%s), reviewed at %s" % (c["n"], c["title"], c["id"], c["reviewed"]))
        if st["state"] == "unknown":
            lines.append("  could not tell: " + st["why"])
        elif st["state"] == "fresh":
            lines.append("  no commit has touched %s since" % ", ".join(c["watches"]))
        else:
            for x in st["commits"]:
                lines.append("  %s %s" % (x["hash"], x["subject"]))
        for r in c["status"]:
            eff, why = effective_state(r, data)
            if why:
                lines.append("  status %s is shown as %s: %s" % (r["id"], eff, why))
    return "\n".join(lines)


# ---- explainers of the manual -------------------------------------------------------------------------------------------------

_modules = {}


def explainers(directory=None):
    """The explainer modules of manual/explainers/*.py, by the ID each declares. Discovered, not registered by hand; loaded by path so a
    module of this folder can never shadow one of the generator's."""
    directory = directory or os.path.join(MANUAL_DIR, "explainers")
    out = {}
    if not os.path.isdir(directory):
        return out
    for name in sorted(os.listdir(directory)):
        if not name.endswith(".py") or name.startswith("_"):
            continue
        path = os.path.join(directory, name)
        if path not in _modules:
            spec = importlib.util.spec_from_file_location("manual_explainer_" + name[:-3], path)
            mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(mod)
            _modules[path] = mod
        mod = _modules[path]
        if not hasattr(mod, "ID") or not hasattr(mod, "render"):
            raise MdError("manual/explainers/%s needs ID and render(D, ctx)" % name)
        if mod.ID in out:
            raise MdError("two manual explainers share the id %s" % mod.ID)
        out[mod.ID] = mod.render
    return out


# ---- the pages' pieces --------------------------------------------------------------------------------------------------------

def glossary_page(chapters):
    """The entries of appendix A: every term, with the chapter that defines it and the chapters that use it."""
    terms = terms_of(chapters)
    used = {}
    for c in chapters:
        for m in re.finditer(r"\]\(term:([^)\s]+)\)", c["raw"]):
            used.setdefault(m.group(1), [])
            if c["id"] not in used[m.group(1)]:
                used[m.group(1)].append(c["id"])
    return terms, used


def sources_of(chapters):
    """Every `Source:` line of a sidebar, with its chapter, and every document a chapter's code table or status evidence leans on."""
    side, docs = [], {}
    for c in chapters:
        for b in c["blocks"]:
            if b["type"] == "quote" and b["lines"] and b["lines"][0].startswith("**From the stacks"):
                title = re.match(r"^\*\*From the stacks(?:\s*[:—.-]\s*)?(.*?)\*\*", b["lines"][0])
                for l in b["lines"][1:]:
                    if l.startswith("Source"):
                        side.append((c, (title.group(1).strip(" .:—") if title else ""), l))
        for kind, rest in pointers(c["raw"]):
            path = rest.split("#")[0].split("::")[0]
            if path.startswith("docs/") and path.endswith(".md"):
                docs.setdefault(path, [])
                if c["id"] not in docs[path]:
                    docs[path].append(c["id"])
    return side, docs
