#!/bin/bash
# Builds the spike E programs into dist/e/: the no_std baseline, std on the stub platform, std on the borrowed Trusty platform and std on
# the patched Desk platform (for both the "desk" and the "dos" flavour), each packed with tools/elf2go32.py.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/e-std-stubs/build.sh        (after tools/setup.sh)
#
#   no_std + alloc   base       -Zbuild-std=core,alloc          targets/dos-std.json      --> dist/e/base/HELLO.EXE
#   std, stub OS     probe      patched std, os "stub"          targets/stub-std.json     --> dist/e/stub/{PROBE,HELLOSTD,HELLOMAP}.EXE
#   std, os trusty   probe      unmodified std + shim/trusty    targets/dos-trusty.json   --> dist/e/trusty/...
#   std, os desk     probe      patched std + shim/desk         targets/desk-app.json     --> dist/e/desk/...
#   std, os dos      probe      patched std + shim/desk         targets/dos-std.json      --> dist/e/dos/...
#
# The patched std is a copy of the toolchain's rust-src with patch/desk-std.patch applied and patch/overlay/ copied over it
# (patch/mkstd.sh); cargo is pointed at the copy by `__CARGO_TESTS_ONLY_SRC_ROOT`.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD
E=spikes/e-std-stubs
OUT=dist/e
export RUSTC_BOOTSTRAP=1

STUBIFY="$ROOT/vendor/djgpp/djgpp/i586-pc-msdosdjgpp/bin/stubify"
[ -x "$STUBIFY" ] || { echo "build: DJGPP not found in vendor/; run tools/setup.sh" >&2; exit 1; }
mkdir -p "$OUT"
rm -f "$OUT/stub.exe"
"$STUBIFY" -g "$OUT/stub.exe" > /dev/null

# The patched std, once per build.
STD="$ROOT/$E/target/desk-src"
$E/patch/mkstd.sh "$STD"

# pack <flavour directory> <target directory> <cargo bin name> <DOS name>
pack() {
  mkdir -p "$OUT/$1"
  cp vendor/cwsdpmi/bin/CWSDPMI.EXE "$OUT/$1/CWSDPMI.EXE"
  tools/elf2go32.py "$OUT/stub.exe" "$E/target/$2/release/$3" "$OUT/$1/$4.EXE"
}

# 1. The baseline: core and alloc only, the runtime's own panic handler.
(cd $E && cargo build --release --locked --target targets/dos-std.json -p base -Zbuild-std=core,alloc,compiler_builtins)
pack base dos-std base HELLO

# 2. std on the stub platform, the borrowed Trusty platform, and the Desk platform (as os "desk" and as os "dos").
(
  cd $E
  export __CARGO_TESTS_ONLY_SRC_ROOT="$STD/library"
  cargo build --release --locked --target targets/stub-std.json -p probe
  cargo build --release --locked --target targets/desk-app.json -p probe --features desk
  cargo build --release --locked --target targets/dos-std.json -p probe --features desk --target-dir target/dos-flavour
  unset __CARGO_TESTS_ONLY_SRC_ROOT
  cargo build --release --locked --target targets/dos-trusty.json -p probe --features trusty
)
for bin in probe:PROBE hello_std:HELLOSTD hello_map:HELLOMAP; do
  pack stub stub-std "${bin%%:*}" "${bin##*:}"
  pack desk desk-app "${bin%%:*}" "${bin##*:}"
  pack dos dos-flavour/dos-std "${bin%%:*}" "${bin##*:}"
  pack trusty dos-trusty "${bin%%:*}" "${bin##*:}"
done

echo "build: $OUT/{base,stub,trusty,desk,dos}"
