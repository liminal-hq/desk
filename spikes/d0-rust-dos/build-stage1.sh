#!/bin/bash
# Stage 1: the same hello program linked by DJGPP's binutils `ld` with no C library, no crt0 and no gcc.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/d0-rust-dos/build-stage1.sh        (after tools/setup.sh)
#
#   stage1 crate  --cargo -Zbuild-std, target core/dos.json (PE-flavoured COFF)--> libstage1.a
#   libstage1.a   --tools/coff2go32.py (still needed)---------------------------> go32 COFF archive
#   the archive   --i586-pc-msdosdjgpp-ld -nostdlib -static -lgcc---------------> dist/d0/HELLO1.EXE (ld writes the stub itself)
#
# libgcc.a is linked for `__udivdi3`, the one helper the objects need that nothing in the runtime supplies; memcpy and friends come
# from rt/src/mem.rs.
set -euo pipefail
cd "$(dirname "$0")/../.."

DJGPP="$PWD/vendor/djgpp/djgpp"
export PATH="$DJGPP/bin:$PATH"
[ -x "$DJGPP/bin/i586-pc-msdosdjgpp-ld" ] || { echo "build-stage1: DJGPP not found in vendor/; run tools/setup.sh" >&2; exit 1; }
OUT=dist/d0
mkdir -p "$OUT/s1"

(cd spikes/d0-rust-dos && RUSTC_BOOTSTRAP=1 cargo build --release --locked -p stage1 --target ../../core/dos.json)
tools/coff2go32.py spikes/d0-rust-dos/target/dos/release/libstage1.a "$OUT/s1/stage1_go32.a" i586-pc-msdosdjgpp-ar

# The entry symbol is `_start` as the object spells it. `___image_start` and `___image_end` are the C names `__image_start` and
# `__image_end` that rt/src/timer.rs locks between; the default script provides `_end`, and the image starts at 0x1000.
i586-pc-msdosdjgpp-ld -nostdlib -static -e _start --defsym ___image_start=0x1000 --defsym ___image_end=_end \
  -o "$OUT/s1/hello1.exe" "$OUT/s1/stage1_go32.a" -L "$DJGPP/lib/gcc/i586-pc-msdosdjgpp/12.2.0" -lgcc
cp "$OUT/s1/hello1.exe" "$OUT/HELLO1.EXE"
echo "build-stage1: $OUT/HELLO1.EXE ($(wc -c < "$OUT/HELLO1.EXE") bytes)"
