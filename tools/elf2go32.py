#!/usr/bin/env python3
# Packs a statically linked 32-bit x86 ELF executable into a DJGPP go32 .EXE: the DJGPP stub, then a COFF image the stub loads.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/elf2go32.py [--dpmi NAME] [--info] <stub.exe> <in.elf> <out.exe>
#
# The linker (rust-lld) has already resolved every symbol and relocation; this tool only rearranges bytes. It reads the ELF program
# headers, not the sections: the first PT_LOAD becomes `.text` (code and read-only data), and the second, if there is one, becomes
# `.data` (its file-backed part) and `.bss` (the rest of its memory size). Addresses stay as the linker chose them: they are offsets
# from the base of the block the stub allocates for the image, which is where the stub puts the image (see spikes/d0-rust-dos/rt).
#
# What the stub requires of the COFF image, found by reading its code (the stub ships inside DJGPP's `stubify`; `stubify -g` writes
# it out) and checked by running the results:
#
#   - the file header (20 bytes), the a.out header (28 bytes) and exactly three section headers (40 bytes each: `.text`, `.data`,
#     `.bss`, in that order) at fixed offsets, 168 bytes in all; it reads those 168 bytes and checks only the magic 0x014C;
#   - the entry point is the a.out header's `entry`, a plain offset (the stub far-jumps to code-selector:entry);
#   - each section is copied from `scnptr` (relative to the COFF image) to `vaddr` (relative to the block base) in sector-sized reads;
#     keep `scnptr` a multiple of 512 so nothing before the section is copied along with it;
#   - `.bss` is zeroed by the stub from its `vaddr` for `size` bytes, in dwords;
#   - the block is the end of `.bss` rounded up to 64 KiB, and at least 128 KiB (the stub's own minimum).
# Nothing else in the image is read: no symbols, no relocations, no line numbers.
import struct
import sys


def fail(msg):
    sys.exit("elf2go32: " + msg)


def parse_elf(data):
    if data[:4] != b"\x7fELF" or data[4] != 1 or data[5] != 1:
        fail("not a 32-bit little-endian ELF file")
    e_type, e_machine, _ver, e_entry, e_phoff, _shoff, _flags, _ehsize, e_phentsize, e_phnum = struct.unpack_from(
        "<HHIIIIIHHH", data, 16
    )
    if e_type != 2:
        fail("not an executable (ET_EXEC); link with -static and no PIE")
    if e_machine != 3:
        fail("not an Intel 80386 ELF file")
    loads = []
    for i in range(e_phnum):
        p_type, p_offset, p_vaddr, _paddr, p_filesz, p_memsz, p_flags, _align = struct.unpack_from(
            "<IIIIIIII", data, e_phoff + i * e_phentsize
        )
        if p_type == 1 and p_memsz:
            loads.append((p_vaddr, p_offset, p_filesz, p_memsz, p_flags))
    loads.sort()
    return e_entry, loads


def align(x, a):
    return (x + a - 1) & ~(a - 1)


def main():
    args = sys.argv[1:]
    dpmi = None
    info = False
    while args and args[0].startswith("--"):
        opt = args.pop(0)
        if opt == "--dpmi":
            dpmi = args.pop(0)
        elif opt == "--info":
            info = True
        else:
            fail("unknown option " + opt)
    if len(args) != 3:
        sys.exit(__doc__ and "usage: tools/elf2go32.py [--dpmi NAME] [--info] <stub.exe> <in.elf> <out.exe>")
    stub_path, elf_path, out_path = args

    stub_all = open(stub_path, "rb").read()
    if stub_all[:2] != b"MZ":
        fail("the stub is not an MZ executable")
    last, pages = struct.unpack_from("<HH", stub_all, 2)
    stub_size = pages * 512 - ((512 - last) % 512 if last else 0)
    stub = bytearray(stub_all[:stub_size])
    if dpmi is not None:
        # The DPMI server's name is the last field of the stub information block at offset 0x200 (offset 0x44 in the block), 16 bytes.
        if stub[0x200:0x208] != b"go32stub" or len(dpmi) > 15:
            fail("cannot patch the DPMI server name in this stub")
        stub[0x244:0x254] = dpmi.encode("ascii").ljust(16, b"\0")

    elf = open(elf_path, "rb").read()
    entry, loads = parse_elf(elf)
    if not 1 <= len(loads) <= 2:
        fail("expected one or two loadable segments (text, then data and bss), found %d" % len(loads))
    t_vaddr, t_off, t_filesz, t_memsz, _t_flags = loads[0]
    if t_memsz != t_filesz:
        fail("the text segment has a bss tail; put .bss in the data segment")
    text = elf[t_off : t_off + t_filesz]
    if len(loads) == 2:
        d_vaddr, d_off, d_filesz, d_memsz, _ = loads[1]
        data = elf[d_off : d_off + d_filesz]
        bss_size = d_memsz - d_filesz
        if d_vaddr < t_vaddr + len(text):
            fail("the data segment overlaps the text segment")
    else:
        d_vaddr, data, bss_size = align(t_vaddr + len(text), 4), b"", 0
    # The stub copies and clears in dwords.
    text += b"\0" * (align(len(text), 4) - len(text))
    data += b"\0" * (align(len(data), 4) - len(data))
    bss_vaddr = d_vaddr + len(data)
    bss_size = align(bss_size, 4)
    if not t_vaddr <= entry < t_vaddr + len(text):
        fail("the entry point 0x%x is outside the text segment" % entry)

    t_scn = 512
    d_scn = t_scn + align(len(text), 512)
    end = d_scn + align(len(data), 512)

    # File header: magic, sections, time, symbol table pointer and count, optional header size, flags (no relocations, executable,
    # no line numbers, little-endian: the value DJGPP's own executables carry).
    out = struct.pack("<HHIIIHH", 0x14C, 3, 0, 0, 0, 28, 0x107)
    # a.out header: ZMAGIC, version stamp, sizes, entry, base of text, base of data.
    out += struct.pack("<HHIIIIII", 0x10B, 0, len(text), len(data), bss_size, entry, t_vaddr, d_vaddr)
    out += struct.pack("<8sIIIIIIHHI", b".text", t_vaddr, t_vaddr, len(text), t_scn, 0, 0, 0, 0, 0x20)
    out += struct.pack("<8sIIIIIIHHI", b".data", d_vaddr, d_vaddr, len(data), d_scn, 0, 0, 0, 0, 0x40)
    out += struct.pack("<8sIIIIIIHHI", b".bss", bss_vaddr, bss_vaddr, bss_size, 0, 0, 0, 0, 0, 0x80)
    assert len(out) == 168
    out += b"\0" * (t_scn - len(out))
    out += text + b"\0" * (align(len(text), 512) - len(text))
    out += data + b"\0" * (align(len(data), 512) - len(data))
    assert len(out) == end

    with open(out_path, "wb") as f:
        f.write(bytes(stub) + out)
    if info:
        block = max(align(bss_vaddr + bss_size, 65536), 0x20000)
        print(
            "elf2go32: %s: stub %d + coff %d = %d bytes; text %#x+%#x, data %#x+%#x, bss %#x+%#x, entry %#x, block %d KiB"
            % (out_path, len(stub), len(out), len(stub) + len(out), t_vaddr, len(text), d_vaddr, len(data), bss_vaddr, bss_size, entry, block // 1024)
        )


if __name__ == "__main__":
    main()
