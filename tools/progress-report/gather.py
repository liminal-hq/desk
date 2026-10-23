#!/usr/bin/env python3
# Gathers every number and record the progress report shows, from the git history and tree of `main` and from `gh`.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 tools/progress-report/gather.py [--ref main] [--out data.json] [--offline] [--update-snapshot] [--dry-run]
#
# Standard library only, plus `git` and (optionally) `gh` on the PATH. History comes from `git log` (never from branches that
# have been deleted), counts come from the tree of the ref, and pull request and CI data come from `gh`. When `gh` is missing or
# not signed in the data falls back to snapshots/gh.json (written by --update-snapshot) and says so under "sources". Nothing in
# the repository is written; --dry-run prints a summary and writes no file, and works in a shallow clone.
import datetime
import json
import os
import re
import struct
import subprocess
import sys

USAGE = "usage: gather.py [--ref REF] [--out FILE] [--offline] [--update-snapshot] [--update-captures] [--dry-run]"
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))

# The three drawings of the distorted face, oldest first (all reachable from main).
FACE_COMMITS = ["67f7e9a", "1d094a2", "7e4a127"]
FACE_CP = "\U0001FAEA"


class Repo:
    """Read-only access to one ref of a git repository."""

    def __init__(self, root, ref):
        self.root, self.ref = root, ref
        self._paths = None

    def git(self, *args, check=True):
        p = subprocess.run(["git", *args], cwd=self.root, capture_output=True, text=True, encoding="utf-8", errors="replace")
        if check and p.returncode != 0:
            raise RuntimeError("git %s: %s" % (" ".join(args), p.stderr.strip()))
        return p.stdout

    def show_bytes(self, path, ref=None):
        p = subprocess.run(["git", "show", "%s:%s" % (ref or self.ref, path)], cwd=self.root, capture_output=True)
        if p.returncode != 0:
            return None
        return p.stdout

    def show(self, path, ref=None):
        b = self.show_bytes(path, ref)
        return None if b is None else b.decode("utf-8", errors="replace")

    def blobs(self, paths):
        """{path: bytes} for many files of the ref, read through one `git cat-file --batch` process."""
        out = {}
        if not paths:
            return out
        p = subprocess.run(["git", "cat-file", "--batch"], cwd=self.root, input="".join("%s:%s\n" % (self.ref, x) for x in paths).encode("utf-8"),
                           capture_output=True)
        buf, pos = p.stdout, 0
        for path in paths:
            nl = buf.index(b"\n", pos)
            head = buf[pos:nl].split()
            if len(head) == 3 and head[1] == b"blob":
                size = int(head[2])
                out[path] = buf[nl + 1:nl + 1 + size]
                pos = nl + 1 + size + 1
            else:
                out[path] = None
                pos = nl + 1
        return out

    def paths(self):
        if self._paths is None:
            out = self.git("ls-tree", "-r", "-l", self.ref)
            self._paths = {}
            for line in out.splitlines():
                meta, path = line.split("\t", 1)
                self._paths[path] = int(meta.split()[3]) if meta.split()[3].isdigit() else 0
        return self._paths

    def matching(self, pattern):
        rx = re.compile(pattern)
        return sorted(p for p in self.paths() if rx.search(p))

    def lines(self, pattern):
        return sum((b or b"").count(b"\n") for b in self.blobs(self.matching(pattern)).values())

    def is_shallow(self):
        return self.git("rev-parse", "--is-shallow-repository", check=False).strip() == "true"


