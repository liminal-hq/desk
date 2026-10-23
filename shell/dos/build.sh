#!/bin/bash
# Builds SHELL.EXE into dist/shell/ the way spike A builds VGAVBE.EXE: Rust, rust-lld, then tools/elf2go32.py behind DJGPP's stub. The
# directory also gets CWSDPMI.EXE, the two faces for `fontfile` and the fonts' licence.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: shell/dos/build.sh        (after tools/setup.sh)
set -euo pipefail
cd "$(dirname "$0")/../.."

OUT=dist/shell
mkdir -p "$OUT/obj"

(cd shell/dos && RUSTC_BOOTSTRAP=1 cargo build --release --locked)

STUBIFY="$PWD/vendor/djgpp/djgpp/i586-pc-msdosdjgpp/bin/stubify"
[ -x "$STUBIFY" ] || { echo "build: DJGPP not found in vendor/; run tools/setup.sh" >&2; exit 1; }
rm -f "$OUT/obj/stub.exe"
"$STUBIFY" -g "$OUT/obj/stub.exe" > /dev/null

tools/elf2go32.py "$OUT/obj/stub.exe" shell/dos/target/dos-elf/release/shell "$OUT/SHELL.EXE"
cp vendor/cwsdpmi/bin/CWSDPMI.EXE "$OUT/CWSDPMI.EXE"
cp fonts/generated/ui.dfn "$OUT/UI.DFN"
cp fonts/generated/title.dfn "$OUT/TITLE.DFN"
cp fonts/generated/OFL.txt "$OUT/OFL.txt"
echo "build: $OUT/SHELL.EXE ($(wc -c < "$OUT/SHELL.EXE") bytes)"
