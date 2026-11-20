#!/bin/bash
# Builds the DOS runtime's example programs into dist/d0/ with no C compiler, no C library and no binutils linker.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/build.sh        (after tools/setup.sh)
#
#   dos/examples (Rust, no_std)        --cargo -Zbuild-std, target dos-elf.json, linked by rust-lld with link.ld--> ELF executables
#   the ELF + the DJGPP stub           --dos/tools/elf2go32.py-----------------------------------------------------> dist/d0/NAME.EXE
#   the ELF + CWSDSTUB.EXE             --dos/tools/elf2go32.py-----------------------------------------------------> dist/d0/solo/NAME.EXE
#   vendor/cwsdpmi                                                                                            --> dist/d0/CWSDPMI.EXE
#
# The DJGPP pieces still used are data, not a toolchain: the stub (written out by `stubify -g`) and, for the self-contained
# variant, CWSDPMI's own CWSDSTUB.EXE. Neither gcc, ld, as nor the C library is run.
set -euo pipefail
cd "$(dirname "$0")/.."

OUT=dist/d0
PROGRAMS="hello hw panicky ctx retstart"
mkdir -p "$OUT/obj" "$OUT/solo"

# 1. Rust: one cargo build links every program with rust-lld (the pinned toolchain ships it).
(cd dos && RUSTC_BOOTSTRAP=1 cargo build --release --locked)

# 2. The stub. `stubify -g` writes DJGPP's STUB.EXE (2 KiB) and does nothing else.
STUBIFY="$PWD/vendor/djgpp/djgpp/i586-pc-msdosdjgpp/bin/stubify"
[ -x "$STUBIFY" ] || { echo "build: DJGPP not found in vendor/; run tools/setup.sh" >&2; exit 1; }
rm -f "$OUT/obj/stub.exe"
"$STUBIFY" -g "$OUT/obj/stub.exe" > /dev/null

# 3. Pack. The unit of work is a byte shuffle: dos/tools/elf2go32.py reads the linked ELF and writes stub + go32 COFF image.
for p in $PROGRAMS; do
  name="$(echo "$p" | tr '[:lower:]' '[:upper:]')"
  elf="dos/target/dos-elf/release/$p"
  dos/tools/elf2go32.py "$OUT/obj/stub.exe" "$elf" "$OUT/$name.EXE"
done
dos/tools/elf2go32.py vendor/cwsdpmi/bin/CWSDSTUB.EXE dos/target/dos-elf/release/hello "$OUT/solo/HELLO.EXE"

# 4. The DPMI host the plain executables load when none is running. It must sit beside them.
cp vendor/cwsdpmi/bin/CWSDPMI.EXE "$OUT/CWSDPMI.EXE"

echo "build: $OUT/HELLO.EXE ($(wc -c < "$OUT/HELLO.EXE") bytes), $OUT/solo/HELLO.EXE ($(wc -c < "$OUT/solo/HELLO.EXE") bytes, host included)"