def gh_json(*args):
    """Runs `gh` and parses its JSON output, or returns None when gh is missing, signed out or fails."""
    try:
        p = subprocess.run(["gh", *args], capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if p.returncode != 0:
        return None
    try:
        return json.loads(p.stdout)
    except ValueError:
        return None


def slim_pr(p):
    """A pull request as gh returns it, with the check rollup reduced to what the report uses."""
    out = dict(p)
    out["checks"] = [{"name": c.get("name"), "workflow": c.get("workflowName"), "status": c.get("status"), "conclusion": c.get("conclusion")}
                     for c in out.pop("statusCheckRollup", None) or []]
    return out


def ci_of(checks):
    """passing, failing, pending or none, from the checks on a pull request's head commit."""
    if not checks:
        return "none"
    if any(c.get("conclusion") in ("FAILURE", "CANCELLED", "TIMED_OUT") for c in checks):
        return "failing"
    if all(c.get("status") == "COMPLETED" and c.get("conclusion") in ("SUCCESS", "NEUTRAL", "SKIPPED") for c in checks):
        return "passing"
    return "pending"


def snapshot_open_work():
    """The open-work record committed with the gh snapshot, or None."""
    path = os.path.join(HERE, "snapshots", "gh.json")
    if not os.path.exists(path):
        return None
    with open(path, encoding="utf-8") as f:
        return (json.load(f).get("data") or {}).get("openWork")


def gh_available():
    try:
        return subprocess.run(["gh", "auth", "status"], capture_output=True, timeout=30).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


# ---- history -----------------------------------------------------------------------------------------------------------------

def kind_of(path):
    base = os.path.basename(path)
    if path.startswith("fonts/generated/") or path.startswith("assets/"):
        return "generated"
    if (path.startswith("fonts/test/") or path.startswith("tests/") or "/expected/" in path or base.startswith("validate")
            or base in ("native-check.py", "native-test.sh", "run.sh", "qemu-faults.sh") or base.endswith("check.sh")):
        return "tests"
    if path.endswith(".md"):
        return "docs"
    if path.startswith("tools/") or path.startswith(".github/") or path.endswith(".sh") or base.startswith(".git"):
        return "tools"
    return "source"


def parse_conflicts(body):
    """The bullet list under '**Conflicts**' in a merge commit message: [{path, text}]."""
    out, on = [], False
    for line in body.splitlines():
        if line.startswith("**Conflicts**"):
            on = True
            continue
        if on and line.startswith("**"):
            break
        if on and line.startswith("- "):
            text = line[2:].strip()
            out.append({"text": text, "paths": re.findall(r"`([^`]+)`", text.split(":")[0])})
    return out


def history(repo):
    log = repo.git("log", "--first-parent", "--format=%H%x09%h%x09%aI%x09%P%x09%s", repo.ref)
    first_parent, merges = [], []
    for line in log.splitlines():
        full, short, date, parents, subject = line.split("\t", 4)
        entry = {"hash": short, "full": full, "date": date, "subject": subject, "parents": [p[:7] for p in parents.split()]}
        first_parent.append(entry)
        m = re.search(r"\(#(\d+)\)$", subject)
        if m and len(parents.split()) == 2:
            merges.append((int(m.group(1)), entry, parents.split()))
    prs = []
    for number, entry, parents in sorted(merges, key=lambda t: t[1]["date"]):
        base, head = parents
        commits = []
        for line in repo.git("log", "--reverse", "--format=%h%x09%aI%x09%s%x09%P", "%s..%s" % (base, head)).splitlines():
            h, d, s, p = line.split("\t")
            commits.append({"hash": h, "date": d, "subject": s, "merge": len(p.split()) > 1})
        kinds = {}
        added = deleted = 0
        for line in repo.git("diff", "--numstat", base, entry["full"]).splitlines():
            a, d, path = line.split("\t", 2)
            k = kinds.setdefault(kind_of(path), {"files": 0, "added": 0, "binary": 0})
            k["files"] += 1
            if a == "-":
                k["binary"] += 1
            else:
                k["added"] += int(a)
                added += int(a)
                deleted += int(d)
        prs.append({
            "number": number, "title": re.sub(r"\s*\(#\d+\)$", "", entry["subject"]), "mergeCommit": entry["hash"], "mergeFull": entry["full"],
            "mergedAt": entry["date"], "commits": commits, "commitCount": len(commits), "kinds": kinds,
            "added": added, "deleted": deleted,
        })
    # Merges of main into a branch: the conflict-resolution story is in their messages.
    sync = []
    for line in repo.git("log", "--merges", "--format=%H%x09%h%x09%aI%x09%s", repo.ref).splitlines():
        full, short, date, subject = line.split("\t", 3)
        if subject.startswith("Merge main into "):
            body = repo.git("show", "-s", "--format=%b", full)
            conflicts = parse_conflicts(body)
            sync.append({"hash": short, "date": date, "branch": subject[len("Merge main into "):], "conflicts": conflicts,
                         "clean": re.findall(r"`([^`]+)`", " ".join(l for l in body.splitlines() if "merged cleanly" in l)),
                         "body": body.strip()})
    sync.sort(key=lambda s: s["date"])
    return first_parent, prs, sync


# ---- binary and text parsers --------------------------------------------------------------------------------------------------

def sfnt_table(blob, tag):
    n = struct.unpack(">H", blob[4:6])[0]
    for i in range(n):
        t, _, off, length = struct.unpack(">4sIII", blob[12 + 16 * i: 28 + 16 * i])
        if t == tag:
            return off, length
    return None


def glyph_count(blob):
    t = sfnt_table(blob, b"maxp")
    return struct.unpack(">H", blob[t[0] + 4: t[0] + 6])[0] if t else None


def kern_pairs(blob):
    t = sfnt_table(blob, b"kern")
    if not t:
        return 0
    off = t[0]
    ntab = struct.unpack(">HH", blob[off:off + 4])[1]
    total, o = 0, off + 4
    for _ in range(ntab):
        _, length, _, npairs = struct.unpack(">HHHH", blob[o:o + 8])
        total += npairs
        o += length
    return total


def dem_count(blob):
    if not blob or blob[:4] != b"DEM1":
        return None
    return struct.unpack("<H", blob[6:8])[0]


def parse_size(text):
    return int(text.replace(",", ""))


def doc_sizes(d0):
    """Executable sizes the spike author recorded in docs/D0-RUST-DOS.md, read back from its cost table."""
    out = {}
    for key, rx in [
        ("DESK.EXE", r"`DESK\.EXE` ([\d,]+) bytes \(([\d,]+) stripped\)"),
        ("HELLO.EXE", r"`HELLO\.EXE` ([\d,]+) bytes"),
        ("HELLO1.EXE", r"`HELLO1\.EXE` ([\d,]+) bytes \(([\d,]+) stripped\)"),
        ("solo/HELLO.EXE", r"`solo/HELLO\.EXE` ([\d,]+) bytes"),
        ("CWSDSTUB.EXE", r"\(([\d,]+)-byte `CWSDSTUB\.EXE`\)|\(([\d,]+)-byte CWSDSTUB"),
        ("CWSDPMI.EXE", r"`CWSDPMI\.EXE` beside the plain builds is ([\d,]+)"),
    ]:
        m = re.search(rx, d0 or "")
        if m:
            nums = [g for g in m.groups() if g]
            out[key] = parse_size(nums[0])
            if len(nums) > 1:
                out[key + " stripped"] = parse_size(nums[1])
    if "DESK.EXE" not in out:
        m = re.search(r"DESK\.EXE` ([\d,]+) bytes", d0 or "")
        if m:
            out["DESK.EXE"] = parse_size(m.group(1))
    return out


def faces_history(repo):
    pal = {}
    src = repo.show("fonts/src/emoji/palette.ts", FACE_COMMITS[-1]) or ""
    for m in re.finditer(r"letter: '(\w)', hex: '(#[0-9a-f]{6})'", src):
        pal[m.group(1)] = m.group(2)
    out = []
    for c in FACE_COMMITS:
        if repo.git("cat-file", "-t", c, check=False).strip() != "commit":
            continue
        s = repo.show("fonts/src/emoji/pixel/faces.ts", c) or ""
        m = re.search(re.escape("'" + FACE_CP + "': `") + r"\n(.*?)`", s, re.S)
        if not m:
            continue
        rows = [r.strip() for r in m.group(1).strip().splitlines()]
        if len(rows) != 16 or any(len(r) != 16 for r in rows):
            continue
        meta = repo.git("log", "-1", "--format=%aI%x09%s", c).strip().split("\t")
        out.append({"commit": c, "date": meta[0], "subject": meta[1], "grid": rows})
    return {"palette": pal, "drawings": out}


# ---- the tree ------------------------------------------------------------------------------------------------------------------

def tree_facts(repo):
    paths = repo.paths()
    fonts = []
    font_paths = repo.matching(r"^fonts/generated/[^/]+\.(otf|ttf|dfn)$")
    font_blobs = repo.blobs([p for p in font_paths if p.endswith((".otf", ".ttf"))])
    for p in font_paths:
        entry = {"file": os.path.basename(p), "path": p, "size": paths[p]}
        if p.endswith((".otf", ".ttf")):
            blob = font_blobs[p]
            entry["glyphs"] = glyph_count(blob)
            if p.endswith(".ttf"):
                entry["kernPairs"] = kern_pairs(blob)
        fonts.append(entry)
    dem = {}
    for s in (16, 12, 8):
        dem[str(s)] = dem_count(repo.show_bytes("fonts/generated/emoji/emoji%d.dem" % s))
    roadmap = repo.show("docs/EMOJI-ROADMAP.md") or ""
    emoji_doc = repo.show("docs/EMOJI.md") or ""
    m_all = re.search(r"\| All \| (\d+) \| (\d+) \|", roadmap)
    m_req = re.search(r"(\d+) distinct sequences", roadmap)
    m_vec = re.search(r"vector\*\* emoji are (\d+) of the same set", emoji_doc)
    emoji = {
        "pixel": dem.get("16"), "pixel12": dem.get("12"), "pixel8": dem.get("8"),
        "vector": int(m_vec.group(1)) if m_vec else None,
        "requested": int(m_req.group(1)) if m_req else None,
        "requestedDone": int(m_all.group(1)) if m_all else None,
        "requestedPlanned": int(m_all.group(2)) if m_all else None,
    }
    tests = {"bunDeclarations": 0, "bunFiles": 0}
    for p, blob in repo.blobs(repo.matching(r"^fonts/test/.*\.test\.ts$")).items():
        tests["bunFiles"] += 1
        tests["bunDeclarations"] += len(re.findall(r"^\s*(?:test|it)\(", (blob or b"").decode("utf-8", errors="replace"), flags=re.M))
    tests["nativeTranscripts"] = len(repo.matching(r"^tests/native/.*\.txt$"))
    tests["s1Transcripts"] = len(repo.matching(r"^spikes/s1-dxe3/expected/.*\.txt$"))
    tests["d0Transcripts"] = len(repo.matching(r"^spikes/d0-rust-dos/expected/.*\.txt$"))
    run_sh = repo.show("spikes/s1-dxe3/run.sh") or ""
    block = re.search(r"SCENARIOS='\n(.*?)'", run_sh, re.S)
    tests["s1Scenarios"] = len([l for l in (block.group(1) if block else "").splitlines() if l.strip()])
    loc = {
        "skeletonC": repo.lines(r"^c/.*\.[ch]$"),
        "skeletonCoreRs": repo.lines(r"^core/.*\.rs$"),
        "coff2go32Py": repo.lines(r"^tools/coff2go32\.py$"),
        "d0RtRs": repo.lines(r"^spikes/d0-rust-dos/rt/.*\.rs$"),
        "elf2go32Py": repo.lines(r"^tools/elf2go32\.py$"),
        "elf2binPy": repo.lines(r"^tools/elf2bin\.py$"),
        "kernelRs": repo.lines(r"^kernel/src/.*\.rs$"),
        "bootAsm": repo.lines(r"^boot/.*\.asm$"),
        "fontsTs": repo.lines(r"^fonts/src/(?!emoji/).*\.ts$"),
        "emojiTs": repo.lines(r"^fonts/src/emoji/.*\.ts$"),
        "docsMd": repo.lines(r"^docs/.*\.md$"),
    }
    docs = repo.matching(r"^docs/[^/]+\.md$")
    return {
        "files": len(paths), "docs": len(docs), "fonts": fonts, "emoji": emoji, "tests": tests, "loc": loc,
        "docSizes": doc_sizes(repo.show("docs/D0-RUST-DOS.md")),
    }


def transcripts(repo):
    """Raw text of the transcripts the explainers replay, from the tree of the ref."""
    out = {}
    for key, path in [
        ("nativeExpected", "tests/native/expected.txt"),
        ("s1Panic", "spikes/s1-dxe3/expected/panic.txt"),
        ("s1Life", "spikes/s1-dxe3/expected/life.txt"),
        ("s1Leak", "spikes/s1-dxe3/expected/leak.txt"),
        ("s1LeakCrash", "spikes/s1-dxe3/expected/leak-crash.txt"),
        ("s1LeakControl", "spikes/s1-dxe3/expected/leak-control.txt"),
        ("s1FaultDivCatch", "spikes/s1-dxe3/expected/fault-div-catch.txt"),
        ("d0Hello", "spikes/d0-rust-dos/expected/hello.txt"),
        ("d0Hw", "spikes/d0-rust-dos/expected/hw.txt"),
        ("skeleton", None),
    ]:
        if path:
            out[key] = repo.show(path)
    return out


# ---- pull requests and CI from gh ----------------------------------------------------------------------------------------------

def gh_data(offline):
    snap_path = os.path.join(HERE, "snapshots", "gh.json")
    if not offline and gh_available():
        prs = gh_json("pr", "list", "--state", "all", "--limit", "100", "--json",
                      "number,title,headRefName,baseRefName,headRefOid,createdAt,updatedAt,mergedAt,state,isDraft,additions,deletions,changedFiles,labels,url,"
                      "statusCheckRollup")
        runs = gh_json("run", "list", "--branch", "main", "--limit", "200", "--json",
                       "conclusion,headSha,displayTitle,createdAt,url,workflowName,event")
        repo = gh_json("repo", "view", "--json", "description,repositoryTopics,url,visibility")
        if prs is not None and runs is not None:
            return {"prs": [slim_pr(p) for p in prs], "runs": runs, "repo": repo}, "gh", None
    if os.path.exists(snap_path):
        with open(snap_path, encoding="utf-8") as f:
            snap = json.load(f)
        return snap["data"], "snapshot", snap.get("capturedAt")
    return {"prs": [], "runs": [], "repo": None}, "none", None


# ---- open pull requests: work that is on a branch and not merged ---------------------------------------------------------------

REPORT_BRANCH_PREFIX = "docs/progress-report"
TRANSCRIPT_RX = r"^(tests/native/[^/]+\.txt|spikes/[^/]+/expected/[^/]+\.txt)$"


def pr_state(p):
    """The state tag of a pull request from gh: merged, in review (open and ready), in progress (a draft) or closed."""
    if p.get("mergedAt"):
        return "merged"
    if p.get("state") == "OPEN":
        return "in progress" if p.get("isDraft") else "in review"
    return "closed"


def branch_ref(root, name):
    """The remote-tracking ref of a branch if there is one, else the local branch, else None."""
    for cand in ("origin/" + name, name):
        if subprocess.run(["git", "rev-parse", "--verify", "-q", cand + "^{commit}"], cwd=root, capture_output=True).returncode == 0:
            return cand
    return None


def is_ancestor(root, a, b):
    return subprocess.run(["git", "merge-base", "--is-ancestor", a, b], cwd=root, capture_output=True).returncode == 0


def open_work(root, gh_prs):
    """The open pull requests and what their branches hold, read from git. None when a branch is not available (a shallow clone,
    or a branch that was never fetched), in which case the caller uses the snapshot. A branch's own commits leave out the commits of
    every other open branch that it contains, so a stacked pull request is not credited with the work it was built on."""
    opens = sorted((p for p in gh_prs if p.get("state") == "OPEN"), key=lambda p: p["number"])
    main = branch_ref(root, "main")
    if not main:
        return None
    refs = {}
    for p in opens:
        r = branch_ref(root, p["headRefName"])
        if not r:
            return None
        refs[p["number"]] = r
    heads = {n: subprocess.run(["git", "rev-parse", r], cwd=root, capture_output=True, text=True).stdout.strip() for n, r in refs.items()}
    out, warnings = [], []
    for p in opens:
        n, ref = p["number"], refs[p["number"]]
        base = branch_ref(root, p["baseRefName"]) or main
        below = [refs[m] for m in refs if m != n and is_ancestor(root, refs[m], ref)]
        repo = Repo(root, ref)
        own = []
        for line in repo.git("log", "--reverse", "--format=%h%x09%aI%x09%s%x09%P", ref, "^" + base, *["^" + b for b in below]).splitlines():
            h, d, sub, par = line.split("\t")
            own.append({"hash": h, "date": d, "subject": sub, "merge": len(par.split()) > 1})
        if heads[n] != p.get("headRefOid"):
            warnings.append("the local ref of PR #%d is at %s but gh says its head is %s: fetch before building" % (n, heads[n][:7], (p.get("headRefOid") or "?")[:7]))
        out.append({
            "number": n, "title": p["title"], "branch": p["headRefName"], "base": p["baseRefName"], "head": heads[n][:7], "headFull": heads[n],
            "state": pr_state(p), "ci": ci_of(p.get("checks")), "createdAt": p.get("createdAt"), "updatedAt": p.get("updatedAt"), "url": p.get("url"),
            "labels": [l["name"] for l in p.get("labels", [])], "ghAdditions": p.get("additions"), "ghDeletions": p.get("deletions"),
            "ghChangedFiles": p.get("changedFiles"), "commits": own, "commitCount": len(own),
            "ahead": int(repo.git("rev-list", "--count", ref, "^" + main).strip() or 0), "stackedOn": [int(m) for m in refs if m != n and is_ancestor(root, refs[m], ref)],
            "isReport": p["headRefName"].startswith(REPORT_BRANCH_PREFIX),
        })
    subject = [o for o in out if not o["isReport"]]
    tips = []
    for o in subject:
        repo = Repo(root, refs[o["number"]])
        tests = sum(len(re.findall(rb"^\s*#\[test\]", b or b"", flags=re.M)) for b in repo.blobs(repo.matching(r"^hal/.*\.rs$")).values())
        tips.append({"number": o["number"], "hostTests": tests, "transcripts": len(repo.matching(TRANSCRIPT_RX)), "halLines": repo.lines(r"^hal/.*\.rs$"),
                     "kernelLines": repo.lines(r"^kernel/src/.*\.rs$")})
    union = ""
    if subject:
        union = subprocess.run(["git", "rev-list", "--count", *[refs[o["number"]] for o in subject], "^" + main], cwd=root, capture_output=True, text=True).stdout.strip()
    best = max(tips, key=lambda t: t["hostTests"]) if tips else None
    return {"prs": out, "tips": tips, "commitsOnBranches": int(union or 0), "widest": best["number"] if best else None, "warnings": warnings}


# ---- captures: the few files of other branches that the explainers replay ------------------------------------------------------

def extract(text, spec):
    """The part of a text file a capture keeps: all of it, the Nth fenced block after a marker line, or the first table after one."""
    if not spec:
        return text
    lines = text.splitlines()
    start = next((i for i, l in enumerate(lines) if spec["after"] in l), None)
    if start is None:
        return None
    if "fence" in spec:
        seen, i = 0, start
        while i < len(lines):
            if lines[i].startswith("```"):
                seen += 1
                j = i + 1
                while j < len(lines) and not lines[j].startswith("```"):
                    j += 1
                if seen == spec["fence"]:
                    return "\n".join(lines[i + 1:j]) + "\n"
                i = j
            i += 1
        return None
    if "table" in spec:
        i = start
        while i < len(lines) and not lines[i].startswith("|"):
            i += 1
        j = i
        while j < len(lines) and lines[j].startswith("|"):
            j += 1
        return "\n".join(lines[i:j]) + "\n" if j > i else None
    return None


def load_captures():
    path = os.path.join(HERE, "media", "captures.json")
    return read_json(path) if os.path.exists(path) else []


def capture_source(root, c):
    """(bytes or None, commit) of the capture's source on its branch, or (None, None) when the branch is not available."""
    ref = branch_ref(root, c["branch"])
    if not ref:
        return None, None
    repo = Repo(root, ref)
    blob = repo.show_bytes(c["path"])
    if blob is None:
        return None, repo.git("rev-parse", "--short", ref).strip()
    if c.get("extract"):
        text = extract(blob.decode("utf-8", errors="replace"), c["extract"])
        blob = text.encode("utf-8") if text is not None else None
    return blob, repo.git("rev-parse", "--short", ref).strip()


def check_captures(root):
    """Compares each committed capture with its source on the branch it came from: matches, differs, or not checked."""
    out = []
    for c in load_captures():
        mine = os.path.join(HERE, "media", c["file"])
        have = None
        if os.path.exists(mine):
            with open(mine, "rb") as f:
                have = f.read()
        src, head = capture_source(root, c)
        if have is None:
            status = "missing"
        elif head is None:
            status = "not checked"
        elif src == have:
            status = "matches"
        else:
            status = "differs"
        out.append({"file": c["file"], "branch": c["branch"], "path": c["path"], "commit": c.get("commit"), "branchHead": head, "status": status})
    return out


def update_captures(root):
    caps = load_captures()
    for c in caps:
        src, head = capture_source(root, c)
        if src is None:
            raise SystemExit("cannot capture %s: %s:%s is not available" % (c["file"], c["branch"], c["path"]))
        with open(os.path.join(HERE, "media", c["file"]), "wb") as f:
            f.write(src)
        c["commit"] = head
    with open(os.path.join(HERE, "media", "captures.json"), "w", encoding="utf-8") as f:
        json.dump(caps, f, indent=1)
        f.write("\n")


# ---- the whole record ----------------------------------------------------------------------------------------------------------

def read_json(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def load_roadmap():
    return read_json(os.path.join(HERE, "roadmap.json"))


def resolve_roadmap(roadmap, prs, open_prs=()):
    """Checks every node against what is merged and what is open, and gives each a state tag from gh. A node that claims pull requests
    may keep a 'done' or 'done-emu' state only if every one is merged with CI passing on its merge commit. If they are open, it is
    'review'; if any cannot be found it drops to 'progress'. The tag is merged, in review, in progress or planned. Returns the
    per-branch progress, and any warnings."""
    by_num = {p["number"]: p for p in prs}
    open_by = {p["number"]: p for p in open_prs}
    warnings = []
    for n in roadmap["nodes"]:
        nums = n.get("prs") or ([n["pr"]] if n.get("pr") else [])
        n["prs"] = nums
        n["merged"] = bool(nums) and all(k in by_num for k in nums)
        tags = []
        for k in nums:
            tags.append("merged" if k in by_num else (open_by[k]["state"] if k in open_by else "unknown"))
        if nums:
            n["ci"] = "passing" if all((by_num.get(k) or open_by.get(k) or {}).get("ci") == "passing" for k in nums) else "not all passing"
            if "unknown" in tags:
                if n["state"] in ("done", "done-emu", "review"):
                    warnings.append("node %s drops to progress: PR #%s was not found" % (n["id"], ", #".join(str(k) for k in nums if k not in by_num and k not in open_by)))
                    n["state"] = "progress"
            elif n["merged"]:
                if n["state"] in ("done", "done-emu") and n["ci"] != "passing":
                    warnings.append("node %s drops to progress: its pull requests are merged but CI did not pass on every merge commit" % n["id"])
                    n["state"] = "progress"
                if n["state"] == "review":
                    warnings.append("node %s is marked review but its pull requests are merged" % n["id"])
            else:
                if n["state"] in ("done", "done-emu"):
                    warnings.append("node %s drops to review: PR #%s is open, not merged" % (n["id"], ", #".join(str(k) for k in nums if k in open_by)))
                    n["state"] = "review"
                elif n["ci"] != "passing" and n["state"] == "review":
                    warnings.append("node %s is in review but CI is not passing on every open pull request" % n["id"])
        if "in progress" in tags:
            n["tag"] = "in progress"
        elif "in review" in tags:
            n["tag"] = "in review"
        elif nums and n["merged"]:
            n["tag"] = "merged"
        elif nums:
            n["tag"] = "planned"
        elif n["state"] in ("done", "done-emu"):
            n["tag"] = "merged"
        else:
            n["tag"] = n.get("work", "planned")
    progress = {}
    for n in roadmap["nodes"]:
        if n["branch"] != "root":
            progress.setdefault(n["branch"], {}).setdefault(n["state"], 0)
            progress[n["branch"]][n["state"]] += 1
    return progress, warnings


def local_date(iso):
    """The calendar date of a timestamp in the project's own time zone, where merge day began and ended."""
    d = datetime.datetime.fromisoformat(iso.replace("Z", "+00:00"))
    try:
        from zoneinfo import ZoneInfo
        d = d.astimezone(ZoneInfo("America/Toronto"))
    except Exception:
        d = d.astimezone(datetime.timezone(datetime.timedelta(hours=-4)))
    return d.date().isoformat()


def metric(value, label, how):
    return {"value": value, "label": label, "how": how}


def gather(root=ROOT, ref="main", offline=False, dry_run=False):
    # `main` means the remote's main when there is one: the local branch of a worktree-based checkout can be behind what gh reports.
    repo = Repo(root, (branch_ref(root, "main") or "main") if ref == "main" else ref)
    if repo.git("rev-parse", "--verify", repo.ref, check=False).strip() == "":
        raise SystemExit("no such ref: %s" % ref)
    first_parent, prs, sync = history(repo)
    facts = tree_facts(repo)
    gh, source, captured = gh_data(offline or dry_run)
    gh_by_num = {p["number"]: p for p in gh["prs"]}
    ci_by_sha = {}
    for r in gh["runs"]:
        if r.get("workflowName") == "CI" or r.get("workflowName") is None:
            ci_by_sha.setdefault(r["headSha"], r["conclusion"])
    for p in prs:
        g = gh_by_num.get(p["number"], {})
        p["createdAt"] = g.get("createdAt")
        p["labels"] = [l["name"] for l in g.get("labels", [])]
        p["url"] = g.get("url")
        p["branch"] = g.get("headRefName")
        p["base"] = g.get("baseRefName")
        p["ghAdditions"], p["ghDeletions"], p["ghChangedFiles"] = g.get("additions"), g.get("deletions"), g.get("changedFiles")
        concl = ci_by_sha.get(p["mergeFull"])
        p["ci"] = "passing" if concl == "success" else ("failing" if concl else "unknown")
    work = open_work(root, gh["prs"]) if gh["prs"] else None
    work_source = "git"
    if work is None:
        # a branch is not fetched here: the open work recorded with the snapshot stands in, even when gh itself answered live
        work, work_source = gh.get("openWork") or snapshot_open_work(), "snapshot"
    if work is None:
        work, work_source = {"prs": [], "tips": [], "commitsOnBranches": 0, "widest": None, "warnings": []}, "none"
    captures = check_captures(root)
    roadmap = load_roadmap()
    subject_open = [o for o in work["prs"] if not o["isReport"]]
    progress, warnings = resolve_roadmap(roadmap, prs, work["prs"])
    warnings += work.get("warnings", [])
    warnings += ["capture %s differs from %s:%s" % (c["file"], c["branch"], c["path"]) for c in captures if c["status"] in ("differs", "missing")]
    now = datetime.datetime.now().astimezone().isoformat(timespec="seconds")
    day0 = first_parent[-1] if first_parent else None
    merged_ci_ok = sum(1 for p in prs if p["ci"] == "passing")
    # Merge day: the pull requests merged on the local date of the first merge. The early chapters describe that day, and a later merge
    # (the report's own generator) must not change what they say.
    md_date = local_date(prs[0]["mergedAt"]) if prs else None
    md_prs = [p for p in prs if local_date(p["mergedAt"]) == md_date]
    since = [p for p in prs if p not in md_prs]
    md_last = md_prs[-1]["mergeFull"] if md_prs else None
    md_first_parent = int(repo.git("rev-list", "--count", "--first-parent", md_last).strip()) if md_last else 0
    md_all = int(repo.git("rev-list", "--count", md_last).strip()) if md_last else 0
    md_ci = sum(1 for p in md_prs if p["ci"] == "passing")
    mpath = os.path.join(HERE, "snapshots", "measured.json")
    measured = read_json(mpath) if os.path.exists(mpath) else {}
    emoji, loc, tests = facts["emoji"], facts["loc"], facts["tests"]
    metrics = {
        "prs": metric(len(prs), "pull requests merged", "Merge commits on the first-parent line of %s whose subject ends in (#N), from `git log --first-parent`." % ref),
        "commitsFirst": metric(len(first_parent), "commits on the first-parent line", "`git log --first-parent --oneline %s | wc -l`: the initial commit plus one merge per pull request." % ref),
        "commitsAll": metric(int(repo.git("rev-list", "--count", repo.ref).strip() or 0), "commits in all", "`git rev-list --count %s`, which includes every commit inside the merged branches." % ref),
        "ciGreen": metric(("%d of %d" % (merged_ci_ok, len(prs))) if source != "none" else "unknown", "merge commits with CI green", "The CI workflow's conclusion on each merge commit, from `gh run list --branch main` (source: %s)." % source),
        "files": metric(facts["files"], "files in the tree", "`git ls-tree -r %s | wc -l`." % ref),
        "docs": metric(facts["docs"], "design and result documents", "Markdown files directly under docs/ in the tree of %s." % ref),
        "emojiPixel": metric(emoji["pixel"], "pixel emoji", "The emoji count in the header of fonts/generated/emoji/emoji16.dem (bytes 6 and 7)."),
        "emojiVector": metric(emoji["vector"], "vector emoji", "The count stated in docs/EMOJI.md, which a test in fonts/test checks against the font build; not recomputed here."),
        "fontFiles": metric(len([f for f in facts["fonts"] if f["file"].endswith((".otf", ".ttf"))]), "outline and pixel font files", "OpenType and TrueType files under fonts/generated/ in the tree."),
        "bunTests": metric(tests["bunDeclarations"], "font and emoji test declarations", "A count of test( and it( declarations in fonts/test/*.test.ts, not a test runner's total: loops inside a test can run a declaration many times."),
        "transcripts": metric(tests["nativeTranscripts"] + tests["s1Transcripts"] + tests["d0Transcripts"], "expected transcripts", "Files under tests/native, spikes/s1-dxe3/expected and spikes/d0-rust-dos/expected: each is compared byte for byte with a run."),
        "s1Scenarios": metric(tests["s1Scenarios"], "DOSBox module scenarios", "Lines in the SCENARIOS table of spikes/s1-dxe3/run.sh."),
        "kernelLines": metric(loc["kernelRs"], "lines of kernel Rust", "Newlines in kernel/src/*.rs in the tree."),
        "rtLines": metric(loc["d0RtRs"], "lines of rt (Rust runtime for DOS)", "Newlines in spikes/d0-rust-dos/rt/**/*.rs in the tree."),
        "bootLines": metric(loc["bootAsm"], "lines of boot assembly", "Newlines in boot/*.asm in the tree."),
        "packerLines": metric(loc["elf2go32Py"], "lines in the ELF packer", "Newlines in tools/elf2go32.py in the tree."),
        "docLines": metric(loc["docsMd"], "lines of design prose", "Newlines in docs/*.md in the tree."),
        "prsOpen": metric(len(subject_open), "pull requests open", "Open pull requests from `gh pr list`, not counting the pull requests that carry this report. Each is CI-green or not as `gh` says; none is merged."),
        "openCi": metric(("%d of %d" % (sum(1 for o in subject_open if o["ci"] == "passing"), len(subject_open))) if work_source != "none" else "unknown", "open pull requests with CI green", "The conclusion of every check on the head commit of each open pull request, from `gh pr list` (statusCheckRollup)."),
        "commitsBranches": metric(work["commitsOnBranches"], "commits on open branches, not on main", "`git rev-list --count HEAD1 HEAD2 … ^main` over the heads of the open pull requests (the report's own excluded): each commit counted once, even when branches share it."),
        "hostTests": metric(max([t["hostTests"] for t in work["tips"]] or [0]), "Rust host tests at the widest open branch", "Lines that are `#[test]` under hal/ in the tree of the open branch with the most of them (pull request #%s). It is a count of declarations; the pull request's own document states the same figure from `cargo test`." % work["widest"]),
        "branchTranscripts": metric(max([t["transcripts"] for t in work["tips"]] or [0]), "expected transcripts at the widest open branch", "Files under tests/native and spikes/*/expected in the tree of the open branch with the most; this includes the ones already on main."),
        "halLines": metric(max([t["halLines"] for t in work["tips"] if t["number"] == work["widest"]] or [0]), "lines of Rust in hal/ (open branch)", "Newlines in hal/**/*.rs in the tree of the open branch with the most host tests."),
        "prsMergeDay": metric(len(md_prs), "pull requests merged on merge day", "Merge commits on main whose local date is the date of the first merge, from `git log --first-parent`."),
        "commitsFirstMergeDay": metric(md_first_parent, "commits on the first-parent line at the end of merge day", "`git rev-list --count --first-parent` at the last merge of merge day."),
        "commitsMergeDay": metric(md_all, "commits in all at the end of merge day", "`git rev-list --count` at the last merge of merge day."),
        "ciGreenMergeDay": metric("%d of %d" % (md_ci, len(md_prs)), "merge-day merge commits with CI green", "The CI workflow's conclusion on each merge commit of merge day, from `gh run list --branch main` (source: %s)." % source),
        "daySpan": metric(None, "hours from first commit to last merge", "Author dates of the first commit and the last merge commit on main."),
    }
    if day0 and first_parent:
        a = datetime.datetime.fromisoformat(day0["date"])
        b = datetime.datetime.fromisoformat(first_parent[0]["date"])
        metrics["daySpan"]["value"] = round((b - a).total_seconds() / 3600, 1)
    # Numbers scraped from prose documents fail soft by nature; say so loudly when a document's wording has moved.
    missing = [k for k, m in metrics.items() if m["value"] is None]
    missing += ["docSizes." + k for k in ("DESK.EXE", "HELLO.EXE", "CWSDPMI.EXE", "solo/HELLO.EXE", "CWSDSTUB.EXE") if k not in facts["docSizes"]
                and repo.show("docs/D0-RUST-DOS.md") is not None]
    missing += ["emoji." + k for k in ("pixel", "vector", "requested", "requestedDone") if emoji.get(k) is None and repo.show("docs/EMOJI.md") is not None]
    return {
        "generated": now, "ref": ref, "head": first_parent[0]["hash"] if first_parent else None,
        "shallow": repo.is_shallow(),
        "sources": {"gh": source, "ghCapturedAt": captured, "offline": bool(offline or dry_run)},
        "repo": gh.get("repo"), "main": {"log": first_parent, "files": facts["files"]},
        "prs": prs, "sync": sync, "fonts": facts["fonts"], "emoji": emoji, "tests": tests, "loc": loc,
        "docSizes": facts["docSizes"], "measured": measured, "faces": faces_history(repo), "transcripts": transcripts(repo),
        "roadmap": roadmap, "progress": progress, "warnings": warnings, "missing": sorted(set(missing)), "metrics": metrics,
        "open": work, "openSource": work_source, "captures": captures,
        "mergeDay": {"date": md_date, "numbers": [p["number"] for p in md_prs], "since": [p["number"] for p in since]},
    }


def summary(data):
    lines = ["ref %s at %s (%s)" % (data["ref"], data["head"], "shallow clone" if data["shallow"] else "full history"),
             "gh data: %s" % data["sources"]["gh"], "pull requests merged: %d" % len(data["prs"]),
             "files in tree: %d" % data["main"]["files"],
             "open pull requests: %s (from %s)" % (", ".join("#%d" % o["number"] for o in data["open"]["prs"]), data["openSource"])]
    for k, m in data["metrics"].items():
        lines.append("  %-12s %-8s %s" % (k, m["value"], m["label"]))
    lines += ["warning: " + w for w in data["warnings"]]
    lines += ["MISSING: " + m for m in data["missing"]]
    return "\n".join(lines)


def main(argv):
    args = argv[1:]
    opts = {"--ref": "main", "--out": None}
    flags = set()
    i = 0
    while i < len(args):
        if args[i] in opts and i + 1 < len(args):
            opts[args[i]] = args[i + 1]
            i += 2
        elif args[i] in ("--offline", "--update-snapshot", "--update-captures", "--dry-run"):
            flags.add(args[i])
            i += 1
        else:
            print(USAGE, file=sys.stderr)
            return 2
    dry = "--dry-run" in flags
    ref = opts["--ref"]
    if dry and ref == "main":
        # The dry run reads whatever is checked out, so it also works in a CI clone that has no local `main`.
        ref = "HEAD"
    offline = "--offline" in flags
    if "--update-captures" in flags:
        update_captures(ROOT)
        print("captures updated from the branches; commit the files under tools/progress-report/media/")
        return 0
    if "--update-snapshot" in flags:
        if not gh_available():
            print("cannot update the snapshot: gh is not available or not signed in", file=sys.stderr)
            return 1
        fresh, source, _ = gh_data(False)
        if source != "gh":
            print("cannot update the snapshot: the gh queries failed", file=sys.stderr)
            return 1
        fresh["openWork"] = open_work(ROOT, fresh["prs"])
        if fresh["openWork"] is None:
            print("cannot update the snapshot: a branch of an open pull request is not available (git fetch origin)", file=sys.stderr)
            return 1
        with open(os.path.join(HERE, "snapshots", "gh.json"), "w", encoding="utf-8") as f:
            json.dump({"capturedAt": datetime.datetime.now().astimezone().isoformat(timespec="seconds"), "data": fresh}, f, indent=1, sort_keys=True)
            f.write("\n")
        offline = True  # the data below is read back from the snapshot just written, so the two cannot differ
    data = gather(ROOT, ref, offline=offline, dry_run=dry)
    if dry:
        print(summary(data))
        return 1 if data["missing"] else 0
    text = json.dumps(data, indent=1, ensure_ascii=False)
    if opts["--out"]:
        os.makedirs(os.path.dirname(os.path.abspath(opts["--out"])), exist_ok=True)
        with open(opts["--out"], "w", encoding="utf-8") as f:
            f.write(text + "\n")
    else:
        print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
