#!/usr/bin/env python3
# Compares a serial transcript from the native kernel with an expected transcript, line by line.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tools/native-check.py EXPECTED ACTUAL
#
# Expected-file syntax, one line per transcript line (carriage returns and trailing spaces are ignored):
#   plain text    must match exactly
#   {N:lo..hi}    a decimal number in the range, inclusive (memory totals, entry counts, line numbers)
#   {HEX}         a hex number written with a 0x prefix (addresses)
#   {BITS:m=v}    a hex number written with a 0x prefix whose bits under the mask m equal v (both hex, 0x prefix), for a register
#                 whose other bits depend on the machine (CR0's cache-disable bits are set by a 486's reset and cleared by SeaBIOS)
#   {ANY}         any text
#   ...           on a line of its own: zero or more lines of any content (the BIOS-dependent memory map, for example)
# Exit status 0 when the transcript matches, 1 when it does not (with the first line that could not be matched).
import re
import sys

TOKEN = re.compile(r"\{N:(\d+)\.\.(\d+)\}|\{BITS:(0x[0-9a-fA-F]+)=(0x[0-9a-fA-F]+)\}|\{HEX\}|\{ANY\}")


def compile_line(text):
    """Returns (regex, checks) for one expected line; a check takes the text a capture group matched and says whether it is allowed."""
    out, checks, pos = [], [], 0
    for m in TOKEN.finditer(text):
        out.append(re.escape(text[pos:m.start()]))
        if m.group(0).startswith("{N:"):
            out.append(r"(\d+)")
            lo, hi = int(m.group(1)), int(m.group(2))
            checks.append(lambda t, lo=lo, hi=hi: lo <= int(t) <= hi)
        elif m.group(0).startswith("{BITS:"):
            out.append(r"(0x[0-9a-fA-F]+)")
            mask, value = int(m.group(3), 16), int(m.group(4), 16)
            checks.append(lambda t, mask=mask, value=value: int(t, 16) & mask == value)
        elif m.group(0) == "{HEX}":
            out.append(r"0x[0-9a-fA-F]+")
        else:
            out.append(r".*")
        pos = m.end()
    out.append(re.escape(text[pos:]))
    return re.compile("^" + "".join(out) + "$"), checks


def line_matches(pattern, actual):
    regex, checks = pattern
    m = regex.match(actual)
    return bool(m) and all(check(g) for g, check in zip(m.groups(), checks))


def clean(path):
    return [l.rstrip("\r\n").rstrip() for l in open(path, encoding="utf-8", errors="replace")]


def main(expected_path, actual_path):
    expected = [None if l == "..." else compile_line(l) for l in clean(expected_path)]
    actual = clean(actual_path)
    # Longest prefix of the expected lines that could be matched, for the error message.
    best = [0, 0]
    seen = set()

    def walk(e, a):
        if (e, a) in seen:
            return False
        seen.add((e, a))
        if e > best[0] or (e == best[0] and a > best[1]):
            best[0], best[1] = e, a
        if e == len(expected):
            return a == len(actual)
        if expected[e] is None:
            return any(walk(e + 1, k) for k in range(a, len(actual) + 1))
        return a < len(actual) and line_matches(expected[e], actual[a]) and walk(e + 1, a + 1)

    sys.setrecursionlimit(10000)
    if walk(0, 0):
        print(f"transcript matches {expected_path} ({len(actual)} lines)")
        return 0
    e, a = best
    print(f"transcript does not match {expected_path}", file=sys.stderr)
    exp_line = clean(expected_path)[e] if e < len(expected) else "(end of expected)"
    act_line = actual[a] if a < len(actual) else "(end of transcript)"
    print(f"  expected line {e + 1}: {exp_line}", file=sys.stderr)
    print(f"  actual line   {a + 1}: {act_line}", file=sys.stderr)
    return 1


if len(sys.argv) != 3:
    sys.exit(__doc__)
sys.exit(main(sys.argv[1], sys.argv[2]))
