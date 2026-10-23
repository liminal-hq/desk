#!/usr/bin/env python3
# Flattens a statically linked 32-bit ELF into a raw binary: the loadable segments laid out from the lowest load address, gaps
# filled with zeros. Replaces objcopy, so the native build needs no binutils.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tools/elf2bin.py kernel.elf kernel.bin
import struct
import sys


def main(src, dst):
    data = open(src, "rb").read()
    if data[:4] != b"\x7fELF" or data[4] != 1 or data[5] != 1:
        sys.exit(f"{src}: not a 32-bit little-endian ELF file")
    phoff, = struct.unpack_from("<I", data, 28)
    phentsize, phnum = struct.unpack_from("<HH", data, 42)
    segs = []
    for i in range(phnum):
        ptype, off, vaddr, paddr, filesz, memsz, flags, align = struct.unpack_from("<8I", data, phoff + i * phentsize)
        if ptype == 1 and filesz:
            segs.append((paddr, off, filesz))
    if not segs:
        sys.exit(f"{src}: no loadable segments")
    base = min(s[0] for s in segs)
    out = bytearray(max(s[0] + s[2] for s in segs) - base)
    for paddr, off, filesz in segs:
        out[paddr - base:paddr - base + filesz] = data[off:off + filesz]
    open(dst, "wb").write(out)
    print(f"{dst}: {len(out)} bytes, load address {base:#x}")


if len(sys.argv) != 3:
    sys.exit(__doc__)
main(sys.argv[1], sys.argv[2])
