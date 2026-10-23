#!/usr/bin/env python3
# Converts the COFF objects rustc emits for dos.json into ones DJGPP's linker lays out and relocates correctly.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

"""Convert a Rust static library for DOS into a DJGPP-linkable archive.

The target specification borrows LLVM's 32-bit MinGW triple, so rustc writes PE-flavoured i386 COFF objects. DJGPP's `ld` opens
them (it calls the format `coff-go32`) and links them without a single error, but the program it produces is wrong, because
go32 differs from PE in two ways that this script repairs, plus one object it must leave out:

1. PC-relative addends. For an IMAGE_REL_I386_REL32 relocation (the target of a `call` or `jmp`) PE stores just the addend in
   the instruction field, while go32 expects the field to hold the addend minus the field's own offset in the section plus 4.
   Unpatched calls land a few bytes away from their target.
2. Sections. PE objects carry one section per function (`.text$name`), plus `.rdata`, `.xdata` and `.pdata`. A go32 image has
   three loaded sections, `.text`, `.data` and `.bss`, so each section is renamed and re-flagged into one of them. Unfolded
   sections are placed outside the loaded image.
3. `compiler_builtins`. Its objects declare weak externals, which coff-go32 does not understand. They are dropped from the
   archive: DJGPP's libc and libgcc supply `memcpy`, `memset`, `__udivdi3` and the other helpers the Rust object leaves as
   undefined symbols.

usage: coff2go32.py <in.a> <out.a> <ar tool>
  <ar tool> is DJGPP's archiver (i586-pc-msdosdjgpp-ar), used to read and write the archives so the result has a go32 symbol index.
"""
import os
import struct
import subprocess
import sys
import tempfile

COFF_MACHINE_I386 = 0x14C
IMAGE_REL_I386_REL32 = 0x14

# Section header layout: name[8] vsize vaddr rawsize rawptr relptr lnnoptr nreloc nlnno flags.
HEADER_SIZE = 40
SECTION_HEADERS_AT = 20  # after the 20-byte file header (the object has no optional header)

# go32 section flags (STYP_*).
STYP_TEXT, STYP_DATA, STYP_BSS, STYP_INFO = 0x20, 0x40, 0x80, 0x200

DATA_LIKE = {".data", ".rdata", ".xdata", ".pdata", ".ctors", ".dtors", ".CRT"}


def go32_section(name):
    """Return the go32 (name, flags) a PE section is folded into."""
    base = name.split("$")[0]
    if base == ".text":
        return ".text", STYP_TEXT
    if base == ".bss":
        return ".bss", STYP_BSS
    if base in DATA_LIKE:
        return ".data", STYP_DATA
    return ".drectve", STYP_INFO  # linker directives and anything else are not loaded


def section_name(data, header, string_table):
    """Read a section name; names longer than 8 bytes are `/offset` into the string table."""
    raw = bytes(data[header : header + 8])
    if raw.startswith(b"/"):
        start = string_table + int(raw[1:].split(b"\0")[0])
        return data[start : data.index(b"\0", start)].decode()
    return raw.split(b"\0")[0].decode()


def convert_object(path):
    """Rewrite one object in place. Returns the number of REL32 relocations patched."""
    data = bytearray(open(path, "rb").read())
    machine, section_count = struct.unpack_from("<HH", data, 0)
    if machine != COFF_MACHINE_I386:
        sys.exit(f"coff2go32: {path} is not an i386 COFF object")
    symbol_table, symbol_count = struct.unpack_from("<II", data, 8)
    string_table = symbol_table + symbol_count * 18

    patched = 0
    for index in range(section_count):
        header = SECTION_HEADERS_AT + index * HEADER_SIZE
        new_name, flags = go32_section(section_name(data, header, string_table))
        raw_pointer, reloc_pointer = struct.unpack_from("<II", data, header + 20)
        (reloc_count,) = struct.unpack_from("<H", data, header + 32)
        data[header : header + 8] = new_name.encode().ljust(8, b"\0")
        struct.pack_into("<I", data, header + 36, flags)

        for r in range(reloc_count):
            offset, _symbol, kind = struct.unpack_from("<IIH", data, reloc_pointer + r * 10)
            if kind == IMAGE_REL_I386_REL32 and raw_pointer:
                field = raw_pointer + offset
                (addend,) = struct.unpack_from("<i", data, field)
                struct.pack_into("<i", data, field, addend - (offset + 4))
                patched += 1

    open(path, "wb").write(data)
    return patched


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    source, target, ar = sys.argv[1:]
    with tempfile.TemporaryDirectory() as work:
        subprocess.run([ar, "x", os.path.abspath(source)], cwd=work, check=True)
        objects = sorted(f for f in os.listdir(work) if f.endswith(".o"))
        kept = [o for o in objects if not o.startswith("compiler_builtins")]
        patched = sum(convert_object(os.path.join(work, o)) for o in kept)
        output = os.path.abspath(target)
        if os.path.exists(output):
            os.remove(output)  # `ar rcs` would otherwise add to the old archive
        subprocess.run([ar, "rcs", output] + kept, cwd=work, check=True)
    print(f"coff2go32: kept {len(kept)} of {len(objects)} objects, patched {patched} REL32 relocations -> {target}")


main()
