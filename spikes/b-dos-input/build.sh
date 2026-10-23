#!/bin/bash
# Builds INPUT.EXE into dist/b/ the way spike A builds VGAVBE.EXE: Rust, rust-lld, then tools/elf2go32.py behind DJGPP's stub.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/b-dos-input/build.sh        (after tools/setup.sh)
set -euo pipefail
cd "$(dirname "$0")/../.."

OUT=dist/b
mkdir -p "$OUT/obj"

(cd spikes/b-dos-input && RUSTC_BOOTSTRAP=1 cargo build --release --locked)

STUBIFY="$PWD/vendor/djgpp/djgpp/i586-pc-msdosdjgpp/bin/stubify"
[ -x "$STUBIFY" ] || { echo "build: DJGPP not found in vendor/; run tools/setup.sh" >&2; exit 1; }
rm -f "$OUT/obj/stub.exe"
"$STUBIFY" -g "$OUT/obj/stub.exe" > /dev/null

tools/elf2go32.py "$OUT/obj/stub.exe" spikes/b-dos-input/target/dos-elf/release/dosinput "$OUT/INPUT.EXE"
cp vendor/cwsdpmi/bin/CWSDPMI.EXE "$OUT/CWSDPMI.EXE"
echo "build: $OUT/INPUT.EXE ($(wc -c < "$OUT/INPUT.EXE") bytes)"
