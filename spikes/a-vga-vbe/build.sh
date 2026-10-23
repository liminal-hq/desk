#!/bin/bash
# Builds VGAVBE.EXE into dist/a/ the way d0-rust-dos builds its programs: Rust, rust-lld, then tools/elf2go32.py behind DJGPP's stub.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/a-vga-vbe/build.sh        (after tools/setup.sh)
set -euo pipefail
cd "$(dirname "$0")/../.."

OUT=dist/a
mkdir -p "$OUT/obj"

(cd spikes/a-vga-vbe && RUSTC_BOOTSTRAP=1 cargo build --release --locked)

STUBIFY="$PWD/vendor/djgpp/djgpp/i586-pc-msdosdjgpp/bin/stubify"
[ -x "$STUBIFY" ] || { echo "build: DJGPP not found in vendor/; run tools/setup.sh" >&2; exit 1; }
rm -f "$OUT/obj/stub.exe"
"$STUBIFY" -g "$OUT/obj/stub.exe" > /dev/null

tools/elf2go32.py "$OUT/obj/stub.exe" spikes/a-vga-vbe/target/dos-elf/release/vgavbe "$OUT/VGAVBE.EXE"
cp vendor/cwsdpmi/bin/CWSDPMI.EXE "$OUT/CWSDPMI.EXE"
echo "build: $OUT/VGAVBE.EXE ($(wc -c < "$OUT/VGAVBE.EXE") bytes)"
