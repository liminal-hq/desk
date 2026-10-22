#!/usr/bin/env python3
# Checks the markdown house style (no hard-wrapped paragraphs, no "--" standing in for an em dash, Canadian spelling of the
# common words) and that relative links and image sources resolve.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tools/check-docs.py
#
# Covers every tracked or new markdown file outside design/, vendor/, target/, dist/ and node_modules/. Code fences, inline code,
# tables, list items, HTML lines and YAML front matter are not treated as paragraphs or checked for spelling.
import os
import re
import subprocess
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
SKIP = ("design/", "vendor/", "target/", "dist/", "node_modules/")
US_SPELLINGS = {
    "color": "colour", "colors": "colours", "behavior": "behaviour", "center": "centre", "centered": "centred",
    "neighbor": "neighbour", "canceled": "cancelled", "gray": "grey", "favorite": "favourite", "organize": "organise",
}


def files():
    out = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "--", "*.md"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout.split("\n")
    return [f for f in out if f and not f.startswith(SKIP)]


def prose(line):
    s = line.strip()
    return bool(s) and not re.match(r"^([-*+>|#]|\d+[.)]\s|<|\[.*\]:\s|```)", s)


problems = []
for rel in files():
    path = os.path.join(ROOT, rel)
    in_code = False
    prev_prose = False
    front = False
    for n, line in enumerate(open(path, encoding="utf-8"), 1):
        line = line.rstrip("\n")
        if n == 1 and line == "---":
            front = True
            continue
        if front:
            front = line != "---"
            continue
        if line.lstrip().startswith("```"):
            in_code = not in_code
            prev_prose = False
            continue
        if in_code:
            continue
        plain = re.sub(r"`[^`]*`", "", line)
        html = line.lstrip().startswith("<")
        if html:
            plain = ""
        if prose(line) and prev_prose and not line.endswith("  "):
            problems.append(f"{rel}:{n}: hard-wrapped paragraph (write each paragraph on one line)")
        prev_prose = prose(line)
        if re.search(r"\w --? \w|\w--\w", plain) and not re.search(r"\s--[a-z]", plain):
            problems.append(f"{rel}:{n}: use a real em dash (—), not --")
        for word in re.findall(r"[A-Za-z]+", re.sub(r"\(.*?\)|https?://\S+", "", plain)):
            if word.lower() in US_SPELLINGS:
                problems.append(f"{rel}:{n}: Canadian spelling: {word} -> {US_SPELLINGS[word.lower()]}")
        for target in re.findall(r"\]\(([^)\s#]+)(?:#[^)]*)?\)", line):
            if re.match(r"^[a-z]+:", target):
                continue
            if not os.path.exists(os.path.normpath(os.path.join(os.path.dirname(path), target))):
                problems.append(f"{rel}:{n}: broken link: {target}")
        for target in re.findall(r'<img[^>]*\ssrc="([^"]+)"', line):
            if not re.match(r"^[a-z]+:", target) and not os.path.exists(os.path.normpath(os.path.join(os.path.dirname(path), target))):
                problems.append(f"{rel}:{n}: missing image: {target}")

if problems:
    print("\n".join(problems), file=sys.stderr)
    sys.exit(1)
print("markdown style and links: ok")
