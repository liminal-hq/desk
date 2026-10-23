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

USAGE = "usage: gather.py [--ref REF] [--out FILE] [--offline] [--update-snapshot] [--dry-run]"
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
                      "number,title,headRefName,baseRefName,createdAt,mergedAt,additions,deletions,changedFiles,labels,url")
        runs = gh_json("run", "list", "--branch", "main", "--limit", "200", "--json",
                       "conclusion,headSha,displayTitle,createdAt,url,workflowName,event")
        repo = gh_json("repo", "view", "--json", "description,repositoryTopics,url,visibility")
        if prs is not None and runs is not None:
            return {"prs": prs, "runs": runs, "repo": repo}, "gh", None
    if os.path.exists(snap_path):
        with open(snap_path, encoding="utf-8") as f:
            snap = json.load(f)
        return snap["data"], "snapshot", snap.get("capturedAt")
    return {"prs": [], "runs": [], "repo": None}, "none", None


# ---- the whole record ----------------------------------------------------------------------------------------------------------

def read_json(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def load_roadmap():
    return read_json(os.path.join(HERE, "roadmap.json"))


def resolve_roadmap(roadmap, prs):
    """Checks every node against what is merged. A node that claims a PR must have that PR merged with CI passing on the
    merge commit, or it drops to 'progress'. Returns the roadmap with the evidence the check used, and any warnings."""
    by_num = {p["number"]: p for p in prs}
    warnings = []
    for n in roadmap["nodes"]:
        pr = by_num.get(n.get("pr"))
        if n.get("pr"):
            n["merged"] = bool(pr)
            n["ci"] = pr.get("ci") if pr else "not merged"
            if n["state"] in ("done", "done-emu") and not (pr and pr.get("ci") == "passing"):
                warnings.append("node %s drops to progress: PR #%s is not merged with CI passing" % (n["id"], n["pr"]))
                n["state"] = "progress"
    progress = {}
    for n in roadmap["nodes"]:
        if n["branch"] != "root":
            progress.setdefault(n["branch"], {}).setdefault(n["state"], 0)
            progress[n["branch"]][n["state"]] += 1
    return progress, warnings


def metric(value, label, how):
    return {"value": value, "label": label, "how": how}


def gather(root=ROOT, ref="main", offline=False, dry_run=False):
    repo = Repo(root, ref)
    if repo.git("rev-parse", "--verify", ref, check=False).strip() == "":
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
    roadmap = load_roadmap()
    progress, warnings = resolve_roadmap(roadmap, prs)
    now = datetime.datetime.now().astimezone().isoformat(timespec="seconds")
    day0 = first_parent[-1] if first_parent else None
    merged_ci_ok = sum(1 for p in prs if p["ci"] == "passing")
    mpath = os.path.join(HERE, "snapshots", "measured.json")
    measured = read_json(mpath) if os.path.exists(mpath) else {}
    emoji, loc, tests = facts["emoji"], facts["loc"], facts["tests"]
    metrics = {
        "prs": metric(len(prs), "pull requests merged", "Merge commits on the first-parent line of %s whose subject ends in (#N), from `git log --first-parent`." % ref),
        "commitsFirst": metric(len(first_parent), "commits on the first-parent line", "`git log --first-parent --oneline %s | wc -l`: the initial commit plus one merge per pull request." % ref),
        "commitsAll": metric(int(repo.git("rev-list", "--count", ref).strip() or 0), "commits in all", "`git rev-list --count %s`, which includes every commit inside the merged branches." % ref),
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
    }


def summary(data):
    lines = ["ref %s at %s (%s)" % (data["ref"], data["head"], "shallow clone" if data["shallow"] else "full history"),
             "gh data: %s" % data["sources"]["gh"], "pull requests merged: %d" % len(data["prs"]),
             "files in tree: %d" % data["main"]["files"]]
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
        elif args[i] in ("--offline", "--update-snapshot", "--dry-run"):
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
    if "--update-snapshot" in flags:
        if not gh_available():
            print("cannot update the snapshot: gh is not available or not signed in", file=sys.stderr)
            return 1
        fresh, source, _ = gh_data(False)
        if source != "gh":
            print("cannot update the snapshot: the gh queries failed", file=sys.stderr)
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
