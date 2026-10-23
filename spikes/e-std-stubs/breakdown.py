#!/usr/bin/env python3
# Sums the symbol sizes of an ELF by the crate each symbol belongs to: where the bytes of a std program go.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/e-std-stubs/breakdown.py <elf> [top-n-symbols]
import collections
import re
import subprocess
import sys

elf = sys.argv[1]
top = int(sys.argv[2]) if len(sys.argv) > 2 else 12
out = subprocess.run(["nm", "-S", "-C", "--size-sort", elf], capture_output=True, text=True, check=True).stdout
by_crate = collections.Counter()
syms = []
for line in out.splitlines():
    parts = line.split(None, 3)
    if len(parts) < 4 or parts[2].lower() not in "tdrb":
        continue
    size, kind, name = int(parts[1], 16), parts[2], parts[3]
    m = re.match(r"^<?(?:&?(?:mut )?)?(?:\[)?([A-Za-z_][A-Za-z0-9_]*)::", name)
    crate = m.group(1) if m else "(other)"
    if kind.lower() == "b":
        crate += " [bss]"
    by_crate[crate] += size
    syms.append((size, name))
for crate, size in by_crate.most_common():
    print(f"{size:8d}  {crate}")
print()
for size, name in sorted(syms, reverse=True)[:top]:
    print(f"{size:8d}  {name[:110]}")
